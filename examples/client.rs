use bevy::prelude::*;
use bevy_miniquinn::client::*;

mod helpers;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, bevy_miniquinn::ClientPlugin))
        .add_systems(Startup, connect)
        .add_systems(Update, listen)
        .run();
}

fn connect(mut commands: Commands) {
    commands.spawn(ClientBundle::new(
        "127.0.0.1:4433".parse().unwrap(),
        "my_server".into(),
        helpers::insecure_client_config(),
    ));
}

fn listen(mut reader: MessageReader<message::ServerData>) {
    for msg in reader.read() {
        println!("message from server {}", String::from_utf8_lossy(&msg.data));
    }
}
