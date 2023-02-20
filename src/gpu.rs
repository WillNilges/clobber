use crate::comms::User;
use nvml_wrapper::{error::NvmlError, Nvml};
use sysinfo::{Pid, ProcessExt, ProcessRefreshKind, Signal, System, SystemExt};
use users::get_user_by_uid;

pub struct GPUprocess {
    pub name: String,
    pub pid: usize,
    pub device_number: usize,
    pub user: User,
}

pub struct GPU {
    nvml: Nvml,
    system: System,
}

impl GPU {
    pub fn new() -> GPU {
        GPU {
            nvml: Nvml::init().unwrap(),
            system: System::new_all(),
        }
    }

    pub fn device_count(&self) -> u32 {
        self.nvml.device_count().unwrap()
    }

    pub fn kill_process(&self, pid: usize) -> bool {
        self.system
            .process(Pid::from(pid))
            .map(|process| process.kill_with(Signal::Term))
            .is_some()
    }

    pub fn get_processes(&mut self, device_index: u32) -> Result<Vec<GPUprocess>, NvmlError> {
        self.system
            .refresh_processes_specifics(ProcessRefreshKind::everything());
        self.system.refresh_users_list();
        let mut gpu_processes = vec![];
        let device = self.nvml.device_by_index(device_index).unwrap();
        let nvml_processes = device.running_compute_processes_v2().unwrap();
        for proc in nvml_processes {
            if let Some(process) = self.system.process(Pid::from(proc.pid as usize)) {
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
}
