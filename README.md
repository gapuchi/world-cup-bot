# World Cup Bot

Discord bot for sports prediction pools. Each member can claim one or more teams; when a claimed team's match finishes, the bot awards points and posts an announcement in a configured channel.

The bot can serve **multiple Discord servers** at once. Each server has its own team claims, standings, announcement channel, and league selection. World Cup (`wc`) is fully supported today. The database also catalogs other leagues (e.g. NFL), but only World Cup pools are playable for now.

## Setup

1. Create a [Discord application](https://discord.com/developers/applications) and bot token.
2. Get a free API token from [football-data.org](https://www.football-data.org/client/register).
3. Copy `.env.example` to `.env` and fill in the values.
4. Invite the bot to your server. In the [Discord Developer Portal](https://discord.com/developers/applications) → **OAuth2** → **URL Generator**, select:
   - **Scopes:** `bot`, `applications.commands`
   - **Bot permissions:** View Channels, Send Messages, Embed Links, Create Public Threads, Send Messages in Threads

   Open the generated URL to add the bot. It must be able to send messages (including embeds) in the channel you set with `/config channel`.

### Local development

```bash
cargo run
```

The bot loads environment variables from `.env` via dotenvy.

Slash commands are registered automatically in each guild the bot joins on startup. If commands don't appear, run `/register` in that server.

### Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | yes | Discord bot token |
| `FOOTBALL_DATA_API_TOKEN` | yes | football-data.org API token |
| `DATABASE_PATH` | no | SQLite database path (default: `world_cup.db`) |

## Configuration

Each Discord server configures the bot independently. On a **new** server, an admin must create a season pool first — there is no default until `/config season` has been run in that server.

| Command | Description |
|---------|-------------|
| `/config season <league> <slug> <name>` | Create a season and pool for this server (e.g. `wc`, `wc-2026`, `World Cup 2026`; requires Manage Server). Sets the new pool as default. |
| `/config league <slug>` | Set the default league pool for this server (e.g. `wc`; requires Manage Server). Pool must already exist. |
| `/config leagues` | List league pools in this server and which one is default (requires Manage Server) |
| `/config channel` | Set the announcement channel for the **default** pool in this server (requires Manage Server) |

Match announcements are only sent after `/config channel` has been set for that pool. Each pool keeps its own channel, registrations, scores, and tie-breaker picks. Gameplay commands (`/claim`, `/standings`, etc.) always target the default pool for the server where the command was run.

Use `/season` to see which league and season commands currently target in this server.

### Upgrading an existing deployment

If you already ran the bot on a single server before multi-guild support, your registrations, standings, and config are migrated automatically on startup. No manual steps are required. Additional servers you add later start empty until an admin runs `/config season` in each one.

## Commands

| Command | Description |
|---------|-------------|
| `/claim` | Claim a nation for yourself by name or code (e.g. Brazil, BRA) |
| `/assign` | Claim a nation for another member |
| `/unclaim` | Remove a claimed team by name or code |
| `/team` | Show your claimed teams and tie-breaker player |
| `/pick-player` | Designate a tie-breaker player from your claimed teams' squads |
| `/teams` | List all team assignments |
| `/unclaimed` | List teams not yet claimed |
| `/standings` | Leaderboard (summary embed plus a thread with per-team breakdown) |
| `/season` | Show the default league and season for this server |
| `/help` | List commands (optional: `/help claim` for details) |
| `/version` | Show the bot version |
| `/ping` | Health check |
| `/register` | Re-register slash commands |

Each nation can only be claimed by one person at a time. A person can claim multiple nations.

## Scoring

Points are awarded per match based on the result for each claimed team:

| Result | Points |
|--------|--------|
| Win    | 3      |
| Draw   | 1      |
| Loss   | 0      |

The background poller runs every 5 minutes. It processes **all** configured league pools across every server, fetching finished matches and scorer totals from football-data.org per league (World Cup pools use the `WC` competition, derived from the `wc` league slug). Only pools with an announcement channel configured receive Discord posts.

### Tie-breaker

If two players finish with the same total points, the one whose designated player has scored more goals in the tournament ranks higher. Use `/pick-player` to choose one player from your claimed teams' squads. If you don't pick, tie-breaker goals count as 0. Tie-breaker goals do not add to your score — they only break ties on the leaderboard.

Player squads and scorer data require a football-data.org plan that includes deep data (squads and goal scorers).

## Releasing

`Cargo.toml` is the source of truth for the version. Git tags must match exactly (`v0.1.4` ↔ `version = "0.1.4"`). The release workflow rejects mismatched tags.

Install [cargo-release](https://github.com/crate-ci/cargo-release) once:

```bash
cargo install cargo-release
```

Preview a release, then execute it (bumps `Cargo.toml`, commits, and creates a tag; does not push):

```bash
cargo release patch          # dry run
cargo release patch --execute
git push origin main --follow-tags
```

Or use the [just](https://github.com/casey/just) recipes:

```bash
just release-dry patch
just release patch
```

Pushing a `v*` tag triggers GitHub Actions to run tests, build Linux and macOS binaries with Nix, and publish a GitHub Release with attached archives.
