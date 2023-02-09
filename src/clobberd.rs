use nvml_wrapper::{Nvml,error::NvmlError};
use users::get_user_by_uid;
use sysinfo::{ProcessRefreshKind, Pid, ProcessExt, System, SystemExt, Signal};
use std::{thread, time::Duration};

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

fn get_processes(nvml: &Nvml, system: &mut System, device_index: u32) -> Result<Vec<GPUprocess>, NvmlError>{
    let mut gpu_processes = vec![];
    let device = nvml.device_by_index(device_index).unwrap();
    let nvml_processes = device.running_compute_processes_v2().unwrap();
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
			    name: user.name().to_string_lossy().to_string()
			}
		    },
		    None => User {
			uid: 0,
			name: "Unknown".to_string()
		    }
		}
	    });         
        }
    }
    Ok(gpu_processes)
}

fn kill_process(system: &mut System, pid: usize) -> bool {
    system.process(Pid::from(pid)).map(|process| process.kill_with(Signal::Term)).is_some()
}

fn main() {
    let nvml = Nvml::init().unwrap();
    let mut system = System::new_all();

    let nvml_device_count = nvml.device_count().unwrap();
    let mut locks: Vec<Option<User>> = vec![None; usize::try_from(nvml_device_count).unwrap()];
    
    loop {
	system.refresh_processes_specifics(ProcessRefreshKind::everything());
	system.refresh_users_list();
	for device_number in 0..nvml_device_count {
	    let processes = get_processes(&nvml, &mut system, device_number).unwrap_or(vec![]);
	    if processes.iter().filter(|p| p.user.uid != 0).count() == 0 {
		locks[device_number as usize] = None;
	    } else {
		for p in processes.iter() {
		    match &locks[device_number as usize] {
			None => {
			    locks[device_number as usize] = Some(p.user.clone());
			},
			Some(current) => if current != &p.user {
			    kill_process(&mut system, p.pid);
			}
		    }
		}
	    }
	}
	thread::sleep(Duration::from_millis(250));
    }
}
