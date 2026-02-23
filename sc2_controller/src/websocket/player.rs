use crate::game::game_config::GameConfig;
use crate::game::player_data::PlayerData;
use crate::game::player_result::PlayerResult;
use crate::game::sc2_result::Sc2Result;
use crate::websocket::errors::player_error::PlayerError;
use crate::websocket::port_config::PortConfig;
use crate::websocket::runtime_vars::RuntimeVars;
use axum::extract::ws::{Message as AMessage, WebSocket};
use common::models::aiarena::bot_race::BotRace;
use common::PlayerNum;
use futures_util::{SinkExt, StreamExt};
use protobuf::{EnumOrUnknown, Message, MessageField};
use sc2_proto::common::Race;
use sc2_proto::sc2api::{
    Request, RequestJoinGame, RequestLeaveGame, RequestPing, RequestSaveReplay, Response,
    ResponseDebug, Status,
};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::Message as TMessage;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, error, info, trace};

pub struct Player {
    bot_ws: WebSocket,
    sc2_ws: WebSocketStream<TcpStream>,
    bot_ws_timeout: Duration,
    sc2_ws_timeout: Duration,
}

impl Player {
    pub const fn new(bot_ws: WebSocket, sc2_ws: WebSocketStream<TcpStream>) -> Self {
        Self {
            bot_ws,
            sc2_ws,
            bot_ws_timeout: Duration::from_secs(30),
            sc2_ws_timeout: Duration::from_secs(60),
        }
    }

    /// Receive a message from the client
    /// Returns None if the connection is already closed
    pub async fn bot_recv_message(&mut self) -> Result<AMessage, PlayerError> {
        trace!("Waiting for a message from the client");
        match timeout(self.bot_ws_timeout, self.bot_ws.next()).await {
            Ok(res_msg) => match res_msg {
                Some(Ok(msg)) => {
                    trace!("Message received from client:\n{:?}", &msg);
                    Ok(msg)
                }
                Some(Err(e)) => Err(PlayerError::BotWebsocket(e)),
                None => Err(PlayerError::NoMessageAvailable),
            },
            Err(_) => Err(PlayerError::BotTimeout(self.bot_ws_timeout)),
        }
    }
    /// Send message to the client
    pub async fn bot_send_message(&mut self, msg: AMessage) -> Result<(), PlayerError> {
        trace!("Sending message to client");
        timeout(self.bot_ws_timeout, self.bot_ws.send(msg))
            .await
            .map_err(|_| PlayerError::BotTimeout(self.bot_ws_timeout))
            .and_then(|r| r.map_err(|e| e.into()))
    }
    /// Send a protobuf response to the client
    pub async fn bot_send_response(&mut self, r: &Response) -> Result<(), PlayerError> {
        trace!(
            "Response to client: [{}]",
            format!("{r:?}").chars().take(10).collect::<String>()
        );
        timeout(
            self.bot_ws_timeout,
            self.bot_send_message(AMessage::Binary(
                r.write_to_bytes().expect("Invalid protobuf message"),
            )),
        )
        .await
        .map_err(|_| PlayerError::BotTimeout(self.bot_ws_timeout))
        .and_then(|r| r)
    }
    pub async fn bot_send_bytes(&mut self, r: &[u8]) -> Result<(), PlayerError> {
        trace!(
            "Response to client: [{}]",
            format!("{r:?}").chars().take(100).collect::<String>()
        );
        timeout(
            self.bot_ws_timeout,
            self.bot_send_message(AMessage::Binary(r.to_owned())),
        )
        .await
        .map_err(|_| PlayerError::BotTimeout(self.bot_ws_timeout))
        .and_then(|r| r)
    }

    /// Get a protobuf request from the client
    /// Returns None if the connection is already closed
    pub async fn bot_recv_request(&mut self) -> Result<Request, PlayerError> {
        match self.bot_recv_message().await? {
            AMessage::Binary(bytes) => {
                let resp =
                    Message::parse_from_bytes(&bytes).map_err(PlayerError::ProtoParseError)?;
                trace!("Message from client parsed:\n{}", &resp);
                Ok(resp)
            }
            other => Err(PlayerError::BotUnexpectedMessage(other)),
        }
    }

