#[cfg(feature = "swagger")]
use utoipa::OpenApi;

#[cfg(feature = "swagger")]
#[cfg_attr(feature = "swagger", derive(OpenApi))]
#[cfg_attr(
    feature = "swagger",
    openapi(
        paths(
            crate::routes::start_sc2,
            common::api::process::stats,
            common::api::process::stats_host,
            common::api::process::status,
            common::api::health
        ),
        components(schemas(
            common::models::Status,
            common::models::StartResponse,
            common::models::ProcessStatusResponse,
            common::api::process::ProcStatus
        ))
    )
)]
pub struct ApiDoc;
