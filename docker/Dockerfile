FROM lukemathwalker/cargo-chef:latest-rust-1.68-bullseye AS chef
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

ARG FEATURES=""
RUN cargo build ${CARGO_FLAGS} --features="${FEATURES}"


FROM debian:bullseye-slim as k8s_controller
ARG APP=/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC

RUN mkdir -p ${APP}

WORKDIR ${APP}

COPY --from=builder /app/target/*/k8s_controller ${APP}/k8s_controller

ENTRYPOINT ["./k8s_controller"]
LABEL service=k8s_controller

FROM aiarena/arenaclient-bot-base:v0.3.1 as bot_controller
ARG APP=/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata p7zip-full \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC

RUN mkdir -p ${APP}

WORKDIR ${APP}

COPY --from=builder /app/target/*/bot_controller ${APP}/bot_controller

ENTRYPOINT ["./bot_controller"]
LABEL service=bot_controller

FROM aiarena/arenaclient-sc2-base:4.10 as sc2_controller
ARG APP=/app


RUN apt-get update \
    && apt-get install -y ca-certificates tzdata p7zip-full \
    && rm -rf /var/lib/apt/lists/*

WORKDIR ${APP}

COPY --from=builder /app/target/*/sc2_controller ${APP}/sc2_controller

ENTRYPOINT ["./sc2_controller"]
LABEL service=sc2_controller

FROM debian:bullseye-slim as proxy_controller
ARG APP=/app

USER root
WORKDIR /root/

# Update system
RUN apt-get update \
    && apt-get upgrade --assume-yes --quiet=2 \
    && apt-get install --assume-yes --no-install-recommends --no-show-upgraded


RUN apt-get update \
    && apt-get install -y ca-certificates tzdata p7zip-full \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC

RUN mkdir -p ${APP}

WORKDIR ${APP}

COPY --from=builder /app/target/*/proxy_controller ${APP}/proxy_controller

ENTRYPOINT ["./proxy_controller"]
LABEL service=proxy_controller