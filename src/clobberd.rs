use comms::ClobberConfig;
use comms::Command::*;
use comms::Response::*;
use comms::{Command, Response, User};
use gpu::GPU;
use pings::Ping::*;
use pings::*;
use pod::*;
use rand;
use std::collections::VecDeque;
use std::io::prelude::*;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tokio::*;

mod comms;
mod gpu;
mod pings;
mod pod;

struct DeviceState {
    device_number: u32,
    watching: bool, //false if there should be no scheduling on this GPU
}

#[derive(Clone)]
struct Job {
    id: u16,
    owner: User,
    image_id: String,
    requested_gpus: Vec<u32>,
    container: Option<String>,
}

impl PartialEq for Job {
    fn eq(&self, other: &Job) -> bool {
        self.id == other.id
    }
}

struct SharedState {
    devices: Vec<DeviceState>,
    queued_jobs: VecDeque<Job>,
    active_jobs: Vec<Job>,
    config: ClobberConfig,
}

impl SharedState {
    fn device_owner(&self, device_number: u32) -> Option<&Job> {
        self.active_jobs
            .iter()
            .find(|job| job.requested_gpus.contains(&device_number))
    }
}

fn sock_communicate(shared_state: &mut SharedState, command: Command) -> Response {
    match command {
        Status => GPUStatus {
            locks: shared_state
                .devices
                .iter()
                .map(|d| d.device_number)
                .map(|d| shared_state.device_owner(d))
                .map(|j| j.map(|j| j.owner.clone()))
                .collect(),
        },
        Kill => {
            println!("Received Kill command. Exiting.");
            std::process::exit(0);
        }
        SetWatch {
            device_number,
            watching,
        } => {
            println!(
                "Set watching status of GPU {} to {}",
                device_number, watching
            );
            for device in &mut shared_state.devices {
                if device.device_number == device_number {
                    device.watching = watching;
                    return Success;
                }
            }
            println!("No device with device number {}", device_number);
            Error("Invalid device number".to_string())
        }
        QueueJob {
            user,
            image_id,
            gpus,
        } => {
            let job = Job {
                id: rand::random(),
                owner: user,
                image_id: image_id,
                requested_gpus: gpus,
                container: None,
            };
            shared_state.queued_jobs.push_back(job.clone());
            println!(
                "Queued job {} from uid {} ({:?}) from image {}",
                job.id, job.owner.uid, job.owner.name, job.image_id
            );
            Success
        }
        #[allow(unreachable_patterns)] //Fallback
        _ => {
            println!("Unimplemented command.");
            Error("Unimplemented command".to_string())
        }
    }
}

fn bind(path: impl AsRef<Path>) -> std::io::Result<UnixListener> {
    let path = path.as_ref();
    let _ = std::fs::remove_file(path);
    UnixListener::bind(path)
}

fn pop_first_qualified_job(shared_state: &mut SharedState) -> Option<Job> {
    let current_gpus = shared_state
        .active_jobs
        .iter()
        .map(|j| j.requested_gpus.clone())
        .flatten()
        .collect::<Vec<u32>>();
    let mut job_index = 0;
    let mut found_job = false;
    for (index, job) in shared_state.queued_jobs.iter().enumerate() {
        if job.requested_gpus.iter().all(|g| !current_gpus.contains(g)) {
            job_index = index;
            found_job = true;
            break;
        }
    }
    if found_job {
        shared_state.queued_jobs.remove(job_index)
    } else {
        None
    }
}

async fn remove_finished_jobs(shared_state: &mut SharedState) -> Vec<Job> {
    let mut remove: Vec<Job> = vec![];
    for job in &shared_state.active_jobs {
        let pod = Pod::new(job.owner.uid as u32);
        match pod
            .container_finished(job.container.as_ref().unwrap())
            .await
        {
            Ok(finished) => {
                if finished {
                    println!("Container finished for Job {}", job.id);
                    remove.push(job.clone());
                }
            }
            Err(e) => {
                eprintln!("Error determining container state: {}", e);
                remove.push(job.clone());
            }
        }
    }
    shared_state.active_jobs.retain(|j| !remove.contains(&j));
    for job in &remove {
        send_ping(
            shared_state.config.pings_api_key.clone(),
            job.owner.clone(),
            JobFinished { id: job.id },
        );
    }
    remove
}

async fn start_job<'a>(
    shared_state: &'a mut SharedState,
    pod: &Pod,
    job: &mut Job,
) -> Result<&'a Job, String> {
    let container_id = match pod.create_container(job.image_id.clone()).await {
        Ok(id) => id,
        Err(e) => return Err(e),
    };
    job.container = Some(container_id);
    shared_state.active_jobs.push(job.clone());
    Ok(shared_state.active_jobs.last().unwrap())
}

