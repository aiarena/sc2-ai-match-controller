# Testing

Ensure you have the `aiarena-test-bots` submodule cloned. An easy check would be to check if the `./testing/aiarena-test-bots` 
directory contains any bots. If you are missing the bots, run the following command:

`git submodule update --init --recursive`

To run integration tests,  run the following command from the root of the Git repo:
`docker-compose -f .\testing\docker-compose.yml up --exit-code-from=proxy_controller --build --force-recreate`

To run the integration tests in release mode, run the following command from the root of the Git repo:
## Windows
`$env:TEST_CARGO_FLAGS='--release'; docker-compose -f .\testing\docker-compose.yml up --exit-code-from=proxy_controller --build --force-recreate; Remove-Item Env:\TEST_CARGO_FLAGS`

## Other 
`TEST_CARGO_FLAGS="--release" docker-compose -f ./testing/docker-compose.yml up --exit-code-from=proxy_controller --build --force-recreate`
