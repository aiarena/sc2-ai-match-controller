FROM debian:bookworm-slim

# Install socat for TCP communication
RUN apt-get update && apt-get install -y socat && rm -rf /var/lib/apt/lists/*

# ARG to determine what this container runs (game or bot)
ARG MODE=game
ARG MOVE=R

# Copy scripts
COPY game.sh /game.sh
COPY bot.sh /bot.sh

RUN chmod +x /game.sh /bot.sh

# Set environment variables
ENV MODE=${MODE}
ENV MOVE=${MOVE}

# Default command based on MODE
CMD if [ "$MODE" = "game" ]; then /game.sh; else /bot.sh; fi
