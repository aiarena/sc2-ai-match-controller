#[cfg(feature = "swagger")]
use utoipa::OpenApi;

#[cfg(feature = "swagger")]
#[cfg_attr(feature = "swagger", derive(OpenApi))]
#[cfg_attr(
    feature = "swagger",
    openapi(
        paths(
            crate::routes::terminate_sc2,
            crate::routes::start_sc2,
            common::api::process::stats,
            common::api::process::stats_host,
            common::api::process::terminate_all,
            common::api::process::shutdown,
            common::api::process::status,
            common::api::health
        ),
        components(schemas(
            common::models::Status,
            common::models::TerminateResponse,
            common::models::StartResponse,
            common::models::ProcessStatusResponse,
            common::api::process::ProcStatus
        ))
    )
)]
pub struct ApiDoc;
