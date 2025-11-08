use bevy::prelude::*;

#[derive(Component)]
pub struct Connection(pub(crate) quinn::Connection);