async fn try_start_job(shared_state: &mut SharedState) {
    if let Some(mut job) = pop_first_qualified_job(shared_state) {
        let pod = Pod::new(job.owner.uid as u32);
        match pod.image_exists(&job.image_id).await {
            Ok(exists) => {
                let pings_api_key = shared_state.config.pings_api_key.clone();
                if exists {
                    match start_job(shared_state, &pod, &mut job).await {
                        Ok(job) => {
                            println!("Starting Job {}", job.id);
                            send_ping(pings_api_key, job.owner.clone(), JobStarted { id: job.id });
                        }
                        Err(e) => {
                            eprintln!("Error starting container for image {}: {}", job.image_id, e);
                        }
                    }
                } else {
                    println!("Image {} for uid {} not found", job.image_id, job.owner.uid);
                    send_ping(
                        pings_api_key,
                        job.owner,
                        JobCancelled {
                            id: job.id,
                            reason: format!("Cannot find image {}", job.image_id),
                        },
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "Error finding image {} for uid {}: {}",
                    job.image_id, job.owner.uid, e
                );
                send_ping(
                    shared_state.config.pings_api_key.clone(),
                    job.owner,
                    JobCancelled {
                        id: job.id,
                        reason: format!("Error finding image {}: {}", job.image_id, e),
                    },
                );
            }
        }
    }
}

fn kill_rogue_gpu_processes(gpu: &mut GPU, shared_state: &mut SharedState) {
    for device in &shared_state.devices {
        if !device.watching {
            continue;
        }
        if let Some(current_job) = shared_state.device_owner(device.device_number) {
            let processes = gpu.get_processes(device.device_number).unwrap_or(vec![]);
            for p in processes.iter().filter(|p| p.user.uid != 0) {
                if p.user.uid != current_job.owner.uid {
                    gpu.kill_process(p.pid);
                    println!(
                        "Killed process on GPU {}: pid: {}, uid: {} ({})",
                        device.device_number, p.pid, p.user.uid, p.user.name
                    );
                }
            }
        }
    }
}

fn accept_socket(sock: &UnixListener, shared_state: &mut SharedState) {
    match sock.accept() {
        Ok((mut socket, addr)) => {
            socket
                .set_read_timeout(Some(Duration::from_millis(1000)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_millis(1000)))
                .unwrap();
            println!("Accepted connection from {:?}", addr);
            let mut buf = String::with_capacity(1024);
            match socket.read_to_string(&mut buf) {
                Ok(_size) => {
                    let result = match serde_json::from_str::<Command>(&buf) {
                        Ok(command) => sock_communicate(shared_state, command),
                        Err(e) => {
                            let msg = format!("Error parsing command: {:?}", e);
                            eprintln!("{}", msg);
                            Error(msg)
                        }
                    };
                    if let Err(e) = socket.write(
                        serde_json::to_string(&result)
                            .unwrap_or_else(|e| {
                                eprintln!("Error serializing result: {}", e);
                                "".to_string()
                            })
                            .as_bytes(),
                    ) {
                        eprintln!("Error sending response: {}", e);
                    }
                }
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock => eprintln!("Timeout while reading from socket"),
                    _ => eprintln!("Error reading from socket: {:?}", e),
                },
            }
        }
        Err(e) => match e.kind() {
            io::ErrorKind::WouldBlock => {}
            _ => eprintln!("Error accepting socket connection: {:?}", e),
        },
    }
}

#[tokio::main]
async fn main() {
    let config = comms::get_config().unwrap();

    let mut gpu = GPU::new();

    let server_sock = bind("/run/clobberd.sock").unwrap();
    println!("Started socket");
    if let Err(e) = server_sock.set_nonblocking(true) {
        panic!("Error setting socket to non-blocking: {}", e);
    }

    let mut shared_state = SharedState {
        devices: (0..gpu.device_count())
            .map(|n| DeviceState {
                device_number: n,
                watching: true,
            })
            .collect(),
        queued_jobs: VecDeque::from([]),
        active_jobs: vec![],
        config: config,
    };

    println!("Started Server");

    loop {
        accept_socket(&server_sock, &mut shared_state);
        kill_rogue_gpu_processes(&mut gpu, &mut shared_state);
        remove_finished_jobs(&mut shared_state).await;
        try_start_job(&mut shared_state).await;
        thread::sleep(time::Duration::from_millis(250));
    }
}
