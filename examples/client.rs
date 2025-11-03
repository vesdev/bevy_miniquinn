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
    commands.spawn(Client::new(
        "0.0.0.0:443".parse().unwrap(),
        "my_server".into(),
        helpers::insecure_client_config(),
    ));
}

fn listen(streams: Query<&mut Stream>) {
    for mut stream in streams {
        if let Some(data) = stream.try_recv() {
            println!("message from server {}", String::from_utf8_lossy(&data));
        }
    }
}
