#[cfg(feature = "swagger")]
use common::utoipa;
#[cfg(feature = "swagger")]
use utoipa::OpenApi;

#[cfg(feature = "swagger")]
#[cfg_attr(feature = "swagger", derive(OpenApi))]
#[cfg_attr(
    feature = "swagger",
    openapi(
        paths(
            crate::routes::terminate_bot,
            crate::routes::start_bot,
            crate::routes::download_bot_data,
            common::api::process::stats,
            common::api::process::stats_host,
            common::api::process::terminate_all,
            common::api::process::shutdown,
            common::api::process::status,
            common::api::health,
        ),
        components(schemas(
            common::models::bot_controller::StartBot,
            common::models::bot_controller::BotType,
            common::models::Status,
            common::models::TerminateResponse,
            common::models::StartResponse,
            common::models::ProcessStatusResponse,
            common::api::process::ProcStatus
        ))
    )
)]
pub struct ApiDoc;
