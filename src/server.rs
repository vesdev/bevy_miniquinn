use std::mem;
use std::net::SocketAddr;

use bevy::prelude::*;
use bevy::tasks::*;
use bevy_asynk_strim::SpawnStreamExt;
use bevy_asynk_strim::StreamPlugin;
use bevy_asynk_strim::StreamValue;
use bevy_asynk_strim::asynk_strim;
use quinn::Incoming;

use crate::component::RemoteAddr;
use crate::component::RemoteBundle;

pub fn create(commands: &mut Commands, addr: SocketAddr, config: quinn::ServerConfig) {
    let ep = quinn::Endpoint::server(config, addr).unwrap();
    info!("QUIC server listening on {}:{}", addr.ip(), addr.port());

    let accept_ep = ep.clone();
    let stream = commands.spawn_stream_marked::<Incoming, _, IncomingStream>(Box::pin(
        asynk_strim::stream_fn(|mut yielder| async move {
            loop {
                if let Some(incoming) = accept_ep.accept().await {
                    yielder.yield_item(incoming).await;
                }
            }
        }),
    ));

    commands.spawn(Server { _ep: ep, addr }).add_child(stream);
}

#[derive(Component)]
pub struct Server {
    pub addr: SocketAddr,
    _ep: quinn::Endpoint,
}

#[derive(Component, Default)]
pub struct IncomingStream;

#[derive(Component)]
pub struct ConnectTask(pub(crate) Task<Option<quinn::Connection>>);

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_plugins(StreamPlugin)
            .add_systems(Update, (handle_incoming, handle_connect));
    }
}

#[allow(clippy::type_complexity)]
fn handle_incoming(
    mut commands: Commands,
    mut incoming: Query<
        (&mut StreamValue<Incoming>, &ChildOf),
        (With<IncomingStream>, Changed<StreamValue<Incoming>>),
    >,
) {
    for (mut value, parent) in &mut incoming {
        let Some(incoming) = value.consume() else {
            continue;
        };

        let addr = incoming.remote_address();
        info!("Accepting connection from {}", addr);

        let task = IoTaskPool::get().spawn(async move { incoming.await.ok() });

        let remote = commands
            .spawn(RemoteBundle {
                addr: RemoteAddr(addr),
            })
            .insert(ConnectTask(task))
            .id();

        commands.entity(parent.0).add_child(remote);
    }
}

#[allow(unused)]
pub enum ClientMessage {
    Data(Vec<u8>),
    Closed,
    Error,
}

fn handle_connect(mut commands: Commands, tasks: Query<(Entity, &RemoteAddr, &mut ConnectTask)>) {
    for (entity, remote_addr, mut task) in tasks {
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        match result {
            Some(conn) => {
                info!("Client connected from {}", remote_addr.0);

                let addr = remote_addr.0;
                let data_stream = commands.spawn_stream::<ClientMessage, _>(Box::pin(
                    asynk_strim::stream_fn(move |mut yielder| async move {
                        let Ok((_send, recv)) = conn.accept_bi().await else {
                            info!("Failed to open bidirectional stream from {}", addr);
                            return;
                        };

                        let mut recv = recv;
                        let mut buf = Vec::new();
                        loop {
                            match recv.read_chunk(1024, true).await {
                                Ok(Some(chunk)) => buf.extend_from_slice(&chunk.bytes),
                                Ok(None) => {
                                    yielder
                                        .yield_item(ClientMessage::Data(mem::take(&mut buf)))
                                        .await
                                }
                                Err(_) => yielder.yield_item(ClientMessage::Error).await,
                            }
                        }
                    }),
                ));

                commands
                    .entity(entity)
                    .remove::<ConnectTask>()
                    .add_child(data_stream);
            }
            None => {
                error!("Failed to establish connection from {}", remote_addr.0);
                commands.entity(entity).despawn();
            }
        }
    }
}
