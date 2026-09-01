# match_controller

Orchestrates StarCraft II AI matches between two bots.

## Flow

`main` loads settings and then runs either **prepare** or **submit** depending on the configured run type.

The two steps run as separate processes and communicate exclusively through files on a shared volume:

- **Match request file** — written by prepare, read by submit. Contains the match identity and bot details needed to run and report the match.
- **Match result file** — written by the game process (or by prepare/submit on failure). Read by submit when collecting results. The first result written is authoritative.

### Prepare

Retrieves the match to play, downloads all required assets, and writes a match request file for the submit step to pick up.

The match is either fetched from the AI Arena API (when credentials are configured) or read from a local match file. Assets (map and bot files) are downloaded from the AI Arena store, optionally via a caching server. Downloads from the cache server are single-attempt with no retries; on failure the asset is fetched directly from the store.

At the start of each run, prepare clears any shared files left by a previous match — the match request, the match result, and the bot exit signals — so submit never picks up stale data.

If asset download fails, prepare still writes the match request file and records an initialization error as the match result, allowing submit to report the failure to the API.

### Submit

Reads the match request file written by prepare; if the file is not present, submit stops immediately.

Once the match request is read, submit waits for the game to finish and write a match result file. If the bots fail to start, submit records an initialization error itself. The first result written to the match result file is kept — any later write attempt (e.g. from a game controller retry) is ignored.

After the match finishes, submit collects logs, replays, and bot data and submits everything to the AI Arena API.

## Retry policy

All GraphQL and store operations use the same retry policy: up to 10 attempts with 60 seconds between retries. Both server errors (5xx) and connection failures are retried. Any other non-success response fails immediately without retrying.

Cache operations are single-attempt with no retry. A failed cache download falls back to fetching directly from the store. A cache upload failure is logged and the store upload proceeds regardless.
