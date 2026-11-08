# dbserver — DOWNBEAT online

One binary. Every score is a replay: the server re-scores it with the same core the game runs, inspects it,
and ranks it. Nothing a client says about its score is trusted.

```
cd server
cargo test                                            # 10 tests: the API in process with bot replays
cargo run --release -- --bind 0.0.0.0:8787 --db downbeat.db --levels ../godot/levels
```

Put it behind a TLS reverse proxy (Caddy, nginx); it speaks plain HTTP. `--levels` points at the shipped
level files, which define the ranked set together with the builtins at every whole BPM from 60 to 220 and
every chart-changing mod combination (about 3000 charts).

| Route | What |
|---|---|
| `GET /v1/health` | ok, ranked chart count, score count, today's date (UTC) |
| `POST /v1/players {name}` | register: `{id, token}`; the client sends `Authorization: Bearer id:token` |
| `POST /v1/scores {replay}` | re-score, inspect, store; answers with the server's numbers and the rank |
| `GET /v1/boards/{fingerprint}?mods=NM&limit=50` | best complete score per player, clean first; `ranked` says whether the chart is in the set |
| `GET /v1/replays/{id}` | the stored replay (ghosts) |
| `GET /v1/daily?date=` | the day's chart: the level JSON as charted and its mods (deterministic from the date) |
| `POST /v1/daily/scores {replay}` | a score on today's (or, across midnight, yesterday's) daily |
| `GET /v1/daily/board?date=` | the daily board |
| `GET /v1/players/{id}` | name, profile rating (osu-weighted best clean ranked pp per chart), plays |
| `GET /v1/levels/{name}` | a shipped level file |

Rules: a replay whose chart fingerprint does not match the level it carries is refused (422). An abandoned
run (replay ends before the chart does) is stored but never ranked. A flagged replay (timing SD under 3 ms,
precision over 0.995, sixteenth-quantised cadence, inputs out of order) is kept, shown with its flags,
ranked under every clean score and left out of ratings. A chart outside the ranked set gets its own board,
marked unranked. Boards hold one entry per player: their best.

The token is a name attached to scores, not a security boundary; the re-scoring is the anti-cheat.
