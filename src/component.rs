use std::net::SocketAddr;

use bevy::prelude::*;

#[derive(Bundle)]
pub struct RemoteBundle {
    pub addr: RemoteAddr,
}

#[derive(Component)]
pub struct RemoteAddr(pub SocketAddr);
