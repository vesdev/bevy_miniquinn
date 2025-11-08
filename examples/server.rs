use bevy::prelude::*;
use bevy_miniquinn::server::*;

mod helpers;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, bevy_miniquinn::ServerPlugin))
        .add_systems(Startup, connect)
        .add_systems(Update, on_connect)
        .add_systems(Update, on_disconnect)
        .run();
}

fn connect(mut commands: Commands) {
    commands.spawn(ServerBundle::new(
        "127.0.0.1:4433".parse().unwrap(),
        helpers::insecure_server_config(),
    ));
}

fn on_connect(mut reader: MessageReader<message::ClientConnected>) {
    if reader.read().next().is_some() {
        println!("client connected");
    }
}

fn on_disconnect(mut reader: MessageReader<message::ClientDisconnected>) {
    if reader.read().next().is_some() {
        println!("client connected");
    }
}