    pub async fn bot_recv_request_bytes(&mut self) -> Result<Vec<u8>, PlayerError> {
        match self.bot_recv_message().await? {
            AMessage::Binary(bytes) => Ok(bytes),

            other => Err(PlayerError::BotUnexpectedMessage(other)),
        }
    }

    /// Send message to sc2
    /// Returns None if the connection is already closed
    async fn sc2_send_message(&mut self, msg: TMessage) -> Result<(), PlayerError> {
        timeout(self.sc2_ws_timeout, self.sc2_ws.send(msg))
            .await
            .map_err(|_| PlayerError::Sc2Timeout(self.sc2_ws_timeout))
            .and_then(|r| r.map_err(|e| e.into()))
    }

    /// Send protobuf request to sc2
    /// Returns None if the connection is already closed
    pub async fn sc2_send_request(&mut self, r: &Request) -> Result<(), PlayerError> {
        trace!("sc2_send_request: {}", r);
        self.sc2_send_message(TMessage::binary(
            r.write_to_bytes().expect("Invalid protobuf message"),
        ))
        .await
    }
    pub async fn sc2_send_bytes(&mut self, r: Vec<u8>) -> Result<(), PlayerError> {
        self.sc2_send_message(TMessage::binary(r)).await
    }
    /// Protobuf to create a new handler
    fn proto_create_game(players: &[CreateGamePlayer], map: &str, realtime: bool) -> Request {
        use sc2_proto::sc2api::{LocalMap, RequestCreateGame};

        let mut r_local_map = LocalMap::new();
        r_local_map.set_map_path(map.to_string());

        let mut r_create_game = RequestCreateGame::new();
        r_create_game.set_local_map(r_local_map);
        r_create_game.set_realtime(realtime);

        r_create_game.player_setup = players.iter().map(CreateGamePlayer::as_proto).collect();

        let mut request = Request::new();
        request.set_create_game(r_create_game);
        request
    }

    pub async fn create_game(&mut self, map: &str, realtime: bool) -> Result<(), PlayerError> {
        let ping_request = create_ping_request();
        for _ in 0..10 {
            match self.sc2_query(&ping_request).await {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    trace!("Error {:?}", e);
                    sleep(Duration::from_secs(3)).await;
                }
            }
        }
        // Craft CreateGame request
        let player_configs: Vec<CreateGamePlayer> = vec![CreateGamePlayer::Participant; 2];

        // Send CreateGame request to first procs
        let proto = Self::proto_create_game(&player_configs, map, realtime);
        let response = self.sc2_query(&proto).await?;

        let resp_create_game = response.create_game();
        if resp_create_game.has_error() {
            let error = resp_create_game.error();
            error!("Could not create handler: {:?}", &error);
            return Err(PlayerError::CreateGame(error));
        }
        // // Throttle fast bots
        // for _ in 0..10 {
        //     match self.sc2_query(&ping_request).await {
        //         Ok(resp) => {
        //             if resp.has_status(){
        //                 if resp.status() == Status::launched{
        //                     break;
        //                 }
        //             }
        //             sleep(Duration::from_secs(3)).await;
        //         }
        //         Err(e) => {
        //             trace!("Error {:?}", e);
        //             sleep(Duration::from_secs(3)).await;
        //         }
        //     }
        // }

        info!("Game created successfully");

