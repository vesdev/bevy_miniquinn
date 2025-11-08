use bevy::{prelude::*, tasks::Task};
type StreamTask = Task<Option<(quinn::SendStream, quinn::RecvStream)>>;
type StreamReadTask = Task<ReadResult>;
pub enum ReadResult {
    Data(Vec<u8>),
    Closed,
    Error,
}

#[derive(Component)]
pub struct Connect(pub(crate) Task<Option<quinn::Connection>>);

#[derive(Component)]
pub struct Stream(pub(crate) StreamTask);

#[derive(Component)]
pub struct StreamRead(pub(crate) StreamReadTask);

#[derive(Component)]
pub struct Accept(pub(crate) Task<Option<quinn::Incoming>>);
