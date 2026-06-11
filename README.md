# World Cup Bot

Discord bot that tracks the FIFA World Cup for your server. Each member can claim one or more national teams; when any claimed team’s match finishes, the bot awards points and posts an announcement in a configured channel.

## Setup

1. Create a [Discord application](https://discord.com/developers/applications) and bot token.
2. Get a free API token from [football-data.org](https://www.football-data.org/client/register).
3. Copy `.env.example` to `.env` and fill in the values.
4. Invite the bot to your server. In the [Discord Developer Portal](https://discord.com/developers/applications) → **OAuth2** → **URL Generator**, select:
   - **Scopes:** `bot`, `applications.commands`
   - **Bot permissions:** View Channels, Send Messages, Embed Links, Create Public Threads, Send Messages in Threads

   Open the generated URL to add the bot. It must be able to send messages (including embeds) in the channel you set with `/config channel`.

   For `$` prefix commands (e.g. `$claim Brazil`), also enable **Message Content Intent** under **Bot** → **Privileged Gateway Intents**.

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

Prefix commands use `$` as the prefix (e.g. `$claim Brazil`).

## Commands

| Command | Description |
|---------|-------------|
| `/config channel` | Set the announcement channel (requires Manage Server) |
| `/claim` | Claim a nation for yourself by name or code (e.g. Brazil, BRA) |
| `/assign` | Claim a nation for another member |
| `/unclaim` | Remove a claimed team by name or code (e.g. Brazil, BRA) |
| `/team` | Show your claimed teams and tie-breaker player |
| `/pick-player` | Designate a tie-breaker player from your claimed teams' squads |
| `/teams` | List all team assignments |
| `/unclaimed` | List teams not yet claimed |
| `/standings` | Leaderboard |
| `/help` | List commands (optional: `/help claim` for details) |
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

The bot polls football-data.org every 5 minutes for finished World Cup (`WC`) matches and scorer totals. Announcements are only sent after `/config channel` has been set.

### Tie-breaker

If two players finish with the same total points, the one whose designated player has scored more goals in the tournament ranks higher. Use `/pick-player` to choose one player from your claimed teams' squads. If you don't pick, tie-breaker goals count as 0. Tie-breaker goals do not add to your score — they only break ties on the leaderboard.

Player squads and scorer data require a football-data.org plan that includes deep data (squads and goal scorers).

## Releasing

`Cargo.toml` is the source of truth for the version. Git tags must match exactly (`v0.1.0` ↔ `version = "0.1.0"`). The release workflow rejects mismatched tags.

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

Pushing a `v*` tag triggers GitHub Actions to run tests, build Linux and macOS binaries with Nix, and publish a GitHub Release with attached archives.

First release at the current version (no bump):

```bash
cargo release release --execute
git push origin main --follow-tags
```
