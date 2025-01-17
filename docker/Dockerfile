FROM lukemathwalker/cargo-chef:latest-rust-1.75-bookworm AS chef
WORKDIR /app

RUN apt update && apt install -y lld clang

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build Stage
FROM chef AS builder
ARG CARGO_FLAGS="--release"

COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook ${CARGO_FLAGS} --recipe-path recipe.json
# Build application
COPY . .

ARG FEATURES=""
RUN cargo build ${CARGO_FLAGS} --features="${FEATURES}"

###########################################################

FROM debian:bookworm-slim AS k8s_controller
ARG APP=/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC

RUN mkdir -m 777 ${APP} \
    && mkdir -m 777 /bots \
    && mkdir -m 777 /logs \
    && mkdir -m 777 /replays \
    && chmod -R 777 /root \
    && chmod -R 777 /tmp

WORKDIR ${APP}

COPY --from=builder /app/target/*/k8s_controller ${APP}/k8s_controller

ENTRYPOINT ["./k8s_controller"]
LABEL service=k8s_controller

###########################################################

FROM aiarena/arenaclient-bot-base:v0.6.1 AS bot_controller
ARG APP=/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata p7zip-full \
    && rm -rf /var/lib/apt/lists/*

ENV TZ=Etc/UTC

RUN mkdir -m 777 ${APP} \
    && mkdir -m 777 /bots \
    && mkdir -m 777 /logs \
    && mkdir -m 777 /replays \
    && chmod -R 777 /root \
    && chmod -R 777 /tmp

WORKDIR ${APP}

COPY --from=builder /app/target/*/bot_controller ${APP}/bot_controller

ENTRYPOINT ["./bot_controller"]
LABEL service=bot_controller

###########################################################

FROM aiarena/arenaclient-sc2-base:4.10-bookworm AS sc2_controller
ARG APP=/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata p7zip-full \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -m 777 ${APP} \
    && mkdir -m 777 /bots \
    && mkdir -m 777 /logs \
    && mkdir -m 777 /replays \
    && chmod -R 777 /root \
    && chmod -R 777 /tmp

WORKDIR ${APP}

COPY --from=builder /app/target/*/sc2_controller ${APP}/sc2_controller

ENTRYPOINT ["./sc2_controller"]
LABEL service=sc2_controller

###########################################################

FROM debian:bookworm-slim AS proxy_controller
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

RUN mkdir -m 777 ${APP} \
    && mkdir -m 777 /bots \
    && mkdir -m 777 /logs \
    && mkdir -m 777 /replays \
    && chmod -R 777 /root \
    && chmod -R 777 /tmp

WORKDIR ${APP}

COPY --from=builder /app/target/*/proxy_controller ${APP}/proxy_controller

ENTRYPOINT ["./proxy_controller"]
LABEL service=proxy_controller

###########################################################

FROM aiarena/arenaclient-bot-base:v0.6.1 AS combined
ARG APP=/app

USER root
WORKDIR /root/

# Update system
RUN apt-get update \
    && apt-get upgrade --assume-yes --quiet=2 \
    && apt-get install --assume-yes --no-install-recommends --no-show-upgraded

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata p7zip-full supervisor \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /var/log/supervisor

ENV TZ=Etc/UTC

RUN mkdir -m 777 ${APP} \
    && mkdir -m 777 /bots \
    && mkdir -m 777 /logs \
    && mkdir -m 777 /replays \
    && chmod -R 777 /root \
    && chmod -R 777 /tmp

WORKDIR ${APP}
COPY --from=sc2_controller /root/StarCraftII/ /root/StarCraftII/
COPY --from=builder /app/target/*/proxy_controller ${APP}/proxy_controller
COPY --from=builder /app/target/*/sc2_controller ${APP}/sc2_controller
COPY --from=builder /app/target/*/bot_controller ${APP}/bot_controller

COPY docker/supervisord.conf /etc/supervisor/conf.d/supervisord.conf

CMD ["/usr/bin/supervisord"]
LABEL service=combined