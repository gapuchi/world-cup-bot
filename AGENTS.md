# Agent guide

This document describes how the codebase is organized and the conventions to follow when making changes.

## Project overview

Discord bot (Rust, [poise](https://github.com/serenity-rs/poise) + serenity) that runs prediction pools for sports leagues. World Cup is live; NFL is planned. Data is stored in SQLite; match data comes from [football-data.org](https://www.football-data.org/).

## Module layout

```
src/
  main.rs           # Entry point: env, DB init, Discord client, poller spawn
  lib.rs            # Module exports only
  types.rs          # Shared bot state (`Data`, `Context`, `Error`)
  commands.rs       # Poise slash/prefix commands
  poller.rs         # Background job: polls all pools, posts announcements
  scoring.rs        # Match points (win/draw/loss)
  standings.rs      # Leaderboard ranking and tie-breakers
  soccar.rs         # Domain logic on soccer API data (not HTTP)
  api/
    mod.rs          # Thin re-export of API clients
    football_data.rs # football-data.org HTTP client + DTOs
  db/
    mod.rs          # `init()` + public re-exports (no business logic)
    migrate.rs      # Schema migrations
    <entity>.rs     # Shared entities (pool, league, registration, …)
    wc/             # World Cup gameplay tables
      match_result.rs
      processed_match.rs
      player_goal_total.rs
      tiebreaker_pick.rs
tests/              # Integration tests (migrate, standings, soccar helpers)
```

## Layer boundaries

### `src/api/` — external HTTP clients only

- Holds `FootballDataApi`, `ApiError`, and serde DTOs (`Team`, `Match`, `SquadPlayer`, `Scorer`, …).
- Methods map 1:1 to football-data.org endpoints.
- No fuzzy search, squad filtering, score interpretation, or Discord/DB logic.
- Adding a new data source → new file under `src/api/` (e.g. `nfl_data.rs`), re-export from `mod.rs`.

### `src/soccar.rs` — soccer domain logic

Lives **outside** `api/` because it is app logic, not the remote API:

- `full_time_score` — when a finished match has both goal counts
- `find_team` / `find_players` — user-facing name lookup
- `fetch_squads_for_teams` — multi-team squad aggregation + role filtering
- `SquadPlayerMatch` — tie-breaker pick domain type

### `src/db/` — persistence accessors

Refactored to one module per entity. Shared catalog/pool modules live at `db/` root; league-specific gameplay tables live in subfolders (`db/wc/`, future `db/nfl/`). `db/mod.rs` only runs migrations and re-exports types.

- DB modules expose `get` / `list` / `upsert` / `delete` style methods on `&Connection`.
- No Discord or HTTP calls inside `db/`.
- WC-specific tables and accessors live under `db/wc/` (table names still use `wc_` prefix).

### Commands and poller

- **Commands** resolve the **active** pool via `Pool::active()` and read competition from the pool's season (`external_season_id`).
- **Poller** processes **all** pools, grouped by `external_season_id`, dispatched by `league_slug`.

## Multi-league pools

Introduced in the "Multi League Support" refactor. Key concepts:

| Concept | Where | Purpose |
|---------|-------|---------|
| `active_pool_id` | `bot_config` | Which pool `/claim`, `/standings`, etc. target |
| `Pool::active()` | `db/pool.rs` | Resolves active pool for commands |
| `Pool::list_all_with_meta()` | `db/pool.rs` | All pools for the poller |
| `PoolMeta` | `db/pool.rs` | Pool + league slug + `external_season_id` |
| `/config league <slug>` | `commands.rs` | Switch active pool |

Rules:

- `pool_id` remains the stable key for registrations, match results, tie-breakers, and announcements.
- Switching active league changes which pool commands see; each league keeps its own pool data.
- Poller dispatches by `league_slug` (`"wc"` implemented, `"nfl"` is a no-op seam).
- Do not hardcode `"WC"` or a fixed pool id outside migration defaults.

## Accessing the soccer API

Use the helper on bot state — do not thread `http` + `api_token` separately:

```rust
// In commands (via poise context)
let teams = ctx.data().soccar_api().fetch_teams(&competition).await?;

// In poller / other code with &Data
let matches = data.soccar_api().fetch_finished_matches(competition).await?;
```

Import types from `crate::api`, domain helpers from `crate::soccar`.

## Coding conventions

- **Small, focused diffs** — match existing style in the file you edit.
- **One idea per unit** — short functions; split when logic spans layers.
- **Tests for real behavior** — `tests/migrate.rs`, `tests/standings.rs`, `tests/api.rs` (soccar score helpers). Don't add trivial tests.
- **Errors** — API layer uses `ApiError`; commands use `types::Error` (`Box<dyn Error + Send + Sync>`).
- **Releases** — version lives in `Cargo.toml`; see `README.md` for `cargo-release` workflow.
- **README** — keep `README.md` accurate when you change user-facing behavior: commands, config/setup, scoring or tie-breaker rules, environment variables, permissions, or release workflow. User docs live in `README.md`; architecture and agent guidance live here.

## Common tasks

| Task | Where to change |
|------|-----------------|
| New slash command | `commands.rs`, register in `main.rs`, update `README.md` |
| New DB table | `db/migrate.rs` + new module under `db/` or `db/<league>/` + re-export in `db/mod.rs` |
| New football-data endpoint | `api/football_data.rs` + domain logic in `soccar.rs` if needed |
| New league poller | `poller.rs` match arm on `league_slug` + `db/league.rs` catalog |
| Leaderboard / tie-breaker rules | `standings.rs`, `scoring.rs`; update `README.md` if rules change |
| User-facing setup or config | `README.md` (env vars, permissions, `/config` subcommands) |

## What not to do

- Put search, filtering, or orchestration in `src/api/`.
- Put HTTP or Discord logic in `src/db/`.
- Bypass `Pool::active()` in gameplay commands.
- Re-introduce a monolithic `db/mod.rs` with all SQL inline.
- Pass raw `reqwest::Client` + token when `soccar_api()` exists.

## Running checks

```bash
cargo test
cargo clippy -- -D warnings   # if touching Rust code
```
