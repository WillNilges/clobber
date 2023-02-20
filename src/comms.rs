use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Status,
    Kill,
    SetWatch {
        device_number: u32,
        watching: bool,
    },
    QueueJob {
        user: User,
        image_id: String,
        gpus: Vec<u32>,
    },
    ActiveJobs,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Success,
    Error(String),
    GPUStatus { locks: Vec<Option<User>> },
    ActiveJobs { jobs: Vec<(u16, u32)> },
}

#[derive(Clone, Eq, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub uid: usize,
}

impl PartialEq for User {
    fn eq(&self, other: &User) -> bool {
        self.uid == other.uid
    }
}
