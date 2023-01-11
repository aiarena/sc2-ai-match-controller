FROM lukemathwalker/cargo-chef:latest-rust-1.65.0 AS chef
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
RUN cargo build ${CARGO_FLAGS} --bin bot_controller


FROM aiarena/arenaclient-bot-base:v0.1
ARG APP=/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC

RUN mkdir -p ${APP}

WORKDIR ${APP}

COPY --from=builder /app/target/*/bot_controller ${APP}/bot_controller

ENTRYPOINT ["./bot_controller"]