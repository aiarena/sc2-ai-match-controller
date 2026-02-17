#!/bin/bash

# Create logs directory and redirect output
mkdir -p /logs
exec > /logs/stdout.log 2> /logs/stderr.log

echo "Starting Rock-Paper-Scissors bot"

# Send the move to the game using socat
echo "DEBUG [$(date +%H:%M:%S.%N | cut -b1-12)]: Connecting to game at $GAME_HOST:$GAME_PORT and playing move: $MOVE"
if echo -n "$MOVE" | socat - TCP:$GAME_HOST:$GAME_PORT; then
    echo "DEBUG [$(date +%H:%M:%S.%N | cut -b1-12)]: Move sent successfully"
    echo "Move sent. Bot exiting."
    exit 0
else
    echo "DEBUG [$(date +%H:%M:%S.%N | cut -b1-12)]: Failed to send move (exit code: $?)"
    echo "ERROR: Failed to connect to game"
    exit 1
fi
