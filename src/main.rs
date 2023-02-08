use std::{collections::{HashMap, HashSet}, process::{Command, Stdio}};
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

fn write_to_user(user: String, message: String) {
    let echo_child = Command::new("echo")
        .arg(message)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start echo process while writing to user");

    let echo_out = echo_child.stdout.expect("Failed to open echo stdout");

    let write_child = Command::new("write")
        .arg(user)
        .stdin(Stdio::from(echo_out))
        .spawn()
        .expect("Failed to start wall process while writing to user");

    let _output = write_child.wait_with_output();
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
        let mut write_message: String;
        if names.len() > 1 {
            write_message = format!(
                    "{} {}",
                    "WARNING! MULTIPLE PROCESSES DETECTED ON GPU".red().bold(), gpu_num.to_string().red().bold()
                );

            // Delete duplicate names in case someone has multiple processes running
            let mut uniques = HashSet::new();
            names.retain(|e| uniques.insert(*e));

            write_message += &format!("{}", "\nPLEASE CONTACT THE FOLLOWING USERS TO COORDINATE WORKLOADS:".red()).to_string();
            for user in &names {
                write_message += &format!("\n- {}", user.red().bold());
            }

            println!("{}", write_message);
            
            let names_string = &format!("{:?}", names).trim_start_matches('[').trim_end_matches(']').to_string();
            for user in &names {
                write_to_user(user.to_string(), format!("WARNING! MULTIPLE PROCESSES DETECTED ON GPU {}\nPLEASE CONTACT THE FOLLOWING USERS TO COORDINATE WORKLOADS:\n{}", gpu_num.to_string(), names_string));
            }
            warned = true; // If this does something, then don't run show_usage
        }
    }
    warned
}