        Ok(())
    }

    /// Wait and receive a protobuf request from sc2
    /// Returns None if the connection is already closed
    pub async fn sc2_recv_response(&mut self) -> Result<Response, PlayerError> {
        match timeout(self.sc2_ws_timeout, self.sc2_ws.next()).await {
            Ok(socket) => match socket {
                Some(Ok(TMessage::Binary(bytes))) => {
                    let msg =
                        Message::parse_from_bytes(&bytes).map_err(PlayerError::ProtoParseError)?;
                    trace!(
                        "sc2_recv_response: {:?}",
                        format!("{msg:?}").chars().take(250).collect::<String>()
                    );
                    Ok(msg)
                }
                Some(Ok(other)) => Err(PlayerError::Sc2UnexpectedMessage(other)),
                Some(Err(e)) => Err(PlayerError::Sc2Websocket(e)),
                None => Err(PlayerError::NoMessageAvailable),
            },
            Err(_) => Err(PlayerError::Sc2Timeout(self.sc2_ws_timeout)),
        }
    }

    pub async fn sc2_recv_bytes(&mut self) -> Result<Vec<u8>, PlayerError> {
        match timeout(self.sc2_ws_timeout, self.sc2_ws.next()).await {
            Ok(Some(Ok(TMessage::Binary(bytes)))) => Ok(bytes),
            Ok(Some(Ok(other))) => Err(PlayerError::Sc2UnexpectedMessage(other)),
            Ok(Some(Err(e))) => Err(PlayerError::Sc2Websocket(e)),
            Ok(None) => Err(PlayerError::NoMessageAvailable),
            Err(_) => Err(PlayerError::Sc2Timeout(self.sc2_ws_timeout)),
        }
    }

    /// Send a request to SC2 and return the reponse
    /// Returns None if the connection is already closed
    pub async fn sc2_query(&mut self, r: &Request) -> Result<Response, PlayerError> {
        trace!("sc2_query");
        self.sc2_send_request(r).await?;
        #[cfg(debug_assertions)]
        {
            let msg = self.sc2_recv_response().await;
            match &msg {
                Ok(resp) => {
                    if !resp.error().is_empty() {
                        error!("{:?}", resp.error());
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            }
            msg
        }
        #[cfg(not(debug_assertions))]
        {
            self.sc2_recv_response().await
        }
    }
    pub async fn sc2_query_bytes(&mut self, r: Vec<u8>) -> Result<Vec<u8>, PlayerError> {
        self.sc2_send_bytes(r).await?;
        self.sc2_recv_bytes().await
    }
    /// Saves replay to path
    pub async fn save_replay(&mut self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            if !parent.exists() && tokio::fs::create_dir_all(parent).await.is_err() {
                return false;
            }
        }
        let mut r = Request::new();
        r.set_save_replay(RequestSaveReplay::new());
        if let Ok(response) = self.sc2_query(&r).await {
            if response.has_save_replay() {
                match File::create(&path).await {
                    Ok(mut buffer) => {
                        let data: &[u8] = response.save_replay().data();
                        buffer
                            .write_all(data)
                            .await
                            .expect("Could not write to replay file");
                        info!("Replay saved to {:?}", &path);
                        true
                    }
                    Err(e) => {
                        error!("Failed to create replay file {:?}: {:?}", &path, e);
                        false
                    }
                }
            } else {
                error!("No replay data available");
                false
            }
        } else {
            error!("Could not save replay");
            false
        }
    }
    async fn wait_for_join_game(
        &mut self,
        port_config: PortConfig,
        config: &GameConfig,
        player_num: PlayerNum,
        player_pass: u32,
    ) -> Result<Option<u32>, PlayerError> {
        loop {
            let msg = self.bot_recv_request().await?;

            if msg.has_quit() {
                return Err(PlayerError::BotQuit);
            } else if msg.has_ping() {
                let resp = self.sc2_query(&msg).await?;
                self.bot_send_response(&resp).await?;
            } else if msg.has_join_game() {
                let req_raw = proto_join_game_participant(
                    &msg,
                    &port_config,
                    config,
                    player_num,
                    player_pass,
                );

                if req_raw.is_none() {
                    return Err(PlayerError::NoMessageAvailable);
                }

                let resp = self.sc2_query(&req_raw.unwrap()).await?;
                self.bot_send_response(&resp).await?;

                let ping_request = create_ping_request();
                for _ in 0..10 {
                    let resp = self.sc2_query(&ping_request).await?;
                    match resp.status() {
                        Status::init_game | Status::in_game => break,
                        _ => {
                            sleep(Duration::from_secs(3)).await;
                            continue;
                        }
                    }
                }

                return Ok(resp.join_game().player_id);
            } else {
                return Err(PlayerError::UnexpectedRequest(msg));
            }
        }
    }

    pub async fn run(
        &mut self,
        config: GameConfig,
        port_config: PortConfig,
        player_num: PlayerNum,
        player_pass: u32,
    ) -> Result<PlayerResult, PlayerError> {
        let mut r_vars = RuntimeVars::new(&config);
        self.bot_ws_timeout = r_vars.timeout_secs;
        let mut response: Response;

        r_vars.player_id = self
            .wait_for_join_game(port_config, &config, player_num, player_pass)
            .await?;

        loop {
            match self.bot_recv_request().await {
                Ok(mut request) => {
                    r_vars.record_frame_time();

                    if config.disable_debug && request.has_debug() {
                        let debug_response = create_empty_debug_response(&request);
                        self.bot_send_response(&debug_response).await?;
                        continue;
                    } else if request.has_leave_game() || request.has_quit() {
                        self.save_replay(r_vars.replay_path()).await;
                        r_vars.set_surrender_flag();
                    }

                    // Using disable_fog=true in observation requests in combination with
                    // show_cloaked/show_burrowed_shadows in the join game request
                    // results in visibility of opponent units in the fog of war.
                    // Here, we make sure it doesn't happen by clearing disable_fog.
                    if request.has_observation() {
                        request.mut_observation().clear_disable_fog();
                    }

                    r_vars.add_tags(&request);

                    response = self.sc2_query(&request).await?;

                    if response.has_game_info() {
                        for pi in &mut response.mut_game_info().player_info {
                            if pi.player_id() != r_vars.player_id() {
                                pi.player_name =
                                    Some(config.players[&player_num.other_player()].name.clone());
                                pi.race_actual = pi.race_requested;
                            } else {
                                pi.player_name = Some(config.players[&player_num].name.clone());
                            }
                        }
                    }
                    self.bot_send_response(&response).await?;

                    r_vars.start_timing();
                    r_vars.start_time();

                    if response.has_leave_game() || response.has_quit() {
                        // self.save_replay(r_vars.replay_path()).await;
                        r_vars.record_avg_frame_time();
                        let result = r_vars.build_result(Sc2Result::Defeat);

                        return Ok(result);
                    } else if response.has_observation() {
                        r_vars.record_avg_frame_time();

                        let observation = response.observation();
                        r_vars.set_game_loops(observation.observation.game_loop());

                        let observation_results = &observation.player_result;

                        if !observation_results.is_empty() {
                            let sc2_result = observation_results
                                .iter()
                                .find(|x| x.player_id() == r_vars.player_id())
                                .map(|x| Sc2Result::from_proto(x.result()))
                                .unwrap();
                            self.save_replay(r_vars.replay_path()).await;
                            let result = r_vars.build_result(sc2_result);

                            return Ok(result);
                        }

                        if r_vars.game_loops > config.max_game_time {
                            self.save_replay(r_vars.replay_path()).await;
                            r_vars.record_avg_frame_time();
                            let mut request = Request::new();
                            let leave_game = RequestLeaveGame::new();
                            request.set_leave_game(leave_game);
                            let _resp = self.sc2_query(&request).await;
                            debug!("Max time reached");
                            let result = r_vars.build_result(Sc2Result::Tie);
                            return Ok(result);
                        }
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                    return match e {
                        PlayerError::NoMessageAvailable => {
                            Ok(r_vars.build_result(Sc2Result::Crash))
                        }
                        PlayerError::BotWebsocket(error) => {
                            error!("{:?}", error);
                            self.save_replay(r_vars.replay_path()).await;
                            r_vars.record_avg_frame_time();
                            let mut request = Request::new();
                            let leave_game = RequestLeaveGame::new();
                            request.set_leave_game(leave_game);
                            let _resp = self.sc2_query(&request).await;
                            Ok(r_vars.build_result(Sc2Result::Crash))
                        }
                        PlayerError::Sc2Websocket(error) => {
                            error!("{:?}", error);
                            Ok(r_vars.build_result(Sc2Result::SC2Crash))
                        }
                        PlayerError::BotUnexpectedMessage(message) => {
                            error!("BotUnexpectedMessage: {:?}", message);
                            self.save_replay(r_vars.replay_path()).await;
                            r_vars.record_avg_frame_time();
                            let mut request = Request::new();
                            let leave_game = RequestLeaveGame::new();
                            request.set_leave_game(leave_game);
                            let _resp = self.sc2_query(&request).await;

                            Ok(r_vars.build_result(Sc2Result::Crash))
                        }
                        PlayerError::Sc2UnexpectedMessage(message) => {
                            error!("SC2UnexpectedMessage: {:?}", message);
                            self.save_replay(r_vars.replay_path()).await;
                            r_vars.record_avg_frame_time();
                            Ok(r_vars.build_result(Sc2Result::SC2Crash))
                        }
                        PlayerError::BotTimeout(d) => {
                            error!("Bot Timeout of {:?}s reached", d);
                            self.save_replay(r_vars.replay_path()).await;
                            r_vars.record_avg_frame_time();
                            let mut request = Request::new();
                            let leave_game = RequestLeaveGame::new();
                            request.set_leave_game(leave_game);
                            let _resp = self.sc2_query(&request).await;

                            Ok(r_vars.build_result(Sc2Result::Timeout))
                        }
                        other => Err(other),
                    };
                }
            }
        }
    }
}

/// Used to pass player setup info to CreateGame
#[allow(dead_code)]
#[derive(Clone, Copy)]
enum CreateGamePlayer {
    Participant,
    Observer,
}

impl CreateGamePlayer {
    fn as_proto(&self) -> sc2_proto::sc2api::PlayerSetup {
        use sc2_proto::sc2api::{PlayerSetup, PlayerType};
        let mut ps = PlayerSetup::new();
        match self {
            Self::Participant => {
                ps.type_ = Some(EnumOrUnknown::new(PlayerType::Participant));
            }
            Self::Observer => {
                ps.type_ = Some(EnumOrUnknown::new(PlayerType::Observer));
            }
        }
        ps
    }
}

fn proto_join_game_participant(
    request: &Request,
    port_config: &PortConfig,
    config: &GameConfig,
    player_num: PlayerNum,
    player_pass: u32,
) -> Option<Request> {
    let mut r_join_game = RequestJoinGame::new();
    let mut player_data = PlayerData::from_join_request(request.join_game());

    if !do_passes_match(player_data.pass_port, player_pass) {
        return None;
    }

    if config.validate_race {
        player_data.race = to_race(&config.players[&player_num].race);
    }
    r_join_game.set_player_name(config.players[&player_num].name.clone());

    r_join_game.options = MessageField::from_option(Some(player_data.interface_options));

    r_join_game.set_race(player_data.race);

    port_config.apply_proto(&mut r_join_game);

    let mut request = request.clone();
    request.set_join_game(r_join_game);
    Some(request)
}

fn do_passes_match(a: u32, b: u32) -> bool {
    // The player is allowed to provide the pass port with a small offset
    // because it gets the pass port with the "--StartPort" parameter and is expected
    // to construct a list of ports in the JoinGame request and provide it in that list.
    // We ignore the last digit to account for the potential offset.
    let a = a / 10;
    let b = b / 10;

    if a != b {
        error!("Player provided wrong pass port {}, expected {}", a, b);
        return false;
    }
    true
}

fn to_race(race: &BotRace) -> Race {
    match race {
        BotRace::Terran => Race::Terran,
        BotRace::Zerg => Race::Zerg,
        BotRace::Protoss => Race::Protoss,
        BotRace::Random | BotRace::NoRace => Race::Random,
    }
}

fn create_empty_debug_response(request: &Request) -> Response {
    let mut debug_response = Response::new();
    let debug_response_debug = ResponseDebug::new();
    debug_response.set_id(request.id());
    debug_response.set_status(Status::in_game);
    debug_response.set_debug(debug_response_debug);
    debug_response
}

fn create_ping_request() -> Request {
    let mut request = Request::new();
    let ping = RequestPing::new();

    request.set_ping(ping);
    request
}
