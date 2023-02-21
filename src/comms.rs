use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ClobberConfig {
    pub pings_api_key: String,
}

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

#[derive(Serialize, Deserialize, Clone)]
pub struct JobDesc {
    pub id: u16,
    pub owner: User,
    pub requested_gpus: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Success,
    Error(String),
    GPUStatus { locks: Vec<Option<User>> },
    ActiveJobs(Vec<JobDesc>),
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

pub fn get_config() -> Result<ClobberConfig, String> {
    let file = match std::fs::read_to_string("/etc/clobber/config.json") {
        Ok(f) => f,
        Err(e) => return Err(e.to_string()),
    };
    serde_json::from_str::<ClobberConfig>(&file).map_err(|e| e.to_string())
}
