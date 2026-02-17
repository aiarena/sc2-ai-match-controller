mod config;
mod docker;

use crate::config::{initialize_config, MatchRequest};
use crate::docker::run_match;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    println!("Starting client controller");

    let config = initialize_config();

    if !config.api_url.is_empty() {
        println!("Reading matches from API at: {}", config.api_url);

        println!("This version of client controller supports only the test-server-api with a single test match");
        let match_request = MatchRequest::from_csv_line("1,basic_bot,T,python,2,loser_bot,T,python,AutomatonLE");

        run_match("aiarena", &config, &match_request);
    } else if !config.matches_file.is_empty() {
        println!("Reading matches from file: {}", config.matches_file);

        let file = File::open(&config.matches_file)
            .unwrap_or_else(|e| panic!("Could not open matches file {}: {e:?}", config.matches_file));
        let reader = BufReader::new(file);

        fs::create_dir_all("target/logs/match")
            .unwrap_or_else(|e| panic!("Could not create target directory: {e:?}"));
        
        let path = Path::new("target/logs/match/match_result.json");
        if path.exists() {
            let _ = fs::remove_file(path);
        }

        for line in reader.lines() {
            let line = line.unwrap_or_else(|e| panic!("Could not read line from matches file: {e:?}"));
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let match_request = MatchRequest::from_csv_line(&line);

            println!("Running match: {:?}", match_request);

            // Store the current line in ./match file (overwrite)
            let mut match_file = File::create("target/logs/match/match-request.csv")
                .unwrap_or_else(|e| panic!("Could not create match file: {e:?}"));
            match_file.write_all(line.as_bytes())
                .unwrap_or_else(|e| panic!("Could not write to match file: {e:?}"));

            run_match("test", &config, &match_request);
        }

    } else {
        eprintln!("Client controller requires either API_URL or MATCHES_FILE to read matches!");
        std::process::exit(1);
    }

    println!("Client controller exits.");
}
