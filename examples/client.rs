use bevy::prelude::*;
use bevy_miniquinn::client::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, bevy_miniquinn::ClientPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, listen);
}

#[allow(unreachable_code)]
fn setup(mut commands: Commands) {
    commands.spawn(Client::new(
        "0.0.0.0:443".parse().unwrap(),
        "my_server".into(),
        // TLS
        todo!(),
    ));
}

fn listen(streams: Query<&mut Stream>) {
    for mut stream in streams {
        if let Some(data) = stream.try_recv() {
            println!("message from server {}", String::from_utf8_lossy(&data));
        }
    }
}
