use std::net::SocketAddr;

use bevy::prelude::*;
use bevy::tasks::*;

use crate::client::message::ServerData;
use crate::component::Connection;
use crate::task;

#[derive(Bundle)]
pub struct ClientBundle {
    pub client: Client,
    pub connection_task: task::Connect,
}

impl ClientBundle {
    pub fn new(addr: SocketAddr, server_name: String, config: quinn::ClientConfig) -> Self {
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

        Self {
            client: Client {
                ep,
                addr,
                server_name,
            },
            connection_task: task::Connect(task),
        }
    }
}

#[derive(Component)]
pub struct Client {
    pub addr: SocketAddr,
    pub server_name: String,
    #[allow(unused)]
    ep: quinn::Endpoint,
}

pub mod message {
    use bevy::prelude::*;
    #[derive(Message)]
    pub struct ServerData {
        /// which client received the data
        pub client: Entity,
        pub data: Vec<u8>,
    }
}

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_message::<ServerData>()
            .add_systems(Update, (connection_task, stream_task));
    }
}

fn connection_task(mut commands: Commands, tasks: Query<(Entity, &Client, &mut task::Connect)>) {
    for (entity, client, mut task) in tasks {
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        match result {
            Some(conn) => {
                info!(
                    "Connected to server {} at {}:{}",
                    client.server_name,
                    client.addr.ip(),
                    client.addr.port(),
                );
                commands
                    .entity(entity)
                    .remove::<task::Connect>()
                    .insert(Connection(conn));
            }
            None => {
                error!(
                    "Failed to connect to server {} at {}:{}",
                    client.server_name,
                    client.addr.ip(),
                    client.addr.port(),
                );
                commands.entity(entity).remove::<task::StreamRead>();
            }
        }
    }
}

fn stream_task(
    mut commands: Commands,
    mut writer: MessageWriter<message::ServerData>,
    streamless: Query<(Entity, &Connection), Without<task::Stream>>,
    mut opened: Query<(Entity, &mut task::Stream)>,
    mut reading: Query<(Entity, &Connection, &mut task::StreamRead)>,
) {
    // spawn task to open stream
    for (entity, conn) in &streamless {
        let conn = conn.0.clone();
        let task = IoTaskPool::get().spawn(async move { conn.open_bi().await.ok() });
        commands.entity(entity).insert(task::Stream(task));
    }

    // poll until open and spawn read task
    for (entity, mut open_task) in &mut opened {
        if let Some(result) = block_on(poll_once(&mut open_task.0)) {
            commands.entity(entity).remove::<task::Stream>();

            if let Some((_, recv)) = result {
                let task = IoTaskPool::get().spawn(async move {
                    let mut recv = recv;
                    let mut buf = Vec::new();
                    loop {
                        match recv.read_chunk(1024, true).await {
                            Ok(Some(chunk)) => {
                                buf.extend_from_slice(&chunk.bytes);
                            }
                            Ok(None) => {
                                return task::ReadResult::Data(buf);
                            }
                            Err(_) => return task::ReadResult::Error,
                        }
                    }
                });
                commands.entity(entity).insert(task::StreamRead(task));
            }
        }
    }

    // poll for read
    for (entity, _, mut stream) in &mut reading {
        if let Some(result) = block_on(poll_once(&mut stream.0)) {
            match result {
                task::ReadResult::Data(data) => {
                    writer.write(message::ServerData {
                        client: entity,
                        data,
                    });
                    commands.entity(entity).remove::<task::StreamRead>();
                }
                task::ReadResult::Closed => {
                    info!("Server closed connection");
                    commands.entity(entity).despawn();
                }
                task::ReadResult::Error => {
                    error!("Connection error with server");
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
