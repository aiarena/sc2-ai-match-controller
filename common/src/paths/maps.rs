//! Map file finder

use std::{fs, io};

use crate::paths::map_dir;

/// Find a map file, returning its relative path to the sc2 map directory
pub fn find_map(name: &str) -> std::io::Result<String> {
    let mut name = name.replace(' ', "");
    if !name.ends_with(".SC2Map") {
        name.push_str(".SC2Map");
    }

    let mapdir = map_dir();
    for outer in fs::read_dir(&mapdir)? {
        let outer_path = outer.unwrap().path();
        if !outer_path.is_dir() {
            let current = outer_path.file_name().unwrap().to_str().unwrap();
            if current.to_ascii_lowercase() == name.to_ascii_lowercase() {
                let relative = outer_path.strip_prefix(&mapdir).unwrap();
                let relative_str = relative.to_str().unwrap();
                return Ok(relative_str.to_owned());
            }
            continue;
        }

        for inner in fs::read_dir(outer_path)? {
            let path = inner.unwrap().path();
            let current = path
                .file_name()
                .unwrap()
                .to_str()
                .expect("Invalid unicode in path");

            if current.to_ascii_lowercase() == name.to_ascii_lowercase() {
                let relative = path.strip_prefix(mapdir).unwrap();
                let relative_str = relative.to_str().unwrap();
                return Ok(relative_str.to_owned());
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Could not find map",
    ))
}
