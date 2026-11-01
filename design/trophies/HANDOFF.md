# Perpetual trophies: design handoff

Status: **design only**. No code has been written. This note lets a new Claude
session (or a person) pick the work up where the first session left it.

- Published design page: https://claude.ai/artifact/AjjRQQFXW9DfWpYRtSruUX
  (version 8). A local copy is beside this note: `perpetual-trophies.html`.
  To update the published page, read it with the Artifact tool's `read` action
  and publish to that URL. Don't create a new one.
- All names and numbers in the design are invented: the local seed fixtures
  (Alice Anderson, Bob Brown, Crossunit Tester, Test A/B Unit) plus "Priya Nair".

## The idea

Badges that belong to a **unit**, not a member. Each one has exactly **one
holder** at a time. Whoever wins it next takes it from the current holder. Every
change of hands is kept as a "reign", and the admin report shows them as an
engraved plate.

## Trophies (11)

Two kinds:

- **Standing record**: the holder's claim is a number. Beat it and the trophy
  moves at once, on the kiosk scan. Records sit in a 365-day window. When the
  holder's claim ages out, the best claim still inside the window takes over;
  if there is none, the holder keeps it.
- **Rolling lead**: scored nightly over a trailing window. A challenger must
  **strictly beat the holder's current score** and meet the minimum.

| Trophy | Kind | Rule | Min | Window |
|---|---|---|---|---|
| The Kettle | rolling | Most busy days first to sign in | 3 days | 30 d |
| The Padlock | rolling | Most busy days last to sign out | 3 days | 30 d |
| The Boomerang | record | Longest gap since previous sign-in at the unit | 90 days | 365 d |
| The Welcome Mat | record | Furthest-travelled **visitor**, by home unit distance (`away_distance_km`) | 50 km | 365 d |
| Iron Roster | rolling | Most hours on site, each sign-in capped at 12 h | 10 h | 28 d |
| The Owl | rolling | Most nights with a sign-in started 22:00–05:00 Sydney time, lasting 30 min or more, one per night | 2 nights | 60 d |
| The Chalk | rolling | Most trainer/assessor sign-outs (`TEACHING_DISCIPLINES`) | 2 | 90 d |
| The Tarp | rolling | Most Storm sign-outs | 2 | 30 d |
| The Spanner | rolling | Most hours in Maintenance (Building/Land `MBaJJzYArrxi`, Equipment `ly9uPGjoundF`), 12 h cap | 4 h | 60 d |
| The Clipboard | rolling | Most hours in Administration (`iP1cxp6Oygoc`, `ouXC4etC8lxy`, `u25bkFaIRr6I`), 12 h cap | 4 h | 60 d |
| Unbroken Chain | rolling | Longest *current* weekly sign-in streak | 4 weeks | current |

A "busy day" has 4 or more members on site, the same threshold `badge_nightly`'s
`BUSY_DAY_MEMBERS` uses for Lights On and Lights Off. Category IDs come from
`api/src/badges.rs`. Virtual categories never count.

## House rules

- One holder per unit and trophy.
- Ties go to the holder. Between challengers on the same score, whoever reached
  it first wins.
- A trophy only ever changes hands. It goes vacant only when the holder leaves the unit.
- Same gamification switches as personal badges (location, kiosk and the member's home unit).
- Home members only, except The Welcome Mat, which is visitors only. Guest
  sign-ins never count.
- **Nothing rewards a long single shift**, because that would cut against
  fatigue management.
- An edited or deleted period that a claim rested on reverts the trophy to the
  previous valid holder. The nightly run checks this.

## Decisions already made by the user (don't revisit)

- **No Tally changes whatsoever.** No Tally notifications. The member who lost a
  trophy is told on their **next kiosk scan** ("while you were away"), once per loss.
- Rejected: a "longest shift" trophy (fatigue), and Muster Bell in both of its
  forms. Most members muster and then sign out to their next activity, so the
  Muster category undercounts; the "busy days attended" version was rejected too.
- Rejected: The Tether (time on site alongside new members).
- The Kettle and The Padlock were daily claims and would change hands almost
  every day. They're now 30-day rolling counts.
- The boomerang icon was redrawn from Game-icons' boomerang proportions
  (CC BY 3.0; the shape was used as a reference, not copied). The Spanner and
  The Clipboard icons are Lucide (ISC).

## Open decisions

1. Can visitors take home trophies? Proposed: no.
2. Does holding a trophy leave a permanent personal badge ("Once held …")?
   Proposed: yes.
3. Show only the holder's score, not a leaderboard? Proposed: yes.
4. Is 30 days enough busy days for The Kettle and The Padlock? Check against prod.
5. Should The Owl count any category, or only call-outs? Proposed: any category.

## Next step: look at prod for more trophies

The cloud session had no AWS credentials, so this hasn't been done. Run it
locally, **read-only**, against `seslogin_prod` (see CLAUDE.md's database
warning). Measure:

- how often each proposed trophy would change hands at real units (churn)
- the categories units actually use, and hours per category
- busy days per unit per 30 days
- patterns that might make new trophies: weekdays, call-out times, visitors

## Build plan (one commit per PR, each deployable)

1. Infra: a `{prefix}_trophy` table (pk `location_id`; sk `holder#<id>` /
   `reign#<id>#<claimed_at>`) in Terraform for test and prod,
   `local-tables.rs`, SCHEMA.md and IAM.
2. `db::Handler` methods for reads and version-conditional writes. Not audited
   (derived from periods, like `BadgeState`); `AuditingHandler` passes them through.
3. `trophies.rs`: the definitions and the steal, tie and revert rules, with unit tests.
4. Nightly evaluation in `badge_nightly` (the rolling leads, record age-outs and
   reconciliation).
5. `Location.trophies` in GraphQL, plus the admin Reports › Badges cabinet and
   the admin guide.
6. On-scan claims (The Boomerang, The Welcome Mat), `trophyChanges` on the
   sign-in/out payloads, the kiosk card, the Who's here mark and the kiosk guide.
7. The kiosk "while you were away" notice (`seen_at` on the reign row), plus a
   "Changed hands this week" section in the weekly digest.
