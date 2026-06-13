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
  registration.rs   # Team claim/unclaim use cases (DB + soccer API)
  wc/               # World Cup gameplay use cases (tie-breaker pick, season)
  commands/         # Poise slash/prefix handlers (thin Discord adapters)
    mod.rs          # `all()` command registry
    meta.rs         # ping, version, help, register
    config.rs       # `/config` subcommands
    registration.rs # claim, assign, unclaim, teams, …
    wc/             # pick-player, standings, season
    nfl/            # placeholder for future NFL commands
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
    guild_config.rs # Per-guild default pool
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

- **Commands** resolve the invoking guild's **default** pool via `Pool::default_for_guild(conn, guild_id)`. API competition codes are derived from league slug via `league_competition_code()`. Pass `ctx.guild_id()` from guild-only commands.
- **Poller** processes **all** pools (every guild), grouped by `league_slug`.

## Multi-guild pools

Each Discord guild has independent gameplay data. Tenancy is scoped at **pool** (`pools.guild_id`).

| Concept | Where | Purpose |
|---------|-------|---------|
| `guild_id` | `pools` | Which Discord server owns the pool |
| `default_pool_id` | `guild_config` | Which pool `/claim`, `/standings`, etc. target in that guild |
| `Pool::default_for_guild()` | `db/pool.rs` | Resolves default pool for a guild |
| `Pool::get_or_create_for_season(conn, guild_id, season_id)` | `db/pool.rs` | Creates pool for guild + season on demand |
| `Pool::get_for_guild_league(conn, guild_id, slug)` | `db/pool.rs` | Latest pool for a guild + league |
| `Pool::list_with_league(conn, guild_id)` | `db/pool.rs` | Pools configured in one guild |
| `Pool::list_all_with_meta()` | `db/pool.rs` | All pools for the poller |
| `/config season` | `commands/config.rs` | Create season + pool for invoking guild |
| `/config league <slug>` | `commands/config.rs` | Set default pool for invoking guild |

Rules:

- `pool_id` remains the stable key for registrations, match results, tie-breakers, and announcements.
- Fresh guilds have no pools until `/config season` runs (no bootstrap pool or season on new installs).
- `leagues` are seeded at migration; `seasons` and pools are created per deployment via `/config season`.
- Do not hardcode guild or pool ids in commands.

## Multi-league pools

Introduced in the "Multi League Support" refactor. Key concepts:

| Concept | Where | Purpose |
|---------|-------|---------|
| `default_pool_id` | `guild_config` | Per-guild default; set by `/config league` or `/config season` |
| `Pool::get_or_create_for_season()` | `db/pool.rs` | Creates pool for guild + season on demand |
| `PoolMeta` | `db/pool.rs` | Pool + league slug + name |
| `/config season` | `commands/config.rs` | Create season + pool for invoking guild |
| `/config league <slug>` | `commands/config.rs` | Switch default pool within a guild |

Rules:

- `pool_id` remains the stable key for registrations, match results, tie-breakers, and announcements.
- Switching default league changes which pool commands see in that guild; each league keeps its own pool data per guild.
- Poller dispatches by `league_slug` (`"wc"` implemented, `"nfl"` is a no-op seam).
- Do not hardcode a fixed pool id outside command-driven setup.

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
- **Releases** — version lives in `Cargo.toml`; use `cargo release` or `just release` (see `justfile`).
- **README** — keep `README.md` accurate when you change user-facing behavior: config/setup, scoring or tie-breaker rules, environment variables, or permissions. Command details live in docstrings and `/help`. User docs live in `README.md`; architecture and agent guidance live here.

## Common tasks

| Task | Where to change |
|------|-----------------|
| New slash command | `commands/` (or `registration.rs` / `wc/` for logic), add to `commands::all()`, write a clear docstring for `/help` |
| New DB table | `db/migrate.rs` + new module under `db/` or `db/<league>/` + re-export in `db/mod.rs` |
| New football-data endpoint | `api/football_data.rs` + domain logic in `soccar.rs` if needed |
| New league poller | `poller.rs` match arm on `league_slug` + `db/league.rs` catalog |
| Leaderboard / tie-breaker rules | `standings.rs`, `scoring.rs`; update `README.md` if rules change |
| User-facing setup or config | `README.md` (env vars, permissions, non-obvious config behavior) |

## What not to do

- Put search, filtering, or orchestration in `src/api/`.
- Put HTTP or Discord logic in `src/db/`.
- Bypass `Pool::default_for_guild()` in gameplay commands — always pass the invoking guild id.
- Re-introduce a monolithic `db/mod.rs` with all SQL inline.
- Pass raw `reqwest::Client` + token when `soccar_api()` exists.

## Running checks

```bash
cargo test
cargo clippy -- -D warnings   # if touching Rust code
```
