mod config;
mod helpers;
mod meta;
mod nfl;
mod registration;
mod wc;

pub use config::{config, config_channel, config_league, config_leagues};
pub use meta::{help, ping, register, version};
pub use registration::{assign, claim, my_team, teams, unclaim, unclaimed};
pub use wc::{pick_player, season, standings};

pub fn all() -> Vec<poise::Command<crate::types::Data, crate::types::Error>> {
    vec![
        ping(),
        version(),
        help(),
        register(),
        config(),
        claim(),
        assign(),
        unclaim(),
        my_team(),
        teams(),
        unclaimed(),
        standings(),
        pick_player(),
        season(),
    ]
}
