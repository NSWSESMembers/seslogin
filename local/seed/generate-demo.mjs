/**
 * Regenerate local/seed/demo.json: Test C Unit with months of realistic activity, all invented.
 *
 *   node local/seed/generate-demo.mjs
 *
 * Deterministic (fixed seed), so a re-run with no edits writes the same file. Times are written as
 * expressions relative to when seeding runs (`now-2h`, `monday-3w+1d+19h`), which `local-seed demo`
 * resolves; see DEVELOPMENT.md, "Seed data". Category ids come from from-prod.json, by name.
 * Review the diff, then `make local-demo` and `cargo test --test seed_fixtures`.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

/** mulberry32: a small seeded PRNG, so the output is reproducible. */
function prng(seed) {
  let a = seed;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const random = prng(20261025);
const randint = (lo, hi) => lo + Math.floor(random() * (hi - lo + 1));
const choice = (xs) => xs[Math.floor(random() * xs.length)];
/** `k` distinct items, in random order. */
const sample = (xs, k) => {
  const pool = [...xs];
  for (let i = 0; i < k; i++) {
    const j = i + Math.floor(random() * (pool.length - i));
    [pool[i], pool[j]] = [pool[j], pool[i]];
  }
  return pool.slice(0, k);
};

const prod = JSON.parse(readFileSync(join(HERE, "from-prod.json"), "utf8"));
const categories = new Map(prod.tables.category.map((c) => [c.name.S, c.id.S]));
const category = (name) => {
  const id = categories.get(name);
  if (!id) throw new Error(`no category named ${name} in from-prod.json`);
  return id;
};

const LOC = "TestCUnit001";
const [K1, K2, K3] = ["TestCKiosk01", "TestCKiosk02", "TestCKiosk03"];
const EPOCH = { N: "1767225600" };
const S = (v) => ({ S: v });
const N = (v) => ({ N: String(v) });

// Invented. First and last names share an initial, a tell that no real member is meant.
const NAMES = [
  ["Morgan", "Marsh"],
  ["Lee", "Lawson"],
  ["Kit", "Kendall"],
  ["Parker", "Pryce"],
  ["Riley", "Rowe"],
  ["Dana", "Dalton"],
  ["Harper", "Hale"],
  ["Quinn", "Quigley"],
  ["Sam", "Sutton"],
  ["Tay", "Truman"],
  ["Uma", "Underwood"],
  ["Val", "Vance"],
  ["Wren", "Whitlock"],
  ["Yael", "York"],
  ["Ash", "Archer"],
  ["Bay", "Brennan"],
  ["Cody", "Calloway"],
  ["Drew", "Dempsey"],
  ["Finn", "Fletcher"],
  ["Gale", "Garrison"],
  ["Indy", "Ingram"],
  ["Jordan", "Jett"],
  ["Oakley", "Orr"],
  ["Charlie", "Crane"],
  ["Ellis", "Easton"],
  ["Frankie", "Fox"],
  ["Noel", "Nash"],
  ["Zed", "Zimmer"],
  ["Eden", "Ellery"],
  ["Xan", "Xavier"],
];
const member = {};
const people = NAMES.map(([first, last], i) => {
  const n = String(i + 1).padStart(2, "0");
  const id = `TestCMemb0${n}`;
  member[last] = id;
  const row = {
    id: S(id),
    location_id: S(LOC),
    first_name: S(first),
    last_name: S(last),
    registration_number: S(`300000${n}`),
    deleted: N(0),
    created_at: EPOCH,
    updated_at: EPOCH,
  };
  // Most are synced from SES; the last few were added by hand, so admins can edit them.
  if (i < 24) row.ses_api_person_id = S(`300000${n}`);
  return row;
});
const person = (last) => people.find((p) => p.last_name.S === last);
Object.assign(person("Nash"), {
  _comment:
    "Absent from SES since 4 days ago: pending deletion by member sync's absence pass.",
  missing_since: N("now-4d"),
});
Object.assign(person("Zimmer"), {
  _comment: "Soft-deleted a month ago; their older periods remain.",
  deleted: N("now-30d"),
});
Object.assign(person("Ellery"), {
  _comment: "A recruit added this week with no activity yet.",
  created_at: N("now-6d"),
  updated_at: N("now-6d"),
});
person("Xavier")._comment = "Never signed in: shows as never seen.";

const ACTIVE = people
  .map((p) => p.last_name.S)
  .filter((l) => !["Nash", "Zimmer", "Ellery", "Xavier"].includes(l));
const LEADERS = ["Marsh", "Lawson", "Kendall", "Pryce"];
const BOAT = ["Kendall", "Rowe", "Hale"];
const CASUAL = ["Garrison", "Ingram", "Jett", "Orr", "Crane", "Easton", "Fox"];
// Signed in right now: kept out of anything else today.
const OPEN_NOW = ["Rowe", "Marsh", "Dalton", "Quigley", "Underwood"];
const VISITOR = "TestBMember1";
const COMMENTS = [
  "Brought the trailer back",
  "Swapped shifts with a teammate",
  "Left early for work",
  "Signed in late, was at the station first",
  "Covered radio for the team",
];
const TRAINING = [
  "Training - Storm & Water",
  "Training - Flood Operator L1",
  "Training - Land Search",
  "Training - Fundamentals",
  "Training - Drive Operational Vehicles",
  "Training - Flood Operator L2 (Boat)",
];

const periods = [];
const nextId = () => `TestCPer${String(periods.length + 1).padStart(4, "0")}`;

/** Sydney local time `week` weeks before this one: `monday-<w>w+<d>d+<h>h+<m>m`. */
function at(week, day, hour, minute = 0) {
  let expr = "monday" + (week ? `-${week}w` : "");
  for (const [n, unit] of [
    [day, "d"],
    [hour, "h"],
    [minute, "m"],
  ]) {
    if (n) expr += `+${n}${unit}`;
  }
  return expr;
}

/** A closed period, signed in and out at `kiosk` (none for admin-entered ones). */
function add(
  who,
  categoryName,
  start,
  minutes,
  { kiosk = K1, comment, guest } = {},
) {
  const end = `${start}+${minutes}m`;
  const row = {
    id: S(nextId()),
    location_id: S(LOC),
    category_id: S(category(categoryName)),
    start_time: N(start),
    end_time: N(end),
    location_live: S(LOC),
    v: N(1),
    created_at: N(start),
    updated_at: N(end),
  };
  if (guest) row.guest_name = S(guest);
  else row.person_id = S(who);
  if (kiosk) {
    row.signed_in_session_id = S(kiosk);
    row.signed_out_session_id = S(kiosk);
  }
  if (comment || (!guest && random() < 0.06))
    row.comment = S(comment ?? choice(COMMENTS));
  periods.push(row);
  return row;
}

/** A period still open now: in `location_open`, no category until sign-out. */
function open(who, start, { guest, comment, note } = {}) {
  const row = {
    id: S(nextId()),
    location_id: S(LOC),
    start_time: N(start),
    location_open: S(LOC),
    location_live: S(LOC),
    v: N(1),
    signed_in_session_id: S(K1),
    created_at: N(start),
    updated_at: N(start),
  };
  if (guest) row.guest_name = S(guest);
  else row.person_id = S(who);
  if (comment) row.comment = S(comment);
  if (note) row._comment = note;
  periods.push(row);
}

for (let week = 12; week >= 0; week--) {
  // Tuesday night: training, or the monthly muster every fourth week.
  const muster = week % 4 === 0;
  const topic = muster
    ? "Other - Unit Meeting/Muster"
    : TRAINING[week % TRAINING.length];
  for (const last of ACTIVE) {
    let chance = LEADERS.includes(last)
      ? 0.9
      : CASUAL.includes(last)
        ? 0.35
        : 0.65;
    if (muster) chance = Math.min(1, chance + 0.15);
    if (random() < chance) {
      const jitter = randint(-10, 20);
      const start =
        at(week, 1, 19) +
        (jitter > 0 ? `+${jitter}m` : jitter < 0 ? `-${-jitter}m` : "");
      add(member[last], topic, start, 150 + randint(-20, 30), {
        kiosk: random() < 0.3 ? K2 : K1,
      });
    }
  }
  if (!muster) {
    const trainer =
      week % 2 ? "Trainer - Storm & Water" : "Trainer - Flood Operator L1";
    add(member.Kendall, trainer, at(week, 1, 18, 30), 210);
  }
  // Thursday: unit admin by the leaders.
  for (const last of ["Marsh", "Lawson"]) {
    if (random() < 0.8)
      add(
        member[last],
        "Other - Administration",
        at(week, 3, 18),
        choice([90, 120, 150]),
      );
  }
  // Alternate Sundays: equipment and building maintenance.
  if (week % 2 === 0) {
    const pool = ACTIVE.filter((a) => week || !OPEN_NOW.includes(a));
    for (const last of sample(pool, 4)) {
      const kind = choice([
        "Other - Maintenance - Equipment",
        "Other - Maintenance - Building/Land",
      ]);
      add(member[last], kind, at(week, 6, 9), choice([150, 180, 210]));
    }
  }
  // Alternate weekends: a 24-hour duty officer roster.
  if (week % 2 === 1) {
    add(member.Lawson, "Other - Duty Officer", at(week, 5, 8), 24 * 60, {
      kiosk: null,
      comment: "Weekend duty officer",
    });
  }
}

// Storm callouts: Saturday afternoons, and one late-night job.
for (const [week, size] of [
  [2, 9],
  [5, 7],
  [10, 6],
]) {
  for (const last of sample(ACTIVE, size)) {
    add(
      member[last],
      "Combat Roles - Storm",
      at(week, 5, 14, choice([0, 15, 30])),
      randint(240, 420),
    );
  }
}
for (const last of sample(ACTIVE, 5)) {
  add(member[last], "Combat Roles - Storm", at(4, 3, 22), randint(180, 260), {
    comment: "Tree down across the highway",
  });
}

// A three-day flood operation: two shifts a day, boat crew on rescue.
for (const day of [4, 5, 6]) {
  for (const hour of [6, 14]) {
    for (const last of sample(
      ACTIVE.filter((a) => !BOAT.includes(a)),
      6,
    )) {
      add(
        member[last],
        "Combat Roles - Flood Ops. (Non Rescue)",
        at(7, day, hour),
        8 * 60,
      );
    }
  }
  for (const last of BOAT)
    add(
      member[last],
      "Combat Roles - Flood Boat Rescue",
      at(7, day, 7),
      10 * 60,
    );
}

// A land search, a community stall, a school talk.
for (const last of sample(ACTIVE, 5))
  add(member[last], "Support Roles - Land Search", at(4, 2, 8), 9 * 60);
for (const last of sample(ACTIVE, 4)) {
  add(
    member[last],
    "Community Ed. & Media - Public Relations",
    at(3, 5, 9),
    240,
    {
      comment: "Shopping centre stall",
    },
  );
}
for (const last of sample(ACTIVE, 2)) {
  add(member[last], "Community Ed. & Media - SES Role", at(8, 2, 10), 120, {
    comment: "Primary school visit",
  });
}

// Older periods for the soft-deleted member.
for (const week of [11, 10, 9, 8, 6])
  add(member.Zimmer, TRAINING[week % TRAINING.length], at(week, 1, 19), 150);

// Test B Unit's member visiting for training.
for (const week of [6, 3, 1])
  add(VISITOR, "Other - Attend Other Unit (not OOAA)", at(week, 1, 19), 150);

// Guests who signed in and out at the kiosk.
for (const [week, name, reason] of [
  [9, "Visiting Volunteer", "Observing training"],
  [5, "Council Officer", "Flood briefing"],
  [2, "Prospective Member", "Information night"],
]) {
  add(null, "Other - Attend Other", at(week, 1, 19, 15), 90, {
    guest: name,
    comment: reason,
  });
}

// Yesterday, whatever day seeding runs, so the daily summary always has something to say.
for (const [last, name, hour, minutes] of [
  ["Marsh", "Other - Administration", 17, 120],
  ["Truman", "Other - Maintenance - Equipment", 16, 150],
  ["Whitlock", "Other - Maintenance - Equipment", 16, 150],
]) {
  add(member[last], name, `today-1d+${hour}h`, minutes);
}

// An admin corrected one period's times after the fact.
Object.assign(
  add(member.Rowe, "Training - Storm & Water", at(3, 1, 19), 150, {
    comment: "Times corrected by admin",
  }),
  {
    _comment: "Edited after sign-out: version 2, updated after it ended.",
    v: N(2),
    updated_at: N("now-2w"),
  },
);
// A period an admin deleted: kept, but out of the location index.
const deleted = add(
  member.Sutton,
  "Training - Fundamentals",
  at(6, 1, 19),
  150,
);
deleted._comment =
  "Deleted by an admin: has `deleted` and no `location_live`, like a real delete.";
delete deleted.location_live;
deleted.deleted = N("now-5w");

// Right now: people signed in, someone who forgot to sign out, and a guest.
for (const [last, ago] of [
  ["Marsh", "now-3h"],
  ["Dalton", "now-2h"],
  ["Quigley", "now-95m"],
  ["Underwood", "now-40m"],
]) {
  open(member[last], ago);
}
open(member.Rowe, "now-15h", {
  note: "Signed in 15 hours ago: their next scan hits the forgot-to-sign-out screen.",
});
open(null, "now-30m", {
  guest: "Visiting Volunteer",
  comment: "Delivering sandbags",
});

const users = [
  {
    _comment: "Admin at Test C Unit only, subscribed to its daily summary.",
    id: S("TestCUnitAdm"),
    email: S("cunit@seslogin.test"),
    super: { BOOL: false },
    enabled: N(1),
    location_grants: { SS: [LOC] },
    email_config: S(JSON.stringify({ [LOC]: { daily: "1" } })),
    created_at: EPOCH,
    updated_at: EPOCH,
  },
  {
    _comment: "Read only at Test C Unit.",
    id: S("TestCViewer1"),
    email: S("cview@seslogin.test"),
    super: { BOOL: false },
    enabled: N(1),
    location_read_only_grants: { SS: [LOC] },
    created_at: EPOCH,
    updated_at: EPOCH,
  },
  {
    _comment: "A disabled former admin: no `enabled`, as the app writes it.",
    id: S("TestCFormer1"),
    email: S("former@seslogin.test"),
    super: { BOOL: false },
    location_grants: { SS: [LOC] },
    created_at: EPOCH,
    updated_at: EPOCH,
  },
];
// Public halves of throwaway P-256 keys: the kiosks look key-enrolled, as every live kiosk now is.
// Nothing signs as them, so no private half is kept. The key window renews on contact, 14 days.
const enrolled = (spki, fingerprint) => ({
  public_key: S(spki),
  key_fingerprint: S(fingerprint),
  key_expires_at: N("now+13d"),
});
const sessions = [
  {
    _comment:
      "Front door kiosk: key-enrolled, last seen a minute ago, Who's here always on.",
    id: S(K1),
    name: S("Test C Unit front door"),
    location_id: S(LOC),
    config: S(JSON.stringify({ signedInStatusInline: true })),
    ...enrolled(
      "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEBFAoBzsGAxXhhfOgtLaRCbRzo/qvpR1p6SGDqWIKZ9aDOrLzOkLzfANQ09zDUtgtxT2kbByiuNhtnASiivYMgQ==",
      "916a7adfae8da395f86602997c902a6e6626656f6ecf19571ca25dd97fd7acce",
    ),
    active: N(1),
    last_contact: N("now-1m"),
    created_at: EPOCH,
    updated_at: EPOCH,
  },
  {
    _comment:
      "Training room kiosk: key-enrolled, pinned light, last seen two days ago.",
    id: S(K2),
    name: S("Test C Unit training room"),
    location_id: S(LOC),
    config: S(JSON.stringify({ theme: "light" })),
    ...enrolled(
      "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3Xvpirr9pC7xUT5RlEFtTiuUeheMRDkzRw/zPG2F879HrER/jqJa5IPjBioHyKqsvguQDs8P6al4jYYcHe0/0Q==",
      "3f58c617148987b6cc1ae6c9ed9d672183e31ba10a9e88f83c25789dcbc67830",
    ),
    active: N(1),
    last_contact: N("now-2d"),
    created_at: EPOCH,
    updated_at: EPOCH,
  },
  {
    _comment: "Retired after weeks offline: no `active`, stamped `retired_at`.",
    id: S(K3),
    name: S("Old shed tablet"),
    location_id: S(LOC),
    config: S("{}"),
    last_contact: N("now-40d"),
    retired_at: N("now-20d"),
    created_at: EPOCH,
    updated_at: EPOCH,
  },
];

const doc = {
  _comment:
    "Demo data for a lived-in dashboard, written by `make local-demo` / `local-seed demo` on top of synthetic.json. " +
    "Generated by generate-demo.mjs: edit that, not this. Test C Unit: 30 invented members, 3 users, 3 kiosks and " +
    "about 12 weeks of activity covering training nights, musters, storm callouts, a flood operation, searches, " +
    "community events, maintenance, duty officers, guests, a visiting member, members signed in now, a forgotten " +
    "sign-out, an edited and a deleted period, a departing, a deleted, a new and a never-seen member. Times are " +
    "relative to when seeding runs (`now-2h`, `monday-3w+1d+19h`: Sydney local time), so re-running it brings the " +
    "data up to date; periods that would land in the future are skipped.",
  tables: {
    location: [
      {
        _comment:
          "Member sync last succeeded 5 hours ago, as the hourly dispatcher would leave it.",
        id: S(LOC),
        name: S("Test C Unit"),
        enabled: { BOOL: true },
        last_successful_member_sync: N("now-5h"),
        created_at: EPOCH,
        updated_at: EPOCH,
      },
    ],
    person: people,
    user: users,
    session: sessions,
    period: periods,
  },
};

writeFileSync(join(HERE, "demo.json"), JSON.stringify(doc, null, 2) + "\n");
console.log(`demo.json: ${people.length} people, ${periods.length} periods`);
