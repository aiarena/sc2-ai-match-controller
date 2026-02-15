use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tokio::net::lookup_host;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    let game_host = std::env::var("GAME_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let game_port = std::env::var("GAME_PORT").expect("Missing GAME_PORT environment variable");
    let game_pass = std::env::var("GAME_PASS").unwrap_or_else(|_| game_port.clone());

    let bot_name = std::env::var("BOT_NAME").expect("Missing BOT_NAME environment variable");
    let opponent_id =
        std::env::var("OPPONENT_ID").expect("Missing OPPONENT_ID environment variable");

    let game_address = format!("{game_host}:{game_port}");
    let server_address = match lookup_host(game_address).await {
        Ok(mut addrs) => addrs.next().map(|x| x.ip().to_string()),
        Err(_) => None,
    }
    .unwrap_or(game_host);

    let _guards = init_controller_logs();
    fs::create_dir_all("/bot/logs")
        .unwrap_or_else(|e| panic!("Could not create bot logs directory: {e:?}"));

    let mut command = construct_bot_command("/bot", &bot_name);
    let command = command
        .stdout(create_log_file("/bot/logs/stdout.log"))
        .stderr(create_log_file("/bot/logs/stderr.log"))
        .arg("--GamePort")
        .arg(&game_port)
        .arg("--LadderServer")
        .arg(server_address)
        .arg("--StartPort")
        .arg(&game_pass)
        .arg("--OpponentId")
        .arg(opponent_id)
        .current_dir("/bot");

    info!("Starting bot with command {:?}", &command);
    match command.status() {
        Ok(exit_status) => {
            info!("Bot process exited with status: {}", exit_status);
        }
        Err(e) => {
            info!("Bot process failed with error: {}", e);
        }
    };
}

fn init_controller_logs() -> (
    tracing_appender::non_blocking::WorkerGuard,
    tracing_appender::non_blocking::WorkerGuard,
) {
    let controller_logs = create_log_file("/logs/controller.log");

    let (non_blocking_stdout, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
    let (non_blocking_controller_logs, controller_logs_guard) =
        tracing_appender::non_blocking(controller_logs);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_controller_logs)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_target(false),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_stdout)
                .with_file(true)
                .with_line_number(true)
                .with_target(false),
        )
        .init();

    info!("Controller logs initialized.");
    (stdout_guard, controller_logs_guard)
}

fn create_log_file(file_name: &str) -> File {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(file_name)
        .expect(&format!("Could not create file {}", file_name))
}

fn construct_bot_command(bot_folder: &str, bot_name: &str) -> Command {
    info!("Constructing bot command...");

    let bot_path = Path::new(&bot_folder);

    if exists(&bot_folder, "run.py") {
        command("python", &["run.py"])
    } else if exists(&bot_folder, &format!("{bot_name}.dll")) {
        command("dotnet", &[&format!("{bot_name}.dll")])
    } else if exists(&bot_folder, &format!("{bot_name}.jar")) {
        command("java", &["-jar", &format!("{bot_name}.jar")])
    } else if exists(&bot_folder, &format!("{bot_name}.js")) {
        command("node", &[&format!("{bot_name}.js")])
    } else if exists(&bot_folder, &format!("{bot_name}.exe")) {
        command("wine", &[&format!("{bot_name}.exe")])
    } else if exists(&bot_folder, &format!("./{bot_name}")) {
        let bot_binary = Path::new(&bot_folder).join(&format!("./{bot_name}"));

        if let Ok(file) = std::fs::metadata(&bot_binary) {
            info!("Setting bot file permissions");
            let mut permissions = file.permissions();
            permissions.set_mode(0o777);
            let _ = std::fs::set_permissions(&bot_binary, permissions);
        }

        Command::new(format!("./{bot_name}"))
    } else {
        // The executable was not found, list the contents of the bot folder for debugging
        info!("Listing contents of bot folder: {}", bot_folder);
        if let Ok(entries) = std::fs::read_dir(&bot_path) {
            for entry in entries.flatten() {
                info!("{:?}", entry.path());
            }
        }

        panic!("Bot executable not found in folder: {}", bot_folder);
    }
}

fn command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd
}

fn exists(folder: &str, executable: &str) -> bool {
    Path::new(folder).join(executable).exists()
}
