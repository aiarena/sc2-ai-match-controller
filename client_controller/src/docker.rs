use crate::config::{Bot, ControllerConfig, MatchRequest};
use std::env::temp_dir;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub fn run_match(run_type: &str, config: &ControllerConfig, request: &MatchRequest) {
    let match_directory = format!("{}/match", config.logs_directory);
    let (bot1_controller, bot1_command, bot1_directory) = select_bot_controller(run_type, config, &request.bot1, &request.bot2, "bot1", "10001");
    let (bot2_controller, bot2_command, bot2_directory) = select_bot_controller(run_type, config, &request.bot2, &request.bot1, "bot2", "10002");

    // Prepare the template to schedule a match
    let template = include_str!("../templates/docker-compose.yaml");
    let template = template.replace("PLACEHOLDER_RUN_TYPE", run_type);
    let template = template.replace("PLACEHOLDER_VERSION", &config.version);
    let template = template.replace("PLACEHOLDER_API_URL", &config.api_url);
    let template = template.replace("PLACEHOLDER_GAME_CONTROLLER", &config.game_controller);
    let template = template.replace("PLACEHOLDER_BOTS_DIRECTORY", &config.bots_directory);
    let template = template.replace("PLACEHOLDER_BOT1_ID", &request.bot1.id);
    let template = template.replace("PLACEHOLDER_BOT1_NAME", &request.bot1.name);
    let template = template.replace("PLACEHOLDER_BOT1_CONTROLLER", &bot1_controller);
    let template = template.replace("PLACEHOLDER_BOT1_COMMAND", &bot1_command);
    let template = template.replace("PLACEHOLDER_BOT1_DIRECTORY", &bot1_directory);
    let template = template.replace("PLACEHOLDER_BOT2_ID", &request.bot2.id);
    let template = template.replace("PLACEHOLDER_BOT2_NAME", &request.bot2.name);
    let template = template.replace("PLACEHOLDER_BOT2_CONTROLLER", &bot2_controller);
    let template = template.replace("PLACEHOLDER_BOT2_COMMAND", &bot2_command);
    let template = template.replace("PLACEHOLDER_BOT2_DIRECTORY", &bot2_directory);
    let template = template.replace("PLACEHOLDER_GAMESETS_DIRECTORY", &config.gamesets_directory);
    let template = template.replace("PLACEHOLDER_LOGS_DIRECTORY", &config.logs_directory);
    let template = template.replace("PLACEHOLDER_MATCH_DIRECTORY", &match_directory);

    let mut compose_file = File::create("target/docker-compose.yaml")
        .unwrap_or_else(|e| panic!("Could not create docker-compose.yaml file: {e:?}"));
    compose_file.write_all(template.as_bytes())
        .unwrap_or_else(|e| panic!("Could not write to docker-compose.yaml file: {e:?}"));

    println!("\nDocker compose:\n{}", template);

    let status = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg("target/docker-compose.yaml")
        .arg("up")
        .arg("-d")
        .arg("--force-recreate")
        .status()
        .expect("Failed to start docker compose");

    if !status.success() {
        eprintln!("Failed to start docker compose");
        std::process::exit(1);
    }

    let mut logs_process = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg("target/docker-compose.yaml")
        .arg("logs")
        .arg("-f")
        .spawn()
        .expect("Unable to stream docker compose logs");

    println!("Waiting for match_controller to exit...");
    loop {
        let output = Command::new("docker")
            .arg("compose")
            .arg("-f")
            .arg("target/docker-compose.yaml")
            .arg("ps")
            .arg("--format")
            .arg("json")
            .arg("match_controller")
            .output()
            .expect("Failed to check match_controller status");

        if output.status.success() {
            let json_str = String::from_utf8_lossy(&output.stdout);
            // If output is empty or container is not running, it has exited
            if json_str.trim().is_empty() || !json_str.contains("\"State\":\"running\"") {
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Get exit code of match_controller
    let output = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg("target/docker-compose.yaml")
        .arg("ps")
        .arg("--all")
        .arg("--format")
        .arg("json")
        .arg("match_controller")
        .output()
        .expect("Failed to get match_controller status");

    let exit_code = if output.status.success() {
        let json_str = String::from_utf8_lossy(&output.stdout);
        // Extract exit code from JSON (e.g., "ExitCode":0)
        json_str
            .split("\"ExitCode\":")
            .nth(1)
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(1)
    } else {
        1
    };

    println!("Match controller exited with code: {}", exit_code);

    // Stop logs process
    logs_process.kill().ok();
    logs_process.wait().ok();

    // Stop docker compose
    let status = Command::new("docker")
        .arg("compose")
        .arg("-f")
        .arg("target/docker-compose.yaml")
        .arg("down")
        .arg("--timeout=0")
        .status()
        .expect("Failed to stop docker compose");

    if !status.success() {
        eprintln!("Failed to stop docker compose");
    }

    if exit_code != 0 {
        eprintln!("Match controller failed with exit code: {}", exit_code);
        std::process::exit(exit_code);
    }
}

fn select_bot_controller(run_type: &str, config: &ControllerConfig, bot: &Bot, opponent: &Bot, bot_directory: &str, game_port: &str) -> (String, String, String) {
    let (controller, command, directory);
    let path = format!("{}/{}", config.bots_directory, bot.name);

    if bot.base.is_empty() {
        // This bot doesn't use custom docker image. Use the default bot controller
        controller = config.bot_controller.clone();
        command = "null".to_string();
        directory = if run_type == "aiarena" {
            format!("{}/{}/{}", config.bots_directory, bot_directory, bot.name)
        } else {
            format!("{}/{}", config.bots_directory, bot.name)
        }
    } else if Path::new(&path).exists() {
        // This bot uses custom docker image and its code is not included in the image
        controller = bot.base.clone();
        command = construct_bot_command(&bot.runtype, &bot.name, &game_port, &opponent.id);
        directory = format!("{}/{}", config.bots_directory, bot.name);
    } else {
        // This bot uses custom docker image and its code is included in the image
        controller = bot.base.clone();
        command = "null".to_string();
        directory = temp_dir().to_string_lossy().to_string().replace('\\', "/");
    }

    (controller, command, directory)
}

fn construct_bot_command(bot_type: &String, bot_name: &String, game_port: &str, opponent_id: &String) -> String {
    let command = match bot_type.as_str() {
        "cppwin32" => format!("wine {bot_name}.exe"),
        "cpplinux" => format!("./{bot_name}"),
        "dotnetcore" => format!("dotnet {bot_name}.dll"),
        "java" => format!("java -jar {bot_name}.jar"),
        "linux" => format!("./{bot_name}"),
        "nodejs" => format!("node {bot_name}.js"),
        "python" => "python run.py".to_string(),
        _ => format!("./{bot_name}"),
    };

    format!(
        "sh -c \"mkdir -p /bot/logs && cd /bot/ && {command} \
         --GamePort {game_port} --LadderServer 127.0.0.1 \
         --StartPort {game_port} --OpponentId {opponent_id} \
         > /bot/logs/stdout.log 2> /bot/logs/stderr.log\""
    )
}
