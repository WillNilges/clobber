use crate::User;

#[derive(Debug)]
pub enum Ping {
    JobCancelled { id: u16, reason: String },
    JobFinished { id: u16 },
    JobStarted { id: u16 },
}

pub fn send_ping(user: User, ping: Ping) {
    println!("Sending ping to {}: {:?}", user.name, ping);
    //TODO implement
}
