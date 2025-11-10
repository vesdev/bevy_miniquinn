use bevy::prelude::*;
use bevy_asynk_strim::StreamValue;
use bevy_miniquinn::client::{self, *};

mod helpers;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, bevy_miniquinn::ClientPlugin))
        .add_systems(Startup, connect)
        .add_systems(Update, listen)
        .run();
}

fn connect(mut commands: Commands) {
    client::connect(
        &mut commands,
        "127.0.0.1:4433".parse().unwrap(),
        "my_server".into(),
        helpers::insecure_client_config(),
    );
}

fn listen(stream: Query<&mut StreamValue<ServerMessage>>) {
    for mut value in stream {
        if let Some(ServerMessage::Data(buf)) = value.consume() {
            info!("Message from remote {}", String::from_utf8_lossy(&buf))
        }
    }
}
