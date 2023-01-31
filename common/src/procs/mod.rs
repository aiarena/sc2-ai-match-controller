pub mod tcp_port;

use std::fs::{File, OpenOptions};
use std::path::PathBuf;

pub fn create_stdout_and_stderr_files(log_file_path: &PathBuf) -> std::io::Result<(File, File)> {
    let stdout_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_file_path)?;

    let stderr_file = stdout_file.try_clone()?;
    Ok((stdout_file, stderr_file))
}
