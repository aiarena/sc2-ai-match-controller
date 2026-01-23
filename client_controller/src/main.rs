mod config;

use crate::config::initialize_config;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};

fn main() {
    println!("Starting client controller");

    // Initialize the configuration for the client controller
    let config = initialize_config();

    // Prepare target directory
    fs::create_dir_all("target")
        .unwrap_or_else(|e| panic!("Could not create target directory: {e:?}"));

    // Prepare matches file
    let mut stdin_content = String::new();
    io::stdin()
        .read_to_string(&mut stdin_content)
        .unwrap_or_else(|e| panic!("Could not read from stdin: {e:?}"));
    let mut matches_file = File::create("target/matches")
        .unwrap_or_else(|e| panic!("Could not create target/matches file: {e:?}"));
    matches_file.write_all(stdin_content.as_bytes())
        .unwrap_or_else(|e| panic!("Could not write to target/matches file: {e:?}"));

    println!("Matches:\n{}", stdin_content);


    // Prepare the template to schedule a match
    let template = include_str!("../templates/docker-compose.yaml");
    let template = template.replace("PLACEHOLDER_VERSION", &config.version);
    let template = template.replace("PLACEHOLDER_BOTS_DIRECTORY", &config.bots_directory);
    let template = template.replace("PLACEHOLDER_GAMESETS_DIRECTORY", &config.gamesets_directory);
    let template = template.replace("PLACEHOLDER_LOGS_DIRECTORY", &config.logs_directory);

    let mut compose_file = File::create("target/docker-compose.yaml")
        .unwrap_or_else(|e| panic!("Could not create docker-compose.yaml file: {e:?}"));
    compose_file.write_all(template.as_bytes())
        .unwrap_or_else(|e| panic!("Could not write to docker-compose.yaml file: {e:?}"));

    println!("Docker compose:\n{}", template);
    
    // Schedule the match
    // docker compose -f ./testing/file-based/docker-compose.yml up --exit-code-from=match_controller --timeout 20 --force-recreate
    let mut command = Command::new("docker")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .arg("compose")
        .arg("-f")
        .arg("target/docker-compose.yaml")
        .arg("up")
        .arg("--exit-code-from=match_controller")
        .arg("--timeout=20")
        .arg("--force-recreate")
        .spawn();

    match command {
        Ok(ref mut child) => {
            match child.wait() {
                Ok(exit_status) => {
                    if !exit_status.success() {
                        eprintln!("Docker compose exited with status: {}", exit_status);
                        std::process::exit(exit_status.code().unwrap_or(1));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to wait for docker compose: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to spawn docker compose: {}", e);
            std::process::exit(1);
        }
    }

    println!("Client controller exits.");
}
