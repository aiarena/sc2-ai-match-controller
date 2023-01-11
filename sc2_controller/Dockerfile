FROM lukemathwalker/cargo-chef:latest-rust-1.65.0-bullseye AS chef
WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build Stage
FROM chef as builder
ARG CARGO_FLAGS="--release"

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook ${CARGO_FLAGS} --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build ${CARGO_FLAGS} --bin sc2_controller


FROM aiarena/arenaclient-sc2-base:4.10
ARG APP=/app


RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

WORKDIR ${APP}

COPY --from=builder /app/target/*/sc2_controller ${APP}/sc2_controller

ENTRYPOINT ["./sc2_controller"]