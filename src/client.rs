use std::mem;
use std::net::SocketAddr;

use bevy::prelude::*;
use bevy::tasks::*;
use bevy_asynk_strim::SpawnStreamExt;
use bevy_asynk_strim::asynk_strim;

#[derive(Component)]
pub struct Client {
    pub remote_addr: SocketAddr,
    pub server_name: String,
    _ep: quinn::Endpoint,
}

#[derive(Component)]
pub struct ConnectTask(pub(crate) Task<Option<quinn::Connection>>);

pub fn connect(
    commands: &mut Commands,
    addr: SocketAddr,
    server_name: String,
    config: quinn::ClientConfig,
) {
    let mut ep = quinn::Endpoint::client("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
    ep.set_default_client_config(config);

    let conn_srv_name = server_name.clone();
    let conn_ep = ep.clone();
    let task = IoTaskPool::get()
        .spawn(async move { conn_ep.connect(addr, &conn_srv_name).unwrap().await.ok() });

    info!(
        "Connecting to QUIC server {} at {}:{}...",
        server_name,
        addr.ip(),
        addr.port()
    );

    commands
        .spawn(Client {
            remote_addr: addr,
            server_name,
            _ep: ep,
        })
        .insert(ConnectTask(task));
}

#[allow(unused)]
pub enum ServerMessage {
    Data(Vec<u8>),
    Closed,
    Error,
}

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, handle_connect);
    }
}

fn handle_connect(mut commands: Commands, tasks: Query<(Entity, &Client, &mut ConnectTask)>) {
    for (entity, client, mut task) in tasks {
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        match result {
            Some(conn) => {
                info!(
                    "Connected to server {} at {}:{}",
                    client.server_name,
                    client.remote_addr.ip(),
                    client.remote_addr.port(),
                );

                let addr = client.remote_addr;
                let data_stream = commands.spawn_stream::<ServerMessage, _>(Box::pin(
                    asynk_strim::stream_fn(move |mut yielder| async move {
                        let Ok((_send, recv)) = conn.open_bi().await else {
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
                                        .yield_item(ServerMessage::Data(mem::take(&mut buf)))
                                        .await
                                }
                                Err(_) => yielder.yield_item(ServerMessage::Error).await,
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
                error!(
                    "Failed to connect to server {} at {}:{}",
                    client.server_name,
                    client.remote_addr.ip(),
                    client.remote_addr.port(),
                );
                commands.entity(entity).despawn();
            }
        }
    }
}
