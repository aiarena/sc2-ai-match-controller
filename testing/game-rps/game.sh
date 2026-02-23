#!/bin/bash

# Create log directory and redirect output
mkdir -p /logs
exec > /logs/stdout.log 2> /logs/stderr.log

echo "Starting Rock-Paper-Scissors game"

# Read match_id from match-request.toml
MATCH_ID=$(grep "^match_id" /match/match-request.toml | sed 's/match_id *= *//' | tr -d ' ')

echo "Playing match: $MATCH_ID"

# Prepare files for player moves
> /logs/player1.log
> /logs/player2.log

# Read moves from both players using socat
# Each socat will accept one connection and write data to file
socat -u TCP-LISTEN:$PLAYER_1_SEAT,reuseaddr,fork OPEN:/logs/player1.log,creat,append 2>/logs/socat1.log &
socat -u TCP-LISTEN:$PLAYER_2_SEAT,reuseaddr,fork OPEN:/logs/player2.log,creat,append 2>/logs/socat2.log &
echo "[$(date +%H:%M:%S.%N | cut -b1-12)] Listening on ports $PLAYER_1_SEAT (Player 1) and $PLAYER_2_SEAT (Player 2)"

# Block until both files have a size greater than 0 bytes
while [[ ! -s /logs/player1.log || ! -s /logs/player2.log ]]; do
    sleep 1
done

echo "[$(date +%H:%M:%S.%N | cut -b1-12)] Data received from both players"

# Get the moves (just first character)
PLAYER1_MOVE=$(head -c 1 /logs/player1.log 2>/dev/null || echo "")
PLAYER2_MOVE=$(head -c 1 /logs/player2.log 2>/dev/null || echo "")

echo "Player 1 played: $PLAYER1_MOVE"
echo "Player 2 played: $PLAYER2_MOVE"

# Determine winner
if [ -z "$PLAYER1_MOVE" ] || [ -z "$PLAYER2_MOVE" ]; then
    RESULT="Tie"
    echo "One or both players did not respond - Tie"
elif [ "$PLAYER1_MOVE" = "$PLAYER2_MOVE" ]; then
    RESULT="Tie"
    echo "Result: Tie"
elif [ "$PLAYER1_MOVE" = "R" ] && [ "$PLAYER2_MOVE" = "S" ]; then
    RESULT="Player1Win"
    echo "Result: Player 1 wins (Rock beats Scissors)"
elif [ "$PLAYER1_MOVE" = "S" ] && [ "$PLAYER2_MOVE" = "P" ]; then
    RESULT="Player1Win"
    echo "Result: Player 1 wins (Scissors beats Paper)"
elif [ "$PLAYER1_MOVE" = "P" ] && [ "$PLAYER2_MOVE" = "R" ]; then
    RESULT="Player1Win"
    echo "Result: Player 1 wins (Paper beats Rock)"
else
    RESULT="Player2Win"
    echo "Result: Player 2 wins"
fi

# Write result to JSON file
cat > /match/match_result.json << EOF
{
  "match": $MATCH_ID,
  "bot1_avg_step_time": 0,
  "bot1_tags": [],
  "bot2_avg_step_time": 0,
  "bot2_tags": [],
  "type": "$RESULT",
  "game_steps": 1
}
EOF

echo "Match result saved to /match/match_result.json"

exit 0
