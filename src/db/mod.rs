mod guild_config;
mod league;
mod migrate;
mod registration;
mod season;
mod team;
mod wc;

use rusqlite::Connection;

pub use guild_config::GuildConfig;
pub use league::{competition_code as league_competition_code, exists as league_exists, supports_season as league_supports_season};
pub use registration::Registration;
pub use season::{Season, SeasonDisplay, SeasonMeta};
pub use migrate::SCHEMA_VERSION;
pub use wc::{
    WcAnnouncedElimination, WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
};

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    migrate::run(conn)
}
