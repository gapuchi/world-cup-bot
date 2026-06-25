# Scratch: wc-remaining

Mutable progress notes—update during implementation. Plan lives in `plan.md`.

## Current

- **PR:** 3 of 3 — complete
- **Branch:** wc-remaining
- **Last completed:** PR 3 elimination announcements

## Stack

```
wc-remaining (phase 1 + phase 2, uncommitted)
```

## Speedbumps

- TBD knockout fixtures needed MatchTeam with optional id/name (phase 1)
- `group_standings` needed split borrows for home/away row updates

## Learnings

- Poller now uses fetch_competition_matches (same call count as old finished-only fetch)
- fetch_teams added once per WC poll for classify_teams parity with /remaining
- Eliminations only marked in DB after successful Discord post
