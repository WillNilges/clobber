use clap::{Parser, Subcommand};
use colored::Colorize;
use pod::*;
use std::os::unix::net::UnixStream;

mod comms;
mod pod;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Status,
    Kill,
    Watch {
        #[arg(short, long)]
        device: u8,
    },
    Unwatch {
        #[arg(short, long)]
        device: u8,
    },
    Queue {
        #[arg(short, long)]
        image: String,
    },
}

#[link(name = "c")]
extern "C" {
    fn getuid() -> u32;
}

fn print_response(response: comms::Response) {
    use comms::Response::*;
    match response {
        Success => {}
        Error(e) => eprintln!("Error: {}", e),
        GPUStatus { locks } => {
            for (index, gpu) in locks.iter().enumerate() {
                if let Some(user) = gpu {
                    println!(
                        "{} {} {} {}",
                        "GPU".yellow(),
                        index.to_string().yellow().bold(),
                        "is being used by".red(),
                        user.name.yellow().bold()
                    );
                } else {
                    println!(
                        "{} {} {}",
                        "GPU".yellow(),
                        index.to_string().yellow().bold(),
                        "is not being used".green()
                    );
                }
            }
        }
        ActiveJobs { jobs } => {}
    }
}

fn send_command(command: comms::Command) {
    use std::io::{Read, Write};
    let mut sock = match UnixStream::connect("/run/clobberd.sock") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error connecting to socket. Is clobberd running?\n{}", e);
            return;
        }
    };

    let json = match serde_json::to_string(&command) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Error serializing to JSON? somehow: {}", e);
            return;
        }
    };

    if let Err(e) = sock.write(json.as_bytes()) {
        eprintln!("Error writing to socket: {}", e);
    }

    sock.flush().unwrap();
    sock.shutdown(std::net::Shutdown::Write).unwrap();

    let mut buf = String::with_capacity(10 * 1024);
    match sock.read_to_string(&mut buf) {
        Ok(_size) => match serde_json::from_str::<comms::Response>(&buf) {
            Ok(response) => print_response(response),
            Err(e) => eprintln!("Error parsing response: {}", e),
        },
        Err(e) => {
            eprintln!("Error reading from socket: {}", e);
        }
    }
}

async fn find_image(uid: u32, image: String) -> Result<Option<String>, String> {
    let pod = Pod::new(uid);
    if let Err(e) = pod.ping().await {
        eprintln!(
            "Error connecting to podman: {}\n\nMaybe try {}",
            e,
            "systemctl enable --user --now podman.socket".green().bold()
        );
        return Err("".to_string());
    }
    pod.image_exists(image.clone())
        .await
        .map(|e| if e { Some(image) } else { None })
        .map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let (uid, is_root) = unsafe { (getuid(), getuid() == 0) };

    let username = "ethanf108";

    if let Some(command) = args.command {
        use Commands::*;
        match command {
            Status => send_command(comms::Command::Status),
            Kill => {
                if !is_root {
                    eprintln!("Permission denied.");
                    return;
                }
                send_command(comms::Command::Kill);
            }
            Watch { device } => {
                if !is_root {
                    eprintln!("Permission denied.");
                    return;
                }
                send_command(comms::Command::SetWatch {
                    device_number: device as u32,
                    watching: true,
                });
            }
            Unwatch { device } => {
                if !is_root {
                    eprintln!("Permission denied.");
                    return;
                }
                send_command(comms::Command::SetWatch {
                    device_number: device as u32,
                    watching: false,
                });
            }
            Queue { image } => match find_image(uid, image).await {
                Ok(Some(image)) => send_command(comms::Command::QueueJob {
                    user: comms::User {
                        uid: uid as usize,
                        name: username.to_string(),
                    },
                    image_id: image,
                    gpus: vec![],
                }),

                Ok(None) => eprintln!("Cannot find image"),
                Err(e) => eprintln!("Error finding image: {}", e),
            },
        }
    }
}
