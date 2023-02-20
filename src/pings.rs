use crate::User;
use core::fmt;

#[derive(Debug)]
pub enum Ping {
    JobCancelled { id: u16, reason: String },
    JobFinished { id: u16 },
    JobStarted { id: u16 },
}

impl fmt::Display for Ping {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use crate::Ping::*;
        match self {
            JobCancelled { id, reason } => write!(f, "Job {} cancelled: {}", id, reason),
            JobStarted { id } => write!(f, "Job {} started", id),
            JobFinished { id } => write!(f, "Job {} finished", id),
        }
    }
}

impl Ping {
    //find a better way to do this
    fn get_route_uuid(&self) -> &str {
        use crate::Ping::*;
        match self {
            JobCancelled { .. } => "97f166a0-d9f9-4888-b8ad-b09656f19330",
            JobStarted { .. } => "f810410a-0dcd-4271-8b75-b6e6ce5ead7e",
            JobFinished { .. } => "aab2a0c6-001c-455a-956f-10c8b685fb87",
        }
    }
}

pub fn send_ping(api_token: String, user: User, ping: Ping) {
    println!("Sending ping to {}: {:?}", user.name, ping);
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        if let Err(e) = client
            .post(format!(
                "https://pings.csh.rit.edu/service/route/{}/ping",
                ping.get_route_uuid()
            ))
            .header("Authorization", format!("Bearer {}", api_token))
            .header("Content-Type", "application/json")
            .body(format!(
                "{{\"username\":\"{}\",\"body\":\"{}\"}}",
                user.name, ping
            ))
            .send()
        {
            eprintln!(
                "Error sending ping to user {} ({}): {:?}",
                user.uid, user.name, e
            );
        }
    });
    //TODO implement
}
