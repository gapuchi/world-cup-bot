# Agent guide

## Related docs

- [`src/db/README.md`](src/db/README.md) — schema and entities
- [`docs/plans/multi-sport-framework/plan.md`](docs/plans/multi-sport-framework/plan.md) — multi-league framework plan
- [`.cursor/rules/`](.cursor/rules/) — scoped reminders (`api-layer`, `db-layer`, `readme-sync`)

## Project overview

Discord bot (Rust, poise + serenity) for sports prediction pools. World Cup is live; NFL planned. SQLite persistence; match data from [football-data.org](https://www.football-data.org/).

## Layer boundaries

Strict layers — details when editing matching paths are in `.cursor/rules/`:

| Layer | Role |
|-------|------|
| `src/api/` | HTTP clients + serde DTOs only; 1:1 with endpoints |
| `src/soccar.rs` | Soccer domain helpers on API data (used by the `wc` league module) |
| `src/db/` | Persistence accessors; shared entities at root; league tables under `db/<slug>/` |
| `src/league.rs` | Compile-time `League` enum — host dispatch face into league modules |
| League modules (`src/wc/`, …) | API client, poll, standings, tie-break, league-only use cases |
| Host use cases (`registration.rs`, `standings.rs` formatters, `poller.rs`) | Shared orchestration via `League` |
| `src/commands/` | Thin Discord adapters only |

**Command adapters vs use cases** — commands handle poise attrs, `guild_id`, defer/reply; use cases resolve season, dispatch via `League`, call `db/` / league modules, return strings.

| Concern | Adapter | Use case |
|---------|---------|----------|
| Registration | `commands/registration.rs` | `registration.rs` → `League` |
| Shared standings | `commands/wc/standings.rs` | `League::standings` → `wc/standings` (format helpers in `standings.rs`) |
| WC-only cmds | `commands/wc/` (`remaining`, `pick-player`) | `wc/` (registered via `commands_for(League)`) |
| Config | `commands/config.rs` | inline DB (small, admin-only) |

New behavior: use case first → thin handler in `commands/` → `commands::all()`.

```rust
// Adapter calls use case; use case owns API + DB + rules via League
let message = registration::claim_for_user(ctx.data(), guild_id, user_id, &team).await?;
ctx.say(message).await?;
```

**Poller** — `poller.rs` lists **live** seasons, groups by league slug, calls `League::poll`; not a command.

## Leagues vs seasons

| Term | Meaning |
|------|---------|
| **League** | Compile-time competition type (`League` enum in `src/league.rs`, slug e.g. `wc`). Adding a league is a code change. |
| **Season** | Runtime instance of a league for one guild (`season_id`). Created via `/config season`. |
| **Command focus** | Guild’s `default_season_id` — which season slash commands use. |
| **Live season** | Season with `polling_enabled` — which seasons the poller processes (independent of focus). |

Catalog rows may exist in `leagues` for future slugs; only variants on `League` can have seasons (`League::supports_season` / `from_slug`).

## Seasons (multi-guild, multi-league)

Tenancy is at **season** (`seasons.guild_id`).

- Gameplay commands: `Season::default_for_guild` / `League::for_guild` — pass invoking `ctx.guild_id()`
- Poller: `Season::list_live_with_meta()` then `League::from_slug` → `poll`
- Setup: `/config season` creates a season for a compiled-in league; fresh guilds have none until then
- `season_id` keys registrations, results, tie-breakers, announcements
- Do not hardcode guild or season ids
- `/config league` changes command focus; data per league stays separate

## Key patterns

- Resolve the focused season’s league with `League::for_guild` / `League::for_season`, then call enum methods (`list_teams`, `standings`, `poll`, …)
- `Data` holds `db` + shared `http`; league modules own their API clients/tokens (e.g. `wc::football_data`)
- Types from `crate::api`; soccer domain helpers from `crate::soccar` (wc module)
- Competition code from `league_competition_code()` via league slug
- League-specific slash commands: exhaustive `commands_for(League)` in `commands/mod.rs`

## Adding a league (checklist)

1. Add `League` variant + `from_slug` / `slug` / `display_name` arms in `src/league.rs`
2. Implement league module (`src/<slug>/`) with teams, standings, poll, optional tie-break
3. Wire enum method arms (`list_teams`, `standings`, `poll`, …)
4. Add `db/<slug>/` accessors if new tables (schema already has `nba_*` / `nfl_*` stubs)
5. Register league-only commands in `commands_for` match
6. Add env vars to `.env.example` / README; fail-fast in `main` if required
7. Seed or keep catalog row in `migrate.rs` `seed_catalog`

## Repo conventions

- **Tests** — `tests/migrate.rs`, `tests/standings.rs`, `tests/api.rs`
- **Errors** — `ApiError` in api; `types::Error` in commands
- **Releases** — `Cargo.toml`; `cargo release` or `just release`

## Common tasks

| Task | Where |
|------|-------|
| New command | Use case → `commands/` handler → `commands::all()` or `commands_for(League)` → docstring |
| New DB table | `migrate.rs` + `db/` or `db/<league>/` → re-export in `db/mod.rs` |
| New API endpoint | `api/…` + league module helpers as needed |
| New league poller | `League::poll` arm + league module `poll` (host `poller.rs` stays generic) |
| Scoring / tie-breakers | league module + shared `scoring` helpers; update `README.md` if user-visible |
| Setup / config UX | `README.md` (see `readme-sync.mdc`) |

## What not to do

- Search, filtering, or orchestration in `api/`
- Business logic in `commands/`
- HTTP or Discord in `db/`
- Bypass `Season::default_for_guild()` / `League::for_guild` in gameplay commands
- Hard-wire `Wc*` types into shared host paths (`registration`, host `standings`, `types`, `db/registration`)
- Monolithic `db/mod.rs` with inline SQL
- Raw `reqwest::Client` + token in host code when a league module client helper exists
- Assume command focus controls the poller (use `polling_enabled` / live seasons)

## Running checks

```bash
cargo test
cargo clippy -- -D warnings
```
