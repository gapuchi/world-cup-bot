use crate::{
    db::EplTiebreakerPick,
    types::{Data, Error},
};

pub async fn pick_tiebreaker_player(
    data: &Data,
    guild_id: u64,
    user_id: u64,
    player_query: &str,
) -> Result<String, Error> {
    crate::tiebreaker::pick_tiebreaker_player(data, guild_id, user_id, player_query, |conn,
                                                                                     season_id,
                                                                                     user_id,
                                                                                     selected| {
        EplTiebreakerPick::upsert(
            conn,
            season_id,
            user_id,
            selected.player_id,
            &selected.player_name,
            selected.team_id,
            &selected.team_name,
        )
    })
    .await
}
