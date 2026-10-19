---
title: NITC export
summary: How your unit's time can flow into Beacon as NITC records, and the unit settings that control it.
order: 75
---

SES Login can copy your members' time into **Beacon** as **NITC** (non-incident)
records, so time spent at training, meetings and other unit activity shows up in
Beacon without anyone entering it twice. It works automatically once it's turned on
for your unit.

## Turning it on for your unit

NITC export is a **unit setting**. Unit settings are managed by the SES Login
maintainers, not from your admin dashboard, so ask them to turn it on or change
it — use **Help/Feedback?** at the bottom of any admin page, or email
[support@seslogin.com](mailto:support@seslogin.com). There are two settings:

| Setting                     | What it does                                                                                                                                                                                      |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **NITC Export From**        | The date your unit starts exporting. Activity that started before it is never exported, so you can switch over cleanly without duplicating NITCs you've already entered by hand. Blank means off. |
| **Mark NITCs as completed** | When ticked, NITCs are marked completed in Beacon as they're exported. Untick it if your unit wants to review and finalise each NITC in Beacon itself.                                            |

Your unit also has to be one whose members come from SES (see
[Members](members.md#members-kept-up-to-date-from-ses)), because that's how SES
Login knows which unit in Beacon to export to.

## What gets exported

An activity entry is exported when **all** of these are true:

- it started on or after your unit's **NITC Export From** date;
- the member has signed out — open entries wait until they're closed;
- its **category** is one that maps to an NITC type (most training, meeting and
  unit-activity categories do — the maintainers set which);
- it lasted less than four days, and its end is after its start.

Guests, and entries with no category, are never exported.

## How entries become NITCs

Entries are grouped into one NITC **per unit, per day, per kind of activity**.
Every member who did that activity that day is a participant in the same NITC, with
their own times. NITCs created by SES Login are named after the category, starting
with "SESLOGIN:", so they're easy to spot in Beacon.

Exports happen within moments of a member signing out, or of an Admin adding or
editing an entry.

## Checking whether an entry has been exported

In the [activity list](activity.md#previous-periods), entries that are eligible
for NITC export have a coloured dot:

- **orange** — not exported yet (usually only for a moment);
- **green** — exported. Hover over it to see which NITC it's in, or click it to
  open that NITC in Beacon.

An entry that stays orange is worth reporting to the maintainers.

## Correcting an exported entry

Fix the entry in SES Login, not in Beacon. [Editing](activity.md#correcting-an-entry)
an exported entry updates its NITC. Changing its category or day moves it to the
right NITC, and deleting it removes the member from the NITC. If that leaves an
NITC with nobody in it, SES Login renames it "SESLOGIN - unused" and marks it
completed, so it doesn't sit in your list of outstanding NITCs.

> [!WARNING]
> Changes made directly to a SES Login NITC in Beacon can be overwritten the next
> time one of its entries changes here.
