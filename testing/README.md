# Testing

Ensure you have the `aiarena-test-bots` submodule cloned. An easy check would be to check if the `./testing/aiarena-test-bots` 
directory contains any bots. If you are missing the bots, run the following command:

`git submodule update --init --recursive`

The integration tests are split into three groups: bot controller tests, match controller tests, and sc2 controller tests.

## Bot controller tests

These tests check that the bot controller can run bots of the supported types.
Run with:

```
docker-compose -f .\testing\bot-controller\docker-compose.yml up --exit-code-from=match_controller --build --force-recreate
```

## Match controller tests

These tests check that the match controller properly prepares the match environment by downloading the bot binaries and data, and that it completes the match by uploading the match result and artifacts.
Run with:

```
docker-compose -f .\testing\match-controller\docker-compose.yml up --exit-code-from=match_controller --build --force-recreate
```

## SC2 controller tests

These tests check that the sc2 game controller produces the correct result of the match.
Run with:

```
docker-compose -f .\testing\sc2-controller\docker-compose.yml up --exit-code-from=match_controller --build --force-recreate
```
