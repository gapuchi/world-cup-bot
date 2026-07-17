# Agent guide

## Related docs

- [`src/db/README.md`](src/db/README.md) — schema and entities
- [`.cursor/rules/`](.cursor/rules/) — scoped reminders (`api-layer`, `db-layer`, `readme-sync`)

## Project overview

Discord bot (Rust, poise + serenity) for sports prediction pools. World Cup is live; NFL planned. SQLite persistence; match data from [football-data.org](https://www.football-data.org/).

## Layer boundaries

Strict layers — details when editing matching paths are in `.cursor/rules/`:

| Layer | Role |
|-------|------|
| `src/api/` | HTTP clients + serde DTOs only; 1:1 with endpoints |
| `src/soccar.rs` | Soccer domain logic on API data (search, scores, squads) |
| `src/db/` | Persistence accessors; one module per entity; no HTTP/Discord |
| Use cases (`registration.rs`, `wc/`, `standings.rs`, `scoring.rs`, `poller.rs`) | Orchestration, game rules, background jobs |
| `src/commands/` | Thin Discord adapters only |

**Command adapters vs use cases** — commands handle poise attrs, `guild_id`, defer/reply; use cases resolve season, call `db/` and `soccar_api()`, return strings.

| Concern | Adapter | Use case |
|---------|---------|----------|
| Registration | `commands/registration.rs` | `registration.rs` |
| WC gameplay | `commands/wc/` | `wc/` |
| Standings | `commands/wc/standings.rs` | `standings.rs` |
| Config | `commands/config.rs` | inline DB (small, admin-only) |

New behavior: use case first → thin handler in `commands/` → `commands::all()`.

```rust
// Adapter calls use case; use case owns API + DB + rules
let message = registration::claim_for_user(ctx.data(), guild_id, user_id, &team).await?;
ctx.say(message).await?;
```

**Poller** — `poller.rs` processes all seasons (every guild), grouped by `league_slug`; not a command.

## Leagues vs seasons

| Term | Meaning |
|------|---------|
| **League** | Compile-time competition type (`League` enum in `src/league.rs`, slug e.g. `wc`). Adding a league is a code change. |
| **Season** | Runtime instance of a league for one guild (`season_id`). Created via `/config season`. |

Catalog rows may exist in `leagues` for future slugs; only variants on `League` can have seasons (`League::supports_season` / `from_slug`).

## Seasons (multi-guild, multi-league)

Tenancy is at **season** (`seasons.guild_id`). Each guild has a `default_season_id` in `guild_config` (**command focus** — which season slash commands use).

- Gameplay commands: `Season::default_for_guild(conn, guild_id)` — pass invoking `ctx.guild_id()`
- Poller: `Season::list_live_with_meta()` — seasons with `polling_enabled` (independent of command focus)
- Setup: `/config season` creates a season for a compiled-in league; fresh guilds have none until then
- `season_id` keys registrations, results, tie-breakers, announcements
- Do not hardcode guild or season ids
- League switch (`/config league`) changes command focus; data per league stays separate

## Key patterns

- `ctx.data().soccar_api()` or `data.soccar_api()` — never pass `http` + `api_token` separately
- Types from `crate::api`; domain helpers from `crate::soccar`
- Competition code from `league_competition_code()` via league slug

## Repo conventions

- **Tests** — `tests/migrate.rs`, `tests/standings.rs`, `tests/api.rs`
- **Errors** — `ApiError` in api; `types::Error` in commands
- **Releases** — `Cargo.toml`; `cargo release` or `just release`

## Common tasks

| Task | Where |
|------|-------|
| New command | Use case module → `commands/` handler → `commands::all()` → docstring |
| New DB table | `migrate.rs` + `db/` or `db/<league>/` module → re-export in `db/mod.rs` |
| New API endpoint | `api/football_data.rs` + `soccar.rs` if needed |
| New league poller | `poller.rs` match on `league_slug` + `db/league.rs` |
| Scoring / tie-breakers | `scoring.rs`, `standings.rs`; update `README.md` if user-visible |
| Setup / config UX | `README.md` (see `readme-sync.mdc`) |

## What not to do

- Search, filtering, or orchestration in `api/`
- Business logic in `commands/`
- HTTP or Discord in `db/`
- Bypass `Season::default_for_guild()` in gameplay commands
- Monolithic `db/mod.rs` with inline SQL
- Raw `reqwest::Client` + token when `soccar_api()` exists

## Running checks

```bash
cargo test
cargo clippy -- -D warnings
```
