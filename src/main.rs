use nvml_wrapper::Nvml;
fn main() {
    println!("Hello, world!");
    let nvml = Nvml::init().unwrap();
    let nvml_device_count = nvml.device_count().unwrap();
    println!("We have {} devices.", nvml_device_count); 
}
