use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;

use xwiidump::{close_eeprom, dump, error_description, open_eeprom, usage};

fn main() {
    process::exit(run(env::args_os()));
}

fn run<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    let program = args
        .first()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("xwiidump"));

    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        let mut stdout = io::stdout().lock();
        let _ = usage(&program, &mut stdout);
        return 0;
    }

    if args.len() != 2 || args[1].as_os_str().is_empty() {
        let mut stderr = io::stderr().lock();
        let _ = usage(&program, &mut stderr);
        return 1;
    }

    let path = PathBuf::from(args[1].clone());
    let file_name = path.to_string_lossy().into_owned();
    let mut file = match open_eeprom(&path) {
        Ok(file) => file,
        Err(error) => {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(
                stderr,
                "Cannot open eeprom file '{file_name}': {}",
                error_description(&error)
            );
            return 1;
        }
    };

    let dump_result = {
        let mut stdout = io::stdout().lock();
        let mut stderr = io::stderr().lock();
        dump(&mut file, &mut stdout, &mut stderr, &file_name)
    };

    let close_result = close_eeprom(file);
    if let Err(error) = close_result {
        let mut stderr = io::stderr().lock();
        let _ = writeln!(
            stderr,
            "Cannot close eeprom file '{file_name}': {}",
            error_description(&error)
        );
        return 1;
    }

    match dump_result {
        Ok(true) => 0,
        Ok(false) | Err(_) => 1,
    }
}
