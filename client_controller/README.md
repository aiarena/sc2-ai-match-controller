
The client controller for AI Arena runs scheduled matches.

It will support the following environments:
- Docker - runs matches [Docker Compose](https://docs.docker.com/compose/)
- Kubernetes - runs matches as jobs in Kubernetes cluster

It will operate in the following modes::
- Pull - when given AI Arena API configuration it will poll the API for scheduled matches
- Push - otherwise, it will read a file from the standard input that contains a list of matches

Right now, the client controller is used to consolidate the testing setup of this repo
and it supports only Docker environment in Push mode of operation.

#### Parameters

The client controller can be configured with the following parameters provided as environment variables:

| Parameter | Default | Description |
|-----------|---------|-------------|
| BOTS_DIRECTORY | ./bots | A folder with bot code and data. Each bot is in a subfolder with its name |
| GAMESETS_DIRECTORY | ./gamesets | A folder with game sets. Currently these are SC2 maps |
| LOGS_DIRECTORY | ./logs | A folder to write logs to |
| VERSION | latest | The version of AI Arena client to run matches with |
