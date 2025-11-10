use bevy::prelude::*;
use bevy_asynk_strim::StreamValue;
use bevy_miniquinn::server::{self, *};

mod helpers;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, bevy_miniquinn::ServerPlugin))
        .add_systems(Startup, connect)
        .add_systems(Update, listen)
        .run();
}

fn connect(mut commands: Commands) {
    server::create(
        &mut commands,
        "127.0.0.1:4433".parse().unwrap(),
        helpers::insecure_server_config(),
    );
}

fn listen(stream: Query<&mut StreamValue<ClientMessage>>) {
    for mut value in stream {
        if let Some(ClientMessage::Data(buf)) = value.consume() {
            info!("Message from remote {}", String::from_utf8_lossy(&buf))
        }
    }
}

// fn listen(mut reader: MessageReader<ClientMessage>, addrs: Query<&RemoteAddr>) {
//     for msg in reader.read() {
//         match msg {
//             ClientMessage::Data { data, .. } => {
//                 if !data.is_empty() {
//                     println!("message from server {}", String::from_utf8_lossy(data));
//                 }
//             }
//             ClientMessage::Connected { client } => {
//                 let addr = addrs.get(*client).unwrap().0;
//                 println!("Client connected at: {addr}");
//             }
//             ClientMessage::Disconnected { client } => {
//                 let addr = addrs.get(*client).unwrap().0;
//                 println!("Client disconnected at: {addr}");
//             }
//         }
//     }
// }
