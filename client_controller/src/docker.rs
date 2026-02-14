use crate::config::Config;
use std::fs::File;
use std::io::Write;
use std::process::Command;

pub fn run_match(run_type: &str, config: &Config) {
    let mut bot1_directory = config.bots_directory.to_string();
    let mut bot2_directory = config.bots_directory.to_string();

    if run_type == "aiarena" {
        // Bots are in separate directories on AI Arena
        bot1_directory = format!("{}/bot1", config.bots_directory);
        bot2_directory = format!("{}/bot2", config.bots_directory);
    }

    // Prepare the template to schedule a match
    let template = include_str!("../templates/docker-compose.yaml");
    let template = template.replace("PLACEHOLDER_RUN_TYPE", run_type);
    let template = template.replace("PLACEHOLDER_VERSION", &config.version);
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

    // Start docker compose
    let status = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg("target/docker-compose.yaml")
        .arg("up")
        .arg("--force-recreate")
        .arg("--exit-code-from=match_controller")
        .arg("--timeout=20")
        .arg("--menu=false")
        .status()
        .expect("Failed to execute docker compose process");

    println!("Docker compose exited with status: {}", status);

    if !status.success() {
        eprintln!("Docker compose exited with status: {}", status);
        std::process::exit(status.code().unwrap_or(1));
    }
}
