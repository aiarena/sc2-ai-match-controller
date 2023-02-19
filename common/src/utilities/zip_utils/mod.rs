use anyhow::anyhow;
use bytes::Bytes;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, trace};

#[derive(Debug, Clone)]
pub struct ZipStruct {
    pub is_dir: bool,
    pub target_dir: String,
    pub target_name: String,
    pub path: PathBuf,
}

pub fn zip_directory_to_path(archive_file_path: &Path, directory: &Path) -> anyhow::Result<()> {
    trace!(
        "Zipping directory {:?} to file {:?}",
        &directory,
        &archive_file_path
    );
    zip_directory(archive_file_path, directory)
}

pub fn zip_directory(file: &Path, directory: &Path) -> anyhow::Result<()> {
    let file = file.to_string_lossy().to_string();
    let dir = directory.join("*").to_string_lossy().to_string();
    let mut command = Command::new("7z");
    command.arg("a").arg(file).arg(dir).arg("-y");
    trace!("Zipping archive with args: {:?}", command.get_args());
    trace!("{:?}", command);
    let process = command.output()?;
    // let mut writer = BufWriter::new(file);
    // writer.write_all(&process.stdout)?;

    return if process.status.success() {
        Ok(())
    } else {
        let str = String::from_utf8(process.stderr);

        trace!("{:?}", str);
        Err(anyhow!("Error while zipping archive: {:?}", str))
    };
}

/// Extracts a ZIP file from memory to the given directory.
pub fn zip_extract_from_bytes(archive_file: &Bytes, target_dir: &Path) -> anyhow::Result<()> {
    let mut tmp_file = tempfile::NamedTempFile::new().unwrap();
    tmp_file.write_all(archive_file).unwrap();

    let new_file = tmp_file.into_temp_path();
    let path = new_file.to_string_lossy().to_string();

    let mut command = Command::new("7z");

    command
        .arg("x")
        .arg(&path)
        .arg(format!("-o{}", target_dir.to_string_lossy()))
        .arg("-r")
        .arg("-tzip")
        .arg("-y");
    debug!("{:?}", command);
    let process = command.output()?;

    let exit_status = process.status;

    if exit_status.success() {
        Ok(())
    } else {
        let msg = format!(
            "{exit_status:?}-Err:{}\nOut:{}",
            String::from_utf8(process.stderr)?,
            String::from_utf8(process.stdout)?
        );
        Err(anyhow::Error::msg(msg))
    }
}

pub fn test_archive(path: &Path) -> anyhow::Result<()> {
    let process = Command::new("7z")
        .arg("t")
        .arg(&path.to_string_lossy().to_string())
        .arg("-r")
        .output()?;

    if process.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8(process.stderr);
        Err(anyhow::Error::msg(msg?))
    }
}

#[cfg(test)]
mod tests {
    use super::{zip_directory, zip_extract_from_bytes};
    use std::fs::File;

    #[test]
    fn test_zip_file_size_is_smaller() {
        let zip_file = include_bytes!("../../../../testing/unittests/test_zip.zip");
        let temp_dir = tempfile::TempDir::new()
            .expect("Could not create tmp directory")
            .into_path();
        let zip_bytes = bytes::Bytes::from_static(zip_file);
        zip_extract_from_bytes(&zip_bytes, &temp_dir).expect("Could not extract archive");
        let dir_size = fs_extra::dir::get_size(&temp_dir).expect("Could not get size of directory");

        let tmp_dir = tempfile::tempdir().expect("Could not create temp directory");

        let path = tmp_dir.path().join("test.zip");
        zip_directory(&path, &temp_dir).expect("Could not zip file");
        let file = File::open(path).expect("Could not open file");
        let zipped_archive_size = file
            .metadata()
            .expect("Could not read tmp file metadata")
            .len();
        assert!(zipped_archive_size < dir_size)
    }
}
