use std::{net::SocketAddr, sync::Arc};

use bevy::prelude::*;
use bevy::tasks::*;
use quinn::{Endpoint, crypto};

#[derive(Component)]
pub struct Client {
    pub addr: SocketAddr,
    pub server_name: String,
    #[allow(unused)]
    ep: Endpoint,
}

impl Client {
    pub fn new(
        addr: SocketAddr,
        server_name: String,
        crypto: Arc<dyn crypto::ClientConfig>,
    ) -> (Client, ConnectionTask) {
        let mut ep = Endpoint::client(addr).unwrap();
        let client_cfg = quinn::ClientConfig::new(crypto);
        ep.set_default_client_config(client_cfg);

        let conn_srv_name = server_name.clone();
        let conn_ep = ep.clone();
        let task = IoTaskPool::get()
            .spawn(async move { conn_ep.connect(addr, &conn_srv_name).unwrap().await.ok() });

        info!(
            "Connectiong to QUIC server {} at {}:{}...",
            server_name,
            addr.ip(),
            addr.port()
        );

        (
            Client {
                ep,
                addr,
                server_name: server_name.clone(),
            },
            ConnectionTask(task),
        )
    }
}

#[derive(Component)]
pub struct Connection(quinn::Connection);

#[derive(Component)]
pub struct ConnectionTask(Task<Option<quinn::Connection>>);

#[derive(Component)]
pub struct Stream(Option<Task<Option<Vec<u8>>>>);

impl Stream {
    pub fn try_recv(&mut self) -> Option<Vec<u8>> {
        if let Some(Some(result)) = self.0.as_mut().map(|task| block_on(poll_once(task))) {
            self.0 = None;
            result
        } else {
            None
        }
    }
}

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, (connection_task, listen));
    }
}

#[allow(unused)]
fn connection_task(mut commands: Commands, tasks: Query<(Entity, &Client, &mut ConnectionTask)>) {
    for (entity, client, mut task) in tasks {
        let Some(result) = block_on(poll_once(&mut task.0)) else {
            continue;
        };

        let Some(conn) = result else {
            error!(
                "Failed to connect to server {} at {}:{}",
                client.server_name,
                client.addr.ip(),
                client.addr.port(),
            );
            continue;
        };

        commands
            .entity(entity)
            .remove::<ConnectionTask>()
            .insert((Connection(conn), Stream(None)));
    }
}

#[allow(unused)]
fn listen(connections: Query<(&mut Connection, &mut Stream)>) {
    for (conn, mut stream) in connections {
        if stream.0.is_none() {
            let conn = conn.0.clone();
            let task = IoTaskPool::get().spawn(async move {
                if let Ok((mut send, mut recv)) = conn.open_bi().await {
                    let mut buf = Vec::new();
                    while let Ok(Some(chunk)) = recv.read_chunk(1024, true).await {
                        buf.extend_from_slice(&chunk.bytes);
                    }
                    Some(buf)
                } else {
                    None
                }
            });
            stream.0 = Some(task);
        }
    }
}
