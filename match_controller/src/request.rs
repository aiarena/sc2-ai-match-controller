use crate::race::BotRace;
use anyhow::bail;
use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MatchRequest {
    pub match_id: u32,
    pub map_name: String,
    pub player_1_id: String,
    pub player_1_name: String,
    pub player_1_race: u8,
    pub player_2_id: String,
    pub player_2_name: String,
    pub player_2_race: u8,
}

impl MatchRequest {
    pub fn read_from_line(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path).with_context(|| format!("Failed to read matches file: {}", path))?;
        parse_match_line(content.trim())
    }

    pub fn read_from_file() -> anyhow::Result<Self> {
        let path = "/match/match-request.toml";
        anyhow::ensure!(std::path::Path::new(path).exists(), "Match request file not found: {}", path);
        config::Config::builder()
            .add_source(config::File::new(path, config::FileFormat::Toml))
            .add_source(config::Environment::default())
            .build()
            .context("Could not read match request data")?
            .try_deserialize::<Self>()
            .context("Could not parse match request data")
    }

    pub fn write_to_file(&self) -> std::io::Result<()> {
        let toml_str = toml::to_string(self).map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Could not serialize match request"))?;
        std::fs::create_dir_all("/match")?;
        std::fs::write("/match/match-request.toml", toml_str)
    }

    pub fn delete_file() -> anyhow::Result<()> {
        let path = std::path::Path::new("/match/match-request.toml");
        if path.exists() {
            std::fs::remove_file(path).context("Failed to delete match request file")?;
        }
        Ok(())
    }
}

fn parse_match_line(line: &str) -> anyhow::Result<MatchRequest> {
    let vec_line: Vec<String> = line.split(',').map(std::string::ToString::to_string).collect();

    if vec_line.len() < 9 || vec_line.len() > 10 {
        bail!("Expected 9 or 10 fields in line, got {}: {:?}", vec_line.len(), vec_line);
    }

    Ok(MatchRequest {
        match_id: 0,
        map_name: format!("{}.SC2Map", vec_line[8]),
        player_1_id: vec_line[0].clone(),
        player_1_name: vec_line[1].clone(),
        player_1_race: BotRace::from_str(&vec_line[2]) as u8,
        player_2_id: vec_line[4].clone(),
        player_2_name: vec_line[5].clone(),
        player_2_race: BotRace::from_str(&vec_line[6]) as u8,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_match_line;

    #[test]
    pub fn test_match_extracts_valid() {
        let m = parse_match_line("bot-id-1,basic_bot,T,python,bot-id-2,loser_bot,P,python,AutomatonLE,Player1Win");
        assert!(m.is_ok());
        let m = m.unwrap();
        assert_eq!(m.player_1_id, "bot-id-1");
        assert_eq!(m.player_2_id, "bot-id-2");
        assert_eq!(m.player_1_name, "basic_bot");
        assert_eq!(m.player_2_name, "loser_bot");
        assert_eq!(m.player_1_race, 1); // Terran
        assert_eq!(m.player_2_race, 3); // Protoss
        assert_eq!(m.map_name, "AutomatonLE.SC2Map");
    }

    #[test]
    pub fn test_match_extracts_invalid_extra_field() {
        let m = parse_match_line("AutomatonLE,AutomatonLE,basic_bot,T,python,bot-id-2,loser_bot,P,python,AutomatonLE,AutomatonLE");
        assert!(m.is_err());
    }
}
