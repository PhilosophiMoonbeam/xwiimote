use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const STATIC_LIBRARY: &str = "libxwiimote.a";
const SHARED_OBJECT: &str = "libxwiimote.so";
const VERSION_SCRIPT: &str = "libxwiimote.sym";

pub fn invalidate(root: &Path, profile: &str) -> io::Result<()> {
    let output = profile_dir(root, profile).join(SHARED_OBJECT);
    match fs::remove_file(&output) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!(
                "cannot remove stale shared object {}: {error}",
                output.display()
            ),
        )),
    }
}

pub fn link(root: &Path, profile: &str) -> io::Result<()> {
    if !cfg!(target_os = "linux") {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "linking libxwiimote.so is supported only on Linux",
        ));
    }

    invalidate(root, profile)?;

    let profile_dir = profile_dir(root, profile);
    let static_library = profile_dir.join(STATIC_LIBRARY);
    let version_script = root.join(VERSION_SCRIPT);
    require_file(&static_library, "Rust static library")?;
    require_file(&version_script, "ELF version script")?;

    let output = profile_dir.join(SHARED_OBJECT);
    let temporary = profile_dir.join(format!(".{SHARED_OBJECT}.{}.tmp", std::process::id()));
    remove_temporary(&temporary)?;

    let mut version_script_arg = OsString::from("-Wl,--version-script=");
    version_script_arg.push(version_script.as_os_str());

    let status = Command::new("cc")
        .current_dir(root)
        .arg("-shared")
        .arg("-Wl,--gc-sections")
        .arg("-Wl,--whole-archive")
        .arg(&static_library)
        .arg("-Wl,--no-whole-archive")
        .arg(version_script_arg)
        .arg("-Wl,-soname,libxwiimote.so.2")
        .arg("-Wl,-z,defs")
        .arg("-ludev")
        .arg("-ldl")
        .arg("-lpthread")
        .arg("-lm")
        .arg("-o")
        .arg(&temporary)
        .status()
        .map_err(|error| {
            let _ = fs::remove_file(&temporary);
            io::Error::new(
                error.kind(),
                format!("cannot execute C linker `cc`: {error}"),
            )
        })?;

    if !status.success() {
        let _ = fs::remove_file(&temporary);
        return Err(io::Error::other(format!(
            "C linker `cc` failed while producing {}: {status}",
            output.display()
        )));
    }

    fs::rename(&temporary, &output).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        io::Error::new(
            error.kind(),
            format!(
                "cannot atomically install shared object {}: {error}",
                output.display()
            ),
        )
    })
}

fn profile_dir(root: &Path, profile: &str) -> PathBuf {
    root.join("target").join(profile)
}

fn require_file(path: &Path, description: &str) -> io::Result<()> {
    match path.metadata() {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{description} is not a file: {}", path.display()),
        )),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!(
                "{description} is unavailable at {}: {error}",
                path.display()
            ),
        )),
    }
}

fn remove_temporary(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!(
                "cannot remove stale temporary shared object {}: {error}",
                path.display()
            ),
        )),
    }
}
