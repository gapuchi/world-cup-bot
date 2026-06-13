use std::collections::HashSet;

use rand::seq::SliceRandom;

use crate::{
    db::{Draft, DraftParticipant, DraftStatus, Registration, Season, WcTiebreakerPick},
    types::{Data, Error},
};

const MIN_PARTICIPANTS: usize = 2;

pub struct DraftView {
    pub status: DraftStatus,
    pub rounds: i64,
    pub current_pick: i64,
    pub total_picks: i64,
    pub participants: Vec<DraftParticipant>,
    pub current_picker: Option<u64>,
}

pub struct TurnChange {
    pub completed: bool,
    pub next_picker: Option<u64>,
    pub pick_number: i64,
    pub total_picks: i64,
}

pub fn get_view(conn: &rusqlite::Connection, season_id: i64) -> rusqlite::Result<Option<DraftView>> {
    let Some(draft) = Draft::get(conn, season_id)? else {
        return Ok(None);
    };
    let participants = Draft::list_participants(conn, season_id)?;
    let current_picker = if draft.status == DraftStatus::Active {
        Draft::current_picker(conn, season_id)?
    } else {
        None
    };
    Ok(Some(DraftView {
        status: draft.status,
        rounds: draft.rounds,
        current_pick: draft.current_pick,
        total_picks: draft.total_picks,
        participants,
        current_picker,
    }))
}

pub fn is_active(conn: &rusqlite::Connection, season_id: i64) -> rusqlite::Result<bool> {
    Ok(matches!(
        Draft::get(conn, season_id)?,
        Some(draft) if draft.status == DraftStatus::Active
    ))
}

pub async fn start(
    data: &Data,
    guild_id: u64,
    member_ids: Vec<u64>,
    rounds: i64,
) -> Result<(DraftView, TurnChange), Error> {
    validate_start(&member_ids, rounds)?;

    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    let league_slug = Season::league_slug_for(&conn, season.id)?;
    if league_slug != "wc" {
        return Err("Drafts are only supported for World Cup seasons.".into());
    }
    if Draft::get(&conn, season.id)?.is_some() {
        return Err("A draft already exists for this season. Cancel it first with `/draft cancel`.".into());
    }

    let mut shuffled = member_ids;
    shuffled.shuffle(&mut rand::thread_rng());
    let participants: Vec<(u64, i64)> = shuffled
        .into_iter()
        .enumerate()
        .map(|(index, user_id)| (user_id, index as i64))
        .collect();
    let total_picks = participants.len() as i64 * rounds;
    let draft = Draft::create_active(&conn, season.id, rounds, total_picks, &participants)?;
    let current_picker = Draft::current_picker(&conn, season.id)?;

    let view = get_view(&conn, season.id)?.expect("draft just created");
    Ok((
        view,
        TurnChange {
            completed: false,
            next_picker: current_picker,
            pick_number: draft.current_pick + 1,
            total_picks: draft.total_picks,
        },
    ))
}

fn validate_start(member_ids: &[u64], rounds: i64) -> Result<(), Error> {
    if rounds < 1 {
        return Err("Draft needs at least one round.".into());
    }
    if member_ids.len() < MIN_PARTICIPANTS {
        return Err(format!(
            "Draft needs at least {MIN_PARTICIPANTS} participants."
        )
        .into());
    }
    if member_ids.len() != member_ids.iter().collect::<HashSet<_>>().len() {
        return Err("Each participant can only appear once.".into());
    }
    Ok(())
}

pub async fn skip(data: &Data, guild_id: u64) -> Result<TurnChange, Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    let draft = active_draft(&conn, season.id)?;
    advance_pick(&conn, season.id, &draft)
}

pub async fn cancel(data: &Data, guild_id: u64) -> Result<(), Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    let Some(draft) = Draft::get(&conn, season.id)? else {
        return Err("No draft is configured for this season.".into());
    };
    if draft.status == DraftStatus::Active || draft.status == DraftStatus::Complete {
        WcTiebreakerPick::delete_all_for_season(&conn, season.id)?;
        Registration::delete_all_for_season(&conn, season.id)?;
        Draft::delete(&conn, season.id)?;
    }
    Ok(())
}

pub async fn advance_after_pick(data: &Data, guild_id: u64) -> Result<TurnChange, Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    let draft = active_draft(&conn, season.id)?;
    advance_pick(&conn, season.id, &draft)
}

pub async fn status(data: &Data, guild_id: u64) -> Result<Option<DraftView>, Error> {
    let conn = data.db.lock().await;
    let season = Season::default_for_guild(&conn, guild_id)?;
    Ok(get_view(&conn, season.id)?)
}

pub fn format_order(participants: &[DraftParticipant]) -> String {
    participants
        .iter()
        .map(|participant| format!("<@{}>", participant.user_id))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_turn_message(change: &TurnChange) -> String {
    if change.completed {
        return "Draft complete — all picks are in. Rosters are locked.".into();
    }
    let picker = change
        .next_picker
        .map(|user_id| format!("<@{user_id}>"))
        .unwrap_or_else(|| "Unknown".into());
    format!(
        "Pick **{}** of **{}** — {} is on the clock. Use `/draft pick`.",
        change.pick_number, change.total_picks, picker
    )
}

fn active_draft(conn: &rusqlite::Connection, season_id: i64) -> Result<crate::db::Draft, Error> {
    let Some(draft) = Draft::get(conn, season_id)? else {
        return Err("No draft is configured for this season.".into());
    };
    if draft.status != DraftStatus::Active {
        return Err("The draft is not active.".into());
    }
    Ok(draft)
}

fn advance_pick(
    conn: &rusqlite::Connection,
    season_id: i64,
    draft: &crate::db::Draft,
) -> Result<TurnChange, Error> {
    if draft.current_pick >= draft.total_picks {
        return Err("The draft is already finished.".into());
    }

    Draft::advance_pick(conn, season_id)?;
    let updated = Draft::get(conn, season_id)?.expect("draft exists");

    if updated.current_pick >= updated.total_picks {
        Draft::mark_complete(conn, season_id)?;
        return Ok(TurnChange {
            completed: true,
            next_picker: None,
            pick_number: updated.total_picks,
            total_picks: updated.total_picks,
        });
    }

    Ok(TurnChange {
        completed: false,
        next_picker: Draft::current_picker(conn, season_id)?,
        pick_number: updated.current_pick + 1,
        total_picks: updated.total_picks,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::db::{GuildConfig, self};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let season = Season::get_or_create(&conn, 1, "wc", "wc-2026", "World Cup 2026").unwrap();
        GuildConfig::set_default_season_id(&conn, 1, season.id).unwrap();
        conn
    }

    #[test]
    fn start_rejects_single_participant() {
        assert!(validate_start(&[1], 1).is_err());
    }

    #[test]
    fn advance_completes_after_total_picks() {
        let conn = test_conn();
        let season = Season::default_for_guild(&conn, 1).unwrap();
        Draft::create_active(&conn, season.id, 1, 2, &[(10, 0), (20, 1)]).unwrap();
        let draft = Draft::get(&conn, season.id).unwrap().unwrap();

        let first = advance_pick(&conn, season.id, &draft).unwrap();
        assert!(!first.completed);
        assert_eq!(first.next_picker, Some(20));

        let draft = Draft::get(&conn, season.id).unwrap().unwrap();
        let second = advance_pick(&conn, season.id, &draft).unwrap();
        assert!(second.completed);

        let finished = Draft::get(&conn, season.id).unwrap().unwrap();
        assert_eq!(finished.status, DraftStatus::Complete);
    }
}
