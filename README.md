# sc2-ai-match-controller

## Overview

sc2-ai-match-controller is the next iteration of the arenaclient created for [AI Arena](https://aiarena.net/) to act as a proxy between 
bots and StarCraft II (SC2). It is made to specifically take advantage of the container infrastructure AI Arena uses to run
its games, and it has split up into three main components:

### match_controller
This is the main controller, and it is in charge of preparing the matches, by downloading bot and game assests from AI Arena, and finalizing the matches, by uploading the match results back to AI Arena.
Through configuration, the controller can run matches locally without downloading assets from AI Arena or uploading results back to it.

### sc2_controller
This controller is running SC2 game engine processes and exposes its API through websocket proxies.
All SC2-specific logic is contained within this controller.

### bot_controller
This controller is a simple API that is solely in charge of starting bots with the arguments received from the match_controller

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
