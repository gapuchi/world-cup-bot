# Agent guide

## Related docs

- [`src/db/README.md`](src/db/README.md) — schema and entities
- [`.cursor/rules/`](.cursor/rules/) — scoped reminders (`api-layer`, `db-layer`, `readme-sync`)

## Project overview

Discord bot (Rust, poise + serenity) for sports prediction pools. World Cup and NFL are live; NBA planned. SQLite persistence; match data from [football-data.org](https://www.football-data.org/) (WC) and ESPN's public site API (NFL).

## Layer boundaries

Strict layers — details when editing matching paths are in `.cursor/rules/`:

| Layer | Role |
|-------|------|
| `src/api/` | HTTP clients + serde DTOs only; 1:1 with endpoints |
| `src/soccar.rs` | Soccer domain logic on API data (search, scores, squads) |
| `src/gridiron.rs` | NFL domain logic on ESPN data (search, scores, rosters) |
| `src/db/` | Persistence accessors; one module per entity; no HTTP/Discord |
| Use cases (`registration.rs`, `draft.rs`, `wc/`, `nfl/`, `standings.rs`, `scoring.rs`, `poller.rs`) | Orchestration, game rules, background jobs |
| `src/commands/` | Thin Discord adapters only |

**Command adapters vs use cases** — commands handle poise attrs, `guild_id`, defer/reply; use cases resolve season, call `db/` and `soccar_api()`, return strings.

| Concern | Adapter | Use case |
|---------|---------|----------|
| Registration | `commands/registration.rs` | `registration.rs` |
| WC gameplay | `commands/wc/` | `wc/` |
| NFL gameplay | `commands/wc/` (shared commands) | `nfl/` |
| Standings | `commands/wc/standings.rs` | `standings.rs` |
| Draft | `commands/wc/draft.rs` | `draft.rs` |
| Config | `commands/config.rs` | inline DB (small, admin-only) |

New behavior: use case first → thin handler in `commands/` → `commands::all()`.

```rust
// Adapter calls use case; use case owns API + DB + rules
let (message, _) = registration::pick_for_user(ctx.data(), guild_id, user_id, &team).await?;
ctx.say(message).await?;
```

**Poller** — `poller.rs` processes all seasons (every guild), grouped by `league_slug`; not a command.

## Seasons (multi-guild, multi-league)

Tenancy is at **season** (`seasons.guild_id`). Each guild has a `default_season_id` in `guild_config`.

- Gameplay commands: `Season::default_for_guild(conn, guild_id)` — pass invoking `ctx.guild_id()`
- Poller: `Season::list_all_with_meta()` for all guilds
- Setup: `/config season` creates a season; fresh guilds have none until then
- `season_id` keys registrations, results, tie-breakers, announcements
- Do not hardcode guild or season ids
- League switch (`/config league`) changes which season commands see; data per league stays separate

## Key patterns

- `ctx.data().soccar_api()` or `data.soccar_api()` — never pass `http` + `api_token` separately
- `ctx.data().espn_api()` or `data.espn_api()` — NFL; no token
- Types from `crate::api`; domain helpers from `crate::soccar` or `crate::gridiron`
- Competition code from `league_competition_code()` via league slug (WC only)
- NFL seasons use `season.season_year` for ESPN queries

## Repo conventions

- **Tests** — `tests/migrate.rs`, `tests/standings.rs`, `tests/api.rs`
- **Errors** — `ApiError` in api; `types::Error` in commands
- **Releases** — `Cargo.toml`; `cargo release` or `just release`

## Common tasks

| Task | Where |
|------|-------|
| New command | Use case module → `commands/` handler → `commands::all()` → docstring |
| New DB table | `migrate.rs` + `db/` or `db/<league>/` module → re-export in `db/mod.rs` |
| New API endpoint | `api/football_data.rs` or `api/espn.rs` + domain module if needed |
| New league poller | `poller.rs` match on `league_slug` + `db/<league>/` |
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
