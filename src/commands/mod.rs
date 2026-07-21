mod config;
mod draft;
mod helpers;
mod meta;
mod nfl;
mod registration;
mod wc;

pub use config::{config, config_channel, config_league, config_leagues};
pub use draft::draft;
pub use meta::{help, ping, register, version};
pub use registration::{assign, claim, my_team, teams, unclaim, unclaimed};
pub use wc::{pick_player, remaining, season, standings};

use crate::league::League;

pub fn all() -> Vec<poise::Command<crate::types::Data, crate::types::Error>> {
    let mut commands = vec![
        ping(),
        version(),
        help(),
        register(),
        config(),
        draft(),
        claim(),
        assign(),
        unclaim(),
        my_team(),
        teams(),
        unclaimed(),
        standings(),
        season(),
    ];
    for league in League::ALL {
        commands.extend(commands_for(*league));
    }
    commands
}

/// League-specific slash commands. Exhaustive over `League` so new variants must register here.
fn commands_for(
    league: League,
) -> Vec<poise::Command<crate::types::Data, crate::types::Error>> {
    match league {
        League::Wc => vec![remaining(), pick_player()],
    }
}
