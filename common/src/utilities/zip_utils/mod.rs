use bytes::Bytes;
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ZipStruct {
    pub is_dir: bool,
    pub target_dir: String,
    pub target_name: String,
    pub path: PathBuf,
}

pub fn zip_directory_to_path(archive_file_path: &Path, directory: &Path) -> io::Result<()> {
    let archive_file = File::create(archive_file_path)?;
    zip_directory(archive_file, directory)
}

pub fn zip_directory<W: Write + io::Seek>(file: W, directory: &Path) -> io::Result<()> {
    let dir = directory.join("*").to_string_lossy().to_string();
    let process = Command::new("7z")
        .arg("a")
        .arg(".zip")
        .arg("-so")
        .arg(dir)
        .output()?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&process.stdout)?;
    Ok(())
}

/// Extracts a ZIP file from memory to the given directory.
pub fn zip_extract_from_bytes(archive_file: &Bytes, target_dir: &Path) -> anyhow::Result<()> {
    let mut tmp_file = tempfile::NamedTempFile::new().unwrap();
    tmp_file.write_all(archive_file).unwrap();

    let new_file = tmp_file.into_temp_path();
    let path = new_file.to_string_lossy().to_string();

    let process = Command::new("7z")
        .arg("x")
        .arg(&path)
        .arg(format!("-o{}", target_dir.to_string_lossy()))
        .arg("-r")
        .arg("-tzip")
        .output()?;

    let exit_status = process.status;

    if exit_status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8(process.stderr)?;
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

    #[test]
    fn test_zip_file_size_is_smaller() {
        let zip_file = include_bytes!("../../../../testing/unittests/test_zip.zip");
        let temp_dir = tempfile::TempDir::new()
            .expect("Could not create tmp directory")
            .into_path();
        let zip_bytes = bytes::Bytes::from_static(zip_file);
        zip_extract_from_bytes(&zip_bytes, &temp_dir).expect("Could not extract archive");
        let dir_size = fs_extra::dir::get_size(&temp_dir).expect("Could not get size of directory");

        let mut tmp_file = tempfile::tempfile().expect("Could not create tempfile");
        zip_directory(&mut tmp_file, &temp_dir).expect("Could not zip file");
        let zipped_archive_size = tmp_file
            .metadata()
            .expect("Could not read tmp file metadata")
            .len();
        assert!(zipped_archive_size < dir_size)
    }
}
