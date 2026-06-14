# World Cup Bot

Discord bot for sports prediction pools. Each member is assigned teams through a snake draft; when an assigned team's game finishes, the bot awards points and posts an announcement in a configured channel.

The bot can serve **multiple Discord servers** at once. Each server has its own draft, team assignments, standings, announcement channel, and league selection. World Cup and NFL are supported today; NBA is planned.

## Setup

1. Create a [Discord application](https://discord.com/developers/applications) and bot token.
2. Get a free API token from [football-data.org](https://www.football-data.org/client/register) (World Cup only).
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
| `FOOTBALL_DATA_API_TOKEN` | yes | football-data.org API token (World Cup) |
| `DATABASE_PATH` | no | SQLite database path (default: `world_cup.db`) |

NFL data comes from ESPN's public site API and needs no extra token.

## Configuration

Each Discord server configures the bot independently. On a **new** server, an admin must create a season first — there is no default until `/config season` has been run in that server. Run `/help config` for subcommands.

`/config season` requires a **season year** (e.g. `2026` for World Cup 2026, `2025` for NFL 2025) so the poller knows which calendar year to fetch.

Match announcements are only sent after `/config channel` has been set for that season. Each season keeps its own channel, registrations, scores, and tie-breaker picks. Gameplay commands always target the default season for the server where the command was run.

Use `/season` to see which league and season commands currently target in this server.

## Commands

Run `/help` to list all commands, or `/help <command>` for details (e.g. `/help draft`).

## Leagues

### World Cup (`wc`)

Admins run a **snake draft** to assign nations. Each nation can only be assigned to one person; a person can hold multiple nations (one per draft round). When an assigned team's match finishes, the bot awards points and posts an announcement in the configured channel.

**Draft** — an admin starts with `/draft start <rounds> @participants`. Pick order is randomized. Only the member on the clock can use `/draft pick`. Admins can `/draft skip`, `/assign`, `/unassign`, or `/draft cancel` (cancelling removes all assignments). When all picks are in, rosters lock.

**Scoring** — points per match based on the result for each assigned team:

| Result | Points |
|--------|--------|
| Win    | 3      |
| Draw   | 1      |
| Loss   | 0      |

**Tie-breaker** — if two players finish with the same total points, the one whose designated player has scored more goals in the tournament ranks higher. Use `/pick-player` to choose one player from your assigned teams' squads. If you don't pick, tie-breaker goals count as 0. Tie-breaker goals do not add to your score — they only break ties on the leaderboard.

The background poller runs every 5 minutes, fetching finished matches and scorer totals from football-data.org (`WC` competition). Player squads and scorer data require a football-data.org plan that includes deep data (squads and goal scorers).

### NFL (`nfl`)

Same draft flow as World Cup. `/draft start` defaults to **1 round** (one team per person) when the active season is NFL; admins can pass a higher round count to override.

**Scoring** — points per finished game for each assigned team:

| Result | Points |
|--------|--------|
| Win    | 1      |
| Tie    | ½      |
| Loss   | 0      |

**Tie-breaker** — total **touchdowns** by your designated player (regular season + playoffs). Use `/pick-player` to choose from your team's roster.

The poller fetches finished games and touchdown leaders from ESPN (regular season and playoffs). No API key required.

### NBA (`nba`)

Coming soon.
