use nvml_wrapper::{error::NvmlError, Nvml};
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::{thread, time::Duration};
use sysinfo::{Pid, ProcessExt, ProcessRefreshKind, Signal, System, SystemExt};
use users::get_user_by_uid;

pub struct GPUprocess {
    name: String,
    pid: usize,
    device_number: usize,
    user: User,
}

#[derive(Clone, Eq)]
pub struct User {
    name: String,
    uid: usize,
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

fn sock_communicate(socket: &mut UnixStream) {
    let mut buf = String::with_capacity(1024);
    match socket.read_to_string(&mut buf) {
        Ok(_size) => {}
        Err(e) => {
            println!("Error reading from socket: {:?}", e);
        }
    }
}

fn bind(path: impl AsRef<Path>) -> std::io::Result<UnixListener> {
    let path = path.as_ref();
    let _ = std::fs::remove_file(path);
    let ret = UnixListener::bind(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path.to_str().unwrap(),
            std::fs::Permissions::from_mode(0o772),
        )?;
    }
    ret
}

fn sock_listen() {
    let server_sock = bind("/run/clobberd.sock").unwrap();
    println!("Started socket server");

    loop {
        match server_sock.accept() {
            Ok((mut socket, addr)) => {
                println!("Accepted connection from {:?}", addr);
                thread::spawn(move || sock_communicate(&mut socket));
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

    loop {
        system.refresh_processes_specifics(ProcessRefreshKind::everything());
        system.refresh_users_list();
        for device_number in 0..locks.len() as u32 {
            let processes = get_processes(&nvml, &mut system, device_number).unwrap_or(vec![]);
            if processes.iter().filter(|p| p.user.uid != 0).count() == 0 {
                locks[device_number as usize] = None;
            } else {
                for p in processes.iter() {
                    match &locks[device_number as usize] {
                        None => {
                            locks[device_number as usize] = Some(p.user.clone());
                        }
                        Some(current) => {
                            if current != &p.user {
                                kill_process(&mut system, p.pid);
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
    thread::spawn(sock_listen);
    gpu_watch();
}
