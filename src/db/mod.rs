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

pub use pool::Pool;
pub use registration::Registration;
pub use season::SeasonDisplay;
pub use wc_match_result::WcMatchResult;
pub use wc_player_goal_total::WcPlayerGoalTotal;
pub use wc_processed_match::WcProcessedMatch;
pub use wc_tiebreaker_pick::WcTiebreakerPick;

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    migrate::run(conn)
}
