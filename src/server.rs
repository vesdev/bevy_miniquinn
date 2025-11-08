use std::net::SocketAddr;

use bevy::prelude::*;
use bevy::tasks::*;

use crate::component::Connection;
use crate::task;
use crate::task::ReadResult;

#[derive(Bundle)]
pub struct ServerBundle {
    pub server: Server,
    pub accept_task: task::Accept,
}

impl ServerBundle {
    pub fn new(addr: SocketAddr, config: quinn::ServerConfig) -> Self {
        let ep = quinn::Endpoint::server(config, addr).unwrap();
        info!("QUIC server listening on {}:{}", addr.ip(), addr.port());

        let accept_ep = ep.clone();
        let task = IoTaskPool::get().spawn(async move { accept_ep.accept().await });

        Self {
            server: Server { ep, addr },
            accept_task: task::Accept(task),
        }
    }
}

#[derive(Component)]
pub struct Server {
    pub addr: SocketAddr,
    ep: quinn::Endpoint,
}

#[derive(Component)]
struct RemoteAddr(SocketAddr);

pub mod message {
    use bevy::prelude::*;

    #[derive(Message)]
    pub struct ClientConnected {
        pub connection: Entity,
    }

    #[derive(Message)]
    pub struct ClientData {
        pub connection: Entity,
        pub data: Vec<u8>,
    }

    #[derive(Message)]
    pub struct ClientDisconnected {
        pub connection: Entity,
    }
}

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_message::<message::ClientConnected>()
            .add_message::<message::ClientData>()
            .add_message::<message::ClientDisconnected>()
            .add_systems(Update, (accept_task, connecting_task, stream_task));
    }
}

fn accept_task(mut commands: Commands, mut servers: Query<(Entity, &Server, &mut task::Accept)>) {
    for (entity, server, mut task) in &mut servers {
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        match result {
            Some(incoming) => {
                let remote_addr = incoming.remote_address();
                info!("Accepting connection from {}", remote_addr);

                let connect_task = IoTaskPool::get().spawn(async move { incoming.await.ok() });

                commands.spawn((task::Connect(connect_task), RemoteAddr(remote_addr)));

                let accept_ep = server.ep.clone();
                let new_task = IoTaskPool::get().spawn(async move { accept_ep.accept().await });
                task.0 = new_task;
            }
            None => {
                warn!("Server endpoint closed");
                commands.entity(entity).remove::<task::Accept>();
            }
        }
    }
}

fn connecting_task(
    mut commands: Commands,
    mut writer: MessageWriter<message::ClientConnected>,
    tasks: Query<(Entity, &RemoteAddr, &mut task::Connect)>,
) {
    for (entity, remote_addr, mut task) in tasks {
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        match result {
            Some(conn) => {
                info!("Client connected from {}", remote_addr.0);

                commands
                    .entity(entity)
                    .remove::<task::Connect>()
                    .remove::<RemoteAddr>()
                    .insert(Connection(conn));

                writer.write(message::ClientConnected { connection: entity });
            }
            None => {
                error!("Failed to establish connection from {}", remote_addr.0);
                commands.entity(entity).despawn();
            }
        }
    }
}

fn stream_task(
    mut commands: Commands,
    mut data_writer: MessageWriter<message::ClientData>,
    mut disconnect_writer: MessageWriter<message::ClientDisconnected>,
    streamless: Query<(Entity, &Connection), Without<task::Stream>>,
    mut opened: Query<(Entity, &mut task::Stream)>,
    mut reading: Query<(Entity, &Connection, &mut task::StreamRead)>,
) {
    // spawn task to accept stream
    for (entity, conn) in &streamless {
        let conn = conn.0.clone();
        let task = IoTaskPool::get().spawn(async move { conn.accept_bi().await.ok() });
        commands.entity(entity).insert(task::Stream(task));
    }

    // poll until stream accepted and spawn read task
    for (entity, mut open_task) in &mut opened {
        if let Some(result) = block_on(poll_once(&mut open_task.0)) {
            commands.entity(entity).remove::<task::Stream>();

            if let Some((_, recv)) = result {
                let task = IoTaskPool::get().spawn(async move {
                    let mut recv = recv;
                    let mut buf = Vec::new();
                    loop {
                        match recv.read_chunk(1024, true).await {
                            Ok(Some(chunk)) => buf.extend_from_slice(&chunk.bytes),
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
                ReadResult::Data(data) => {
                    data_writer.write(message::ClientData {
                        connection: entity,
                        data,
                    });
                    commands.entity(entity).remove::<task::StreamRead>();
                }
                ReadResult::Closed => {
                    info!("Client closed connection");
                    disconnect_writer.write(message::ClientDisconnected { connection: entity });
                    commands.entity(entity).despawn();
                }
                ReadResult::Error => {
                    error!("Client connection error");
                    disconnect_writer.write(message::ClientDisconnected { connection: entity });
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
