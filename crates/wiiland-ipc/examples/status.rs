use std::env;
use std::path::PathBuf;
use std::process;

use wiiland_ipc::Client;

fn main() {
    if let Err(error) = run() {
        eprintln!("status: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let socket = match args.next() {
        None => None,
        Some(option) if option == "--socket" => {
            let path = args
                .next()
                .ok_or_else(|| "--socket requires a path".to_owned())?;
            if args.next().is_some() {
                return Err("usage: status [--socket PATH]".to_owned());
            }
            Some(PathBuf::from(path))
        }
        Some(_) => return Err("usage: status [--socket PATH]".to_owned()),
    };

    let mut client = match socket {
        Some(path) => Client::connect(path),
        None => Client::connect_default(),
    }
    .map_err(|error| error.to_string())?;
    let status = client.status().map_err(|error| error.to_string())?;

    println!("daemon_version={}", status.daemon_version);
    println!("pid={}", status.pid);
    println!("device_count={}", status.device_count);
    println!("dry_run={}", status.dry_run);
    println!("socket_path={}", status.socket_path);
    Ok(())
}
