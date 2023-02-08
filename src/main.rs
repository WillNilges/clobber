use std::collections::{HashMap, HashSet};
use nvml_wrapper::{Nvml,error::NvmlError};
use users::get_user_by_uid;
use sysinfo::{Pid, ProcessExt, System, SystemExt};
use colored::Colorize;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(long, short, action)]
    summary: bool,
}

pub struct GPUprocess {
    name: String,
    pid: usize,
    device_number: usize,
    uid: usize,
    user: String
}

fn main() -> Result<(), NvmlError> {
    let args = Args::parse();
    let nvml = Nvml::init()?;
    let s = System::new_all();

    let running_gpu_processes = get_processes(&nvml, s)?;
    if args.summary {
        print_banner_summary(&nvml, &running_gpu_processes);
    } else {
        print_usage(&running_gpu_processes);
    }
    print_warnings(&running_gpu_processes);
    Ok(())
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

fn print_device_count(nvml: &Nvml) {
    let nvml_device_count = nvml.device_count().unwrap();
    println!("Found {} devices.", nvml_device_count);
}

fn print_banner_summary(nvml: &Nvml, processes: &Vec<GPUprocess>) {
    println!("== CLOBBER ==");
    print_device_count(&nvml);
    for proc in processes {
        println!(
            "Found process \"{}\" ({}) on GPU {} started by {}!", 
            proc.name, proc.pid, proc.device_number, proc.user.red()
        );
    }
    
    if processes.len() == 0 {
        println!("{}", "There are no running GPU processes.".green());
    }
}

fn print_usage(processes: &Vec<GPUprocess>) {
    let mut users = HashMap::new();
    for e in processes {
        users.entry(e.user.to_string()).or_insert(vec!()).push(&e.device_number);
    }

    for (user, mut gpus) in users {
        let set: HashSet<_> = gpus.drain(..).collect(); // dedup
        gpus.extend(set.into_iter());

        let mut gpu_string = format!("{:?}", gpus);
        gpu_string = gpu_string.trim_start_matches('[').trim_end_matches(']').to_string();
        println!("{} {} {}", user.yellow().bold(), "is currently using GPUs".yellow(), gpu_string.yellow().bold());
    }

    if processes.len() == 0 {
        println!("{}", "There are no running GPU processes.".green());
    }
}

// Look through the list of processes, find processes
// that are running on the same GPU, note the names.
fn print_warnings(processes: &Vec<GPUprocess>) -> bool {
    let mut warned = false;
    // List of GPUs that have multiple processes running on them
    // We can count on processes being sorted, since we go through the GPU IDs
    // sequentially
    let mut mult_proc = HashMap::new();
    for e in processes {
        mult_proc.entry(e.device_number).or_insert(vec!()).push(&e.user);
    }

    for (gpu_num, mut names) in mult_proc {
        if names.len() > 1 {
            println!(
                    "{} {}",
                    "WARNING! MULTIPLE PROCESSES DETECTED ON GPU".red().bold(), gpu_num.to_string().red().bold()
                );

            // Delete duplicate names in case someone has multiple processes running
            let mut uniques = HashSet::new();
            names.retain(|e| uniques.insert(*e));

            println!("{}", "PLEASE CONTACT THE FOLLOWING USERS TO COORDINATE WORKLOADS:".red());
            for user in names {
                println!("- {}", user.red().bold());
            }
            warned = true; // If this does something, then don't run show_usage
        }
    }
    warned
}
