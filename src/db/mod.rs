mod bot_config;
mod league;
mod migrate;
mod pool;
mod registration;
mod season;
mod team;
mod wc_match_result;
mod wc_player_goal_total;
mod wc_processed_match;
mod wc_tiebreaker_pick;

use rusqlite::Connection;

pub use bot_config::BotConfig;
pub use league::{exists as league_exists, supports_pool as league_supports_pool};
pub use pool::{Pool, PoolMeta};
pub use registration::Registration;
pub use season::{Season, SeasonDisplay};
pub use wc_match_result::WcMatchResult;
pub use wc_player_goal_total::WcPlayerGoalTotal;
pub use wc_processed_match::WcProcessedMatch;
pub use wc_tiebreaker_pick::WcTiebreakerPick;

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    migrate::run(conn)
}
