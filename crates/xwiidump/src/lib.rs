//! Reader and formatter for Wii Remote EEPROM dumps.
//!
//! The output format intentionally follows the historical `xwiidump` utility:
//! eight bytes are emitted per line, while an EOF marker is emitted without a
//! trailing newline.  The generic dump function keeps the byte stream logic
//! independently testable from the command-line process.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{IntoRawFd, RawFd};
use std::path::Path;

/// Number of bytes in one displayed EEPROM record.
pub const RECORD_SIZE: usize = 8;

/// Write the command's two-line usage text.
pub fn usage<W: Write>(program: &str, stream: &mut W) -> io::Result<()> {
    writeln!(stream, "Usage: {program} FILE")?;
    writeln!(
        stream,
        "Read a Wii Remote EEPROM file and write its contents to stdout."
    )
}

/// Read from `reader`, retrying interruptions exposed by the `Read` trait.
///
/// `std::fs::File` normally retries `EINTR` internally, but this explicit loop
/// also covers readers used by tests and any future reader implementation.
pub fn read_retry<R: Read>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

/// Return an errno description in the form used by `strerror(3)`.
///
/// Rust's `io::Error` display adds ` (os error N)` to platform errors, while
/// the historical utility prints only the libc description.
pub fn error_description(error: &io::Error) -> String {
    let text = error.to_string();
    let Some(errno) = error.raw_os_error() else {
        return text;
    };
    let suffix = format!(" (os error {errno})");
    text.strip_suffix(&suffix)
        .map_or_else(|| text.clone(), str::to_owned)
}

/// Dump an EEPROM stream to the historical xwiidump text format.
///
/// The returned boolean is `true` for a complete stream (including an empty
/// stream) and `false` for a partial record or read failure.  Diagnostics are
/// written to `stderr`; failures writing either output stream are returned as
/// `io::Error` because no faithful output status can be established then.
pub fn dump<R: Read, O: Write, E: Write>(
    reader: &mut R,
    stdout: &mut O,
    stderr: &mut E,
    file: &str,
) -> io::Result<bool> {
    let mut offset = 0usize;
    let mut byte = [0u8; 1];

    loop {
        // The original utility uses `%zu` here (decimal), retaining that
        // detail rather than formatting this as a hexadecimal offset.
        write!(stdout, "0x{offset:08}:")?;

        for index in 0..RECORD_SIZE {
            let error = match read_retry(reader, &mut byte) {
                Ok(1) => {
                    write!(stdout, " 0x{:02x}", byte[0])?;
                    offset += 1;
                    continue;
                }
                Ok(0) => {
                    write!(stdout, " (eof)")?;
                    if index != 0 {
                        writeln!(
                            stderr,
                            "Unexpected end of eeprom file '{file}' at offset 0x{offset:08x}"
                        )?;
                        return Ok(false);
                    }
                    return Ok(true);
                }
                Err(error) => error,
                Ok(_) => io::Error::from_raw_os_error(libc::EIO),
            };

            let errno = error.raw_os_error().unwrap_or(libc::EIO);
            write!(stdout, " (read error {errno})")?;
            writeln!(
                stderr,
                "Cannot read eeprom file '{file}' at offset 0x{offset:08x}: {}",
                error_description(&error)
            )?;
            return Ok(false);
        }

        writeln!(stdout)?;
    }
}

/// Open an EEPROM path for reading, retrying interruptions where exposed.
pub fn open_eeprom(path: &Path) -> io::Result<File> {
    loop {
        match File::open(path) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}

/// Close an EEPROM file and preserve close errors for the CLI diagnostic.
///
/// `File`'s destructor cannot report `close(2)` failures, so ownership is
/// transferred to the raw descriptor and closed explicitly on Linux.
pub fn close_eeprom(file: File) -> io::Result<()> {
    let descriptor: RawFd = file.into_raw_fd();
    // SAFETY: `descriptor` was just transferred from a live `File`, and this
    // function is the sole owner responsible for closing it.
    let result = unsafe { libc::close(descriptor) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::{dump, read_retry};
    use std::io::{self, Cursor, ErrorKind, Read};

    #[test]
    fn empty_stream_is_success_without_newline() {
        let mut input = Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let mut errors = Vec::new();

        assert!(dump(&mut input, &mut output, &mut errors, "empty").unwrap());
        assert_eq!(output, b"0x00000000: (eof)");
        assert!(errors.is_empty());
    }

    #[test]
    fn partial_stream_reports_byte_offset_without_newline() {
        let mut input = Cursor::new(vec![0x10, 0x20, 0xff]);
        let mut output = Vec::new();
        let mut errors = Vec::new();

        assert!(!dump(&mut input, &mut output, &mut errors, "partial").unwrap());
        assert_eq!(output, b"0x00000000: 0x10 0x20 0xff (eof)");
        assert_eq!(
            errors,
            b"Unexpected end of eeprom file 'partial' at offset 0x00000003\n"
        );
    }

    #[test]
    fn complete_record_then_boundary_eof() {
        let mut input = Cursor::new(vec![0, 1, 2, 0x7f, 0x80, 0xfe, 0xff, 0x55]);
        let mut output = Vec::new();
        let mut errors = Vec::new();

        assert!(dump(&mut input, &mut output, &mut errors, "complete").unwrap());
        assert_eq!(
            output,
            b"0x00000000: 0x00 0x01 0x02 0x7f 0x80 0xfe 0xff 0x55\n0x00000008: (eof)"
        );
        assert!(errors.is_empty());
    }

    struct InterruptOnce<R> {
        inner: R,
        interrupted: bool,
    }

    impl<R: Read> Read for InterruptOnce<R> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::Error::from(ErrorKind::Interrupted));
            }
            self.inner.read(buffer)
        }
    }

    #[test]
    fn read_retry_retries_interrupted_reader() {
        let mut reader = InterruptOnce {
            inner: Cursor::new(vec![0xab]),
            interrupted: false,
        };
        let mut byte = [0u8; 1];
        assert_eq!(read_retry(&mut reader, &mut byte).unwrap(), 1);
        assert_eq!(byte, [0xab]);
    }

    struct FailsAfterOne {
        first: bool,
    }

    impl Read for FailsAfterOne {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.first {
                self.first = false;
                buffer[0] = 0x7f;
                Ok(1)
            } else {
                Err(io::Error::from_raw_os_error(libc::EIO))
            }
        }
    }

    #[test]
    fn read_error_reports_errno_and_offset() {
        let mut input = FailsAfterOne { first: true };
        let mut output = Vec::new();
        let mut errors = Vec::new();

        assert!(!dump(&mut input, &mut output, &mut errors, "broken").unwrap());
        assert_eq!(output, b"0x00000000: 0x7f (read error 5)");
        assert_eq!(
            errors,
            b"Cannot read eeprom file 'broken' at offset 0x00000001: Input/output error\n"
        );
    }

    #[test]
    fn record_offset_uses_decimal_digits() {
        let mut input = Cursor::new(vec![0; 16]);
        let mut output = Vec::new();
        let mut errors = Vec::new();

        assert!(dump(&mut input, &mut output, &mut errors, "sixteen").unwrap());
        assert!(output.starts_with(b"0x00000000:"));
        assert!(output.windows(11).any(|window| window == b"0x00000016:"));
    }
}
