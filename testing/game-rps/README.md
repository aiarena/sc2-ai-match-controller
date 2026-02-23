# Rock-Paper-Scissors Game

A simple Rock-Paper-Scissors game implementation for testing and reference purposes.

## Overview

This is a minimal game implementation where:
- The **game** opens two TCP ports (specified by `PLAYER_1_SEAT` and `PLAYER_2_SEAT` environment variables)
- Two **bots** connect to these ports and send their moves (R for Rock, P for Paper, S for Scissors)
- The game reads the match request from `/match/match-request.toml` and writes the result to `/match/match-result.json`
- All other configuration is read from environment variables
- All stdout goes to `/logs/stdout.log` and stderr to `/logs/stderr.log`

## Components

### Game
- AI Arena protocol:
    - Listens on ports specified by `PLAYER_1_SEAT` and `PLAYER_2_SEAT` environment variables
    - Reads match ID from `/match/match-request.toml`
- Game protocol:
    - Accepts single character moves: R (Rock), P (Paper), or S (Scissors)
    - Determines winner using standard Rock-Paper-Scissors rules
- AI Arena protocol:
    - Writes result to `/match/match-result.json` with format:
      ```json
      {
        "match": <match_id>,
        "bot1_avg_step_time": 0,
        "bot1_tags": [],
        "bot2_avg_step_time": 0,
        "bot2_tags": [],
        "type": "Player1Win|Player2Win|Tie",
        "game_steps": 1
      }
      ```

**Game:**
```bash
docker build --build-arg MODE=game -t rps-game .
```

### Bot
- AI Arena protocol:
    - Connects to game using `GAME_HOST` and `GAME_PORT` environment variables
- Game protocol:
    - Sends move specified by `MOVE` environment variable (R, P, or S)

**Rock Bot (always plays Rock):**
```bash
docker build --build-arg MODE=bot --build-arg MOVE=R -t rps-bot-rock .
```

**Paper Bot (always plays Paper):**
```bash
docker build --build-arg MODE=bot --build-arg MOVE=P -t rps-bot-paper .
```
