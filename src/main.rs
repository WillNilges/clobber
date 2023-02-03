use nvml_wrapper::Nvml;

use users::get_user_by_uid;
use sysinfo::{Pid, ProcessExt, System, SystemExt};
use colored::Colorize;

fn main() {
    println!("== CLOBBERING STATE mACHINE HOUSE ==");
    let nvml = Nvml::init().unwrap();
    let nvml_device_count = nvml.device_count().unwrap();
    println!("Found {} devices.", nvml_device_count); 

    let mut s = System::new_all();
    s.refresh_users_list();
    for device_number in 0..nvml_device_count {
        let device = nvml.device_by_index(device_number).unwrap();
        let nvml_processes = device.running_compute_processes_v2().unwrap();
        //println!("Processes: {:?}", nvml_processes);

        for proc in nvml_processes {
            let gpu_process = proc.pid;
            if let Some(process) = s.process(Pid::from(gpu_process as usize)) {
                print!("Found Process \"{}\" ({}) on GPU {} started by ", process.name().red(), gpu_process, device_number);
                if let Some(user_id) = process.user_id() {

                    let user = get_user_by_uid(**user_id).unwrap();
                    println!("{}!", user.name().to_string_lossy().red());


                } else {
                    println!("Unknown.");
                }
            }
        }
    }
}
