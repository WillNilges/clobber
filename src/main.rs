use std::collections::HashMap;
use nvml_wrapper::{Nvml,error::NvmlError};
use users::get_user_by_uid;
use sysinfo::{Pid, ProcessExt, System, SystemExt};
use colored::Colorize;

fn main() -> Result<(), NvmlError> {
    let nvml = Nvml::init()?;
    let s = System::new_all();

    println!("== CLOBBER ==");
    print_device_count(&nvml);

    let running_gpu_processes = get_processes(&nvml, s)?;
    banner_summary(&running_gpu_processes);
    who_is_using_what(&running_gpu_processes);
    print_warnings(&running_gpu_processes);
    Ok(())
}

pub struct GPUprocess {
    name: String,
    pid: usize,
    device_number: usize,
    uid: usize,
    user: String
}

fn get_processes(nvml: &Nvml, mut system: System) -> Result<Vec<GPUprocess>, NvmlError>{
    let nvml_device_count = nvml.device_count().unwrap();
    system.refresh_users_list();
    let mut gpu_processes = vec![];
    for device_number in 0..nvml_device_count {
        let device = nvml.device_by_index(device_number).unwrap();
        let nvml_processes = device.running_compute_processes_v2().unwrap();
        for proc in nvml_processes {
            let gpu_process = proc.pid;
            if let Some(process) = system.process(Pid::from(gpu_process as usize)) {
                let mut gpu_process = GPUprocess {
                    name: process.name().to_string(),
                    pid: gpu_process as usize,
                    device_number: device_number as usize,
                    uid: 0,
                    user: "Unknown".to_string()
                };

                // Sometimes, it's not a sure bet that a UID will be found. So we have to handle
                // that.
                if let Some(user_id) = process.user_id() {
                    let user = get_user_by_uid(**user_id).unwrap();

                    gpu_process.uid = **user_id as usize;
                    gpu_process.user = user.name().to_string_lossy().to_string();
                }
                gpu_processes.push(gpu_process); 
            }
        }
    }
    Ok(gpu_processes)
}

fn banner_summary(processes: &Vec<GPUprocess>) {
    for proc in processes {
        println!(
                    "Found process \"{}\" ({}) on GPU {} started by {}!", 
                    proc.name, proc.pid, proc.device_number, proc.user.red()
                );
    }
}

fn who_is_using_what(processes: &Vec<GPUprocess>) {
    for proc in processes {
        println!(
            "{} {} {}.", 
             proc.user.yellow().bold(), 
             "is currently using GPU".yellow(), 
             proc.device_number.to_string().yellow().bold()
         );
    }
}

fn print_warnings(processes: &Vec<GPUprocess>) {
    let mut gpus = vec![];
    for proc in processes {
       if gpus.contains(&proc.device_number) {
            println!(
                "{} {}",
                "WARNING! MULTIPLE PROCESSES DETECTED ON GPU".red().bold(), (proc.device_number.to_string()).red().bold()
            );
       }
       gpus.push(proc.device_number);
    }

    let mut map = HashMap::new();
    for e in processes {
        map.entry(e.device_number).or_insert(vec!()).push(e);
    }
    println!("{}", "PLEASE CONTACT THE FOLLOWING USERS TO COORDINATE WORKLOADS:".red());
    for (size, item) in map {
        if size > 1 { 
            println!("- {}", item[0].user.red().bold());
        }
    }

    //println!("PLEASE CONTACT THE FOLLOWING PEOPLE TO COORDINATE YOUR WORKLOAD: {:?}", map);
}

fn print_device_count(nvml: &Nvml) {
    let nvml_device_count = nvml.device_count().unwrap();
    println!("Found {} devices.", nvml_device_count);
}
