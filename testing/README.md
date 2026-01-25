# Testing

Ensure you have the `aiarena-test-bots` submodule cloned. An easy check would be to check if the `./testing/aiarena-test-bots` 
directory contains any bots. If you are missing the bots, run the following command:

`git submodule update --init --recursive`

The integration tests are split into three groups: bot controller tests, match controller tests, and sc2 controller tests.

## Bot/SC2 controller tests

Bot controller tests check that the bot controller can run bots of the supported types.
SC2 controller tests check that the sc2 game controller produces the correct result of the match.

Create `config.toml` file under `client_controller` directory with contents:
```
VERSION = "latest"

BOTS_DIRECTORY = "../../testing/aiarena-test-bots"
GAMESETS_DIRECTORY = "../../testing/testing-maps"
```

Run under `client_controller` directory with:

```
cargo run
```

The client controller will wait for matches to run.

For bot controller tests pipe file `testing/bot-controller/test-matches` or paste them in the console.
For SC2 controller tests pipe file `testing/sc2-controller/test-matches` or paste them in the console.

## Match controller tests

These tests check that the match controller properly prepares the match environment by downloading the bot binaries and data, and that it completes the match by uploading the match result and artifacts.

Create `config.toml` file under `client_controller` directory with contents:
```
VERSION = "latest"

API_URL = "http://host.docker.internal:3000"
```

Run `teting/test-api-server` and then `client_controller` with:

```
cargo run
```

in their corresponding directory.
