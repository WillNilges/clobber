use comms::Command::*;
use comms::Response::*;
use comms::{Command, Response, User};
use nvml_wrapper::{error::NvmlError, Nvml};
use std::io::prelude::*;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::{thread, time::Duration};
use sysinfo::{Pid, ProcessExt, ProcessRefreshKind, Signal, System, SystemExt};
use users::get_user_by_uid;

mod comms;

pub struct GPUprocess {
    name: String,
    pid: usize,
    device_number: usize,
    user: User,
}

impl PartialEq for User {
    fn eq(&self, other: &User) -> bool {
        self.uid == other.uid
    }
}

fn get_processes(
    nvml: &Nvml,
    system: &mut System,
    device_index: u32,
) -> Result<Vec<GPUprocess>, NvmlError> {
    let mut gpu_processes = vec![];
    let device = nvml.device_by_index(device_index).unwrap();
    let nvml_processes = device.running_graphics_processes_v2().unwrap();
    for proc in nvml_processes {
        if let Some(process) = system.process(Pid::from(proc.pid as usize)) {
            gpu_processes.push(GPUprocess {
                name: process.name().to_string(),
                pid: proc.pid as usize,
                device_number: device_index as usize,
                user: match process.user_id() {
                    Some(user_id) => {
                        let user = get_user_by_uid(**user_id).unwrap();
                        User {
                            uid: **user_id as usize,
                            name: user.name().to_string_lossy().to_string(),
                        }
                    }
                    None => User {
                        uid: 0,
                        name: "Unknown".to_string(),
                    },
                },
            });
        }
    }
    Ok(gpu_processes)
}

fn kill_process(system: &mut System, pid: usize) -> bool {
    system
        .process(Pid::from(pid))
        .map(|process| process.kill_with(Signal::Term))
        .is_some()
}

fn sock_communicate(command: Command) -> Response {
    match command {
        Status => GPUStatus { locks: vec![] },
        _ => {
            println!("Admin command received on public socket.");
            Error("Operation not permitted, must be root.".to_string())
        }
    }
}

fn sock_admin_communicate(command: Command) -> Response {
    match command {
        Kill => {
            println!("Received Kill command. Exiting.");
            std::process::exit(0);
        }
        _ => sock_communicate(command),
    }
}

fn bind(path: impl AsRef<Path>, public: bool) -> std::io::Result<UnixListener> {
    let path = path.as_ref();
    let _ = std::fs::remove_file(path);
    let ret = UnixListener::bind(path);
    #[cfg(unix)]
    if public {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path.to_str().unwrap(),
            std::fs::Permissions::from_mode(0o772),
        )?;
    }
    ret
}

fn sock_listen(admin: bool) {
    let server_sock = bind(
        if admin {
            "/run/clobberd-admin.sock"
        } else {
            "/run/clobberd.sock"
        },
        !admin,
    )
    .unwrap();

    println!(
        "Started socket {} server",
        if admin { "admin" } else { "public" }
    );

    loop {
        match server_sock.accept() {
            Ok((mut socket, addr)) => {
                println!("Accepted connection from {:?}", addr);
                thread::spawn(move || {
                    let mut buf = String::with_capacity(1024);
                    match socket.read_to_string(&mut buf) {
                        Ok(_size) => {
                            let command = match serde_json::from_str::<Command>(&buf) {
                                Ok(command) => command,
                                Err(e) => panic!("Error parsing command: {:?}", e),
                            };
                            let result = if admin {
                                sock_admin_communicate(command)
                            } else {
                                sock_communicate(command)
                            };
                            if let Err(e) = socket.write(
                                serde_json::to_string(&result)
                                    .unwrap_or("".to_string())
                                    .as_bytes(),
                            ) {
                                println!("Error sending response: {}", e);
                            }
                        }
                        Err(e) => {
                            println!("Error reading from socket: {:?}", e);
                        }
                    }
                });
            }
            Err(e) => {
                println!("Error accepting socket connection: {:?}", e);
            }
        }
    }
}

fn gpu_watch() {
    let mut system = System::new_all();
    let nvml = Nvml::init().unwrap();

    let nvml_device_count = nvml.device_count().unwrap();
    let mut locks: Vec<Option<User>> = vec![None; nvml_device_count as usize];

    println!("Started GPU watch");

    loop {
        system.refresh_processes_specifics(ProcessRefreshKind::everything());
        system.refresh_users_list();
        for device_number in 0..locks.len() as u32 {
            let processes = get_processes(&nvml, &mut system, device_number).unwrap_or(vec![]);
            if processes.iter().filter(|p| p.user.uid != 0).count() == 0 {
                if locks[device_number as usize].is_some() {
                    println!("Released lock on GPU {}", device_number);
                }
                locks[device_number as usize] = None;
            } else {
                for p in processes.iter() {
                    match &locks[device_number as usize] {
                        None => {
                            locks[device_number as usize] = Some(p.user.clone());
                            println!(
                                "User {} ({}) acquired lock on GPU {}",
                                p.user.uid, p.user.name, device_number
                            );
                        }
                        Some(current) => {
                            if current != &p.user {
                                kill_process(&mut system, p.pid);
                                println!(
                                    "Killed process on GPU {}: pid: {}, uid: {} ({})",
                                    device_number, p.pid, p.user.uid, p.user.name
                                );
                            }
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn main() {
    thread::spawn(|| sock_listen(false));
    thread::spawn(|| sock_listen(true));
    gpu_watch();
}
