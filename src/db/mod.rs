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
pub use league::{competition_code as league_competition_code, exists as league_exists, supports_pool as league_supports_pool};
pub use pool::{Pool, PoolMeta};
pub use registration::Registration;
pub use season::{Season, SeasonDisplay};
pub use migrate::SCHEMA_VERSION;
pub use wc::{
    WcMatchResult, WcPlayerGoalTotal, WcProcessedMatch, WcTiebreakerPick,
};

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    migrate::run(conn)
}
