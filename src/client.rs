use std::net::SocketAddr;

use bevy::prelude::*;
use bevy::tasks::*;

#[derive(Bundle)]
pub struct ClientBundle {
    pub client: Client,
    pub connection_task: ConnectionTask,
}

impl ClientBundle {
    pub fn new(addr: SocketAddr, server_name: String, config: quinn::ClientConfig) -> Self {
        let mut ep = quinn::Endpoint::client(addr).unwrap();
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
            connection_task: ConnectionTask(task),
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

#[derive(Component)]
pub struct Connection(quinn::Connection);

#[derive(Component)]
pub struct ConnectionTask(Task<Option<quinn::Connection>>);

#[derive(Component, Default)]
pub struct StreamTask(Option<Task<Option<Vec<u8>>>>);

pub mod message {
    use bevy::prelude::*;
    #[derive(Message)]
    pub struct Data {
        pub client: Entity,
        pub data: Vec<u8>,
    }
}

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, (connection_task, stream_task));
    }
}

fn connection_task(mut commands: Commands, tasks: Query<(Entity, &Client, &mut ConnectionTask)>) {
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
                    .remove::<ConnectionTask>()
                    .insert((Connection(conn), StreamTask::default()));
            }
            None => {
                error!(
                    "Failed to connect to server {} at {}:{}",
                    client.server_name,
                    client.addr.ip(),
                    client.addr.port(),
                );
                commands.entity(entity).remove::<ConnectionTask>();
            }
        }
    }
}

fn stream_task(
    mut writer: MessageWriter<message::Data>,
    mut connections: Query<(Entity, &Connection, &mut StreamTask)>,
) {
    for (_, conn, mut stream) in &mut connections {
        if stream.0.is_none() {
            let conn = conn.0.clone();
            let task = IoTaskPool::get().spawn(async move {
                match conn.open_bi().await {
                    Ok((_send, mut recv)) => {
                        let mut buf = Vec::new();
                        while let Ok(Some(chunk)) = recv.read_chunk(1024, true).await {
                            buf.extend_from_slice(&chunk.bytes);
                        }
                        Some(buf)
                    }
                    Err(_) => None,
                }
            });
            stream.0 = Some(task);
        }
    }

    for (entity, _, mut stream) in &mut connections {
        if stream.0.is_some() {
            let result = stream.0.as_mut().map(|task| block_on(poll_once(task)));
            if let Some(Some(result)) = result {
                stream.0 = None;

                if let Some(result) = result {
                    writer.write(message::Data {
                        client: entity,
                        data: result,
                    });
                }
            }
        }
    }
}
