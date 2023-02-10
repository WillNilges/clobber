use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Status,
    Kill,
    Unlock { device_number: u32 },
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Success,
    Error(String),
    GPUStatus { locks: Vec<User> },
}

#[derive(Clone, Eq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub uid: usize,
}
