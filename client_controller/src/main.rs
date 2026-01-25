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

    let mut run_type = "test";
    let mut rounds = "-1";
    let mut bot1_directory = config.bots_directory.clone();
    let mut bot2_directory = config.bots_directory.clone();

    if !config.api_url.is_empty() {
        println!("Reading matches from API at: {}", config.api_url);

        run_type = "aiarena";

        // Right now it supports only the test api server which responds with 1 match only
        // Set rounds to 1 to avoid infinite loop over the same match
        rounds = "1";

        // Adjustment for when bots are in separate directories
        bot1_directory = format!("{}/bot1", config.bots_directory);
        bot2_directory = format!("{}/bot2", config.bots_directory);
    } else {
        // Prepare matches file
        println!("Reading matches from standard input");

        let mut stdin_content = String::new();
        io::stdin()
            .read_to_string(&mut stdin_content)
            .unwrap_or_else(|e| panic!("Could not read from stdin: {e:?}"));
        let mut matches_file = File::create("target/matches")
            .unwrap_or_else(|e| panic!("Could not create target/matches file: {e:?}"));
        matches_file.write_all(stdin_content.as_bytes())
            .unwrap_or_else(|e| panic!("Could not write to target/matches file: {e:?}"));

        println!("\nMatches:\n{}", stdin_content);
    }

    // Prepare the template to schedule a match
    let template = include_str!("../templates/docker-compose.yaml");
    let template = template.replace("PLACEHOLDER_VERSION", &config.version);
    let template = template.replace("PLACEHOLDER_RUN_TYPE", &run_type);
    let template = template.replace("PLACEHOLDER_ROUNDS", &rounds);
    let template = template.replace("PLACEHOLDER_API_URL", &config.api_url);
    let template = template.replace("PLACEHOLDER_BOTS_DIRECTORY", &config.bots_directory);
    let template = template.replace("PLACEHOLDER_BOT1_DIRECTORY", &bot1_directory);
    let template = template.replace("PLACEHOLDER_BOT2_DIRECTORY", &bot2_directory);
    let template = template.replace("PLACEHOLDER_GAMESETS_DIRECTORY", &config.gamesets_directory);
    let template = template.replace("PLACEHOLDER_LOGS_DIRECTORY", &config.logs_directory);

    let mut compose_file = File::create("target/docker-compose.yaml")
        .unwrap_or_else(|e| panic!("Could not create docker-compose.yaml file: {e:?}"));
    compose_file.write_all(template.as_bytes())
        .unwrap_or_else(|e| panic!("Could not write to docker-compose.yaml file: {e:?}"));

    println!("\nDocker compose:\n{}", template);
    
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
