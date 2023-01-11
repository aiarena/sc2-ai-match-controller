# sc2-ai-match-controller

## Overview

sc2-ai-match-controller is the next iteration of the arenaclient created for [ai-arena](https://ai-arena.net/) to act as a proxy between 
bots and StarCraft II. It is made to specifically take advantage of the container infrastructure [ai-arena](https://ai-arena.net/) uses to run
its games, and it has split up into three main components:

### proxy_controller
This is the main controller, and it is in charge of fetching and starting games. The websocket proxy between bots and SC2 is also contained
within this controller.

### sc2_controller
This controller is a simple API that is solely in charge of starting SC2 with the arguments received from the proxy_controller

### bot_controller
This controller is a simple API that is solely in charge of starting bots with the arguments received from the proxy_controller

Although these controllers can all run in the same container, the goal of creating controllers for each aspect of the SC2 matches was
to split up each controller into its own container.


## Testing
### Unit Tests
Run `cargo test`

### Integration Tests
Please see [Testing README](./testing/README.md)

## Examples
Please see the `examples` directory.


## Contributing
Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

Please make sure to update tests as appropriate.

## License
[GNU GPLv3](https://choosealicense.com/licenses/gpl-3.0/)
