mod guild_config;
mod league;
mod migrate;
mod pool;
mod registration;
mod season;
mod team;
mod wc;

use rusqlite::Connection;

pub use guild_config::GuildConfig;
pub use league::{exists as league_exists, supports_pool as league_supports_pool};
pub use pool::{Pool, PoolMeta};
pub use registration::Registration;
pub use season::{Season, SeasonDisplay};
pub use migrate::{LEGACY_GUILD_ID, SCHEMA_VERSION};
pub use wc::{
    WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
};

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    migrate::run(conn)
}
