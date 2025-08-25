This is the StarCraft II game controller for AI Arena clients.
It controls the StarCraft II game for a match between two bots.

## Input

The controller reads the following parameters from `<log_folder>/sc2_controller/match_request.toml` and the environment variables:

| Key | Default | Description |
|-----|---------|-------------|
| disable_debug | true | Ignores debug action requests by the bots during the match. |
| log_root | "/logs" | The root folder for logs in the arena client. This controller will write logs in `<log_root>/sc2_controller/` |
| map_name | - | The name of the StarCraft II map for the match. |
| match_id | - | An identifier for the match as seen in AI Arena |
| max_game_time | 60486 | Maximum game loops for the match. After this limit, the controller will close the match and call it a tie. |
| max_frame_time | 40 | Milliseconds waiting for a bot to process a game step. After this limit the controller will raise a timeout for this bot. |
| player_1_id | - | Identifier of player 1 |
| player_1_name | - | Display name of player 1 |
| player_1_race | - | Race of player 1 |
| player_1_seat | 10001 | The game port exposed to player 1 |
| player_2_id | - | Identifier of player 2 |
| player_2_name | - | Display name of player 2 |
| player_2_race | - | Race of player 2 |
| player_2_seat | 10002 | The game port exposed to player 2 |
| realtime | false | Determines whether the game runs in real time or the bots control the steps. |
| timeout_secs | 30 | Seconds waiting got a bot to respond during the match. After this limit the controller will raise a timeout for this bot. |
| validate_race | false | Enforce player races as given in `player_1_race` and `player_2_race`. |
| visualize | false | Not used. |

In the current version, the parameters are read from the combination of file `<log_folder>/sc2_controller/match_request.toml` and file `config.toml` of the match controller.
This will be later be changed and the parameters will be read from the environment variables.

## Ports

The controller opens two ports - `player_1_seat` and `player_2_seat` - for the bots to connect to.

In the current version, the controller expects a call to HTTP endpoint /start at port `8083` by the match controller.
This call will be removed later when the client controller (k8s_controller or docker compose) coordinates the match controller to prepare all inputs before the game controller (sc2_controller) is started.

## Output

The controller writes the following files to folder `<log_root>/sc2_controller/`:

| Filename | Description | Example contents |
|----------|-------------|---------|
| match_request.toml | The original request for the match | match_id=1<br>... |
| match_result.json | The result of the match | {"match_id": 1, "bot1_avg_step_time": 0.005, "bot1_tags": [], "bot2_avg_step_time": 0.003, "bot2_tags": [], "result": "Player1Win", "game_steps": 2200 } |
| sc2_controller.log | The logs of the controller | |
| stderr-\<port>.log | Error logs from SC2 game running on this port | |
| stdout-\<port>.log | Output logs from SC2 game running on this port | |
