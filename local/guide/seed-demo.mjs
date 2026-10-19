/**
 * Fill the local database with invented demo activity for the user-guide screenshots.
 *
 * Adds a dozen made-up members to Test A Unit and about four weeks of history:
 * Tuesday-night training, a Saturday equipment check and one weekend storm job.
 * Every name here is invented. Run against the local stack only:
 *
 *   local/guide/reset.sh (which calls this)
 *
 * Re-running is safe for members (matched by member number), but adds periods
 * again — `make local-clear` first.
 */
const API = process.env.API_URL ?? "http://localhost:8000/";
const TOKEN = "slu_TokenSuperUsr.localdev-super";
const LOCATION = "TestAUnit001";

if (!/localhost|127\.0\.0\.1/.test(API))
  throw new Error(`refusing non-local API ${API}`);

const CAT = {
  training: "wXd1O7sNLChh", // Training - Storm & Water
  meeting: "6uFM5NP8o1x8", // Other - Unit Meeting/Muster
  equipment: "ly9uPGjoundF", // Other - Maintenance - Equipment
  storm: "Xu7e3YQO2L91", // Combat Roles - Storm
  admin: "iP1cxp6Oygoc", // Other - Administration
  community: "Ppuzi8VftNLX", // Community Ed. & Media - SES Role
};

const MEMBERS = [
  ["Priya", "Nair", "10000003"],
  ["Tom", "Kowalski", "10000004"],
  ["Mele", "Fifita", "10000005"],
  ["Jordan", "Reid", "10000006"],
  ["Hannah", "Okafor", "10000007"],
  ["Liam", "Tran", "10000008"],
  ["Grace", "MacLeod", "10000009"],
  ["Sam", "Whitfield", "10000010"],
  ["Ruby", "Castellano", "10000011"],
  ["Dev", "Patel", "10000012"],
  ["Erin", "Doyle", "10000013"],
  ["Marcus", "Webb", "10000014"],
];

async function gql(query, variables = {}) {
  const res = await fetch(API, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${TOKEN}`,
    },
    body: JSON.stringify({ query, variables }),
  });
  const body = await res.json();
  if (body.errors) throw new Error(JSON.stringify(body.errors));
  return body.data;
}

// Deterministic pseudo-random, so every run produces the same history.
let seed = 42;
const rand = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
const pick = (arr, n) => [...arr].sort(() => rand() - 0.5).slice(0, n);

/** Unix seconds for a local Sydney wall-clock time `daysAgo` days back. */
function sydney(daysAgo, hour, minute = 0) {
  const now = new Date();
  const d = new Date(now.getTime() - daysAgo * 86400_000);
  const ymd = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Australia/Sydney",
  }).format(d);
  // Find the UTC offset Sydney had on that date (AEST/AEDT).
  const probe = new Date(`${ymd}T12:00:00Z`);
  const tzHour = Number(
    new Intl.DateTimeFormat("en-GB", {
      timeZone: "Australia/Sydney",
      hour: "2-digit",
      hourCycle: "h23",
    }).format(probe),
  );
  const offset = tzHour - 12;
  const utc =
    Date.parse(`${ymd}T00:00:00Z`) + ((hour - offset) * 60 + minute) * 60_000;
  return Math.floor(utc / 1000);
}
const dow = (daysAgo) =>
  new Intl.DateTimeFormat("en-AU", {
    timeZone: "Australia/Sydney",
    weekday: "short",
  }).format(new Date(Date.now() - daysAgo * 86400_000));

const { location } = await gql(
  `query($id: ID!) { location(id: $id) { people { id memberNumber firstName } } }`,
  { id: LOCATION },
);
const byNumber = new Map(location.people.map((p) => [p.memberNumber, p.id]));
for (const [firstName, lastName, memberNumber] of MEMBERS) {
  if (byNumber.has(memberNumber)) continue;
  const { createPerson } = await gql(
    `mutation($l: ID!, $f: String!, $s: String!, $n: String!) {
       createPerson(locationId: $l, firstName: $f, lastName: $s, memberNumber: $n) { id } }`,
    { l: LOCATION, f: firstName, s: lastName, n: memberNumber },
  );
  byNumber.set(memberNumber, createPerson.id);
  console.log(`member ${firstName} ${lastName}`);
}
const people = [...byNumber.values()];

const periods = [];
const add = (person, cat, start, end, comment) =>
  periods.push({ person, cat, start, end, comment });

for (let ago = 28; ago >= 1; ago--) {
  const day = dow(ago);
  if (day === "Tue") {
    for (const p of pick(people, 8 + Math.floor(rand() * 5))) {
      const cat = ago % 14 < 7 ? CAT.training : CAT.meeting;
      add(
        p,
        cat,
        sydney(ago, 19, Math.floor(rand() * 10)),
        sydney(ago, 21, 20 + Math.floor(rand() * 25)),
      );
    }
  } else if (day === "Sat" && ago > 7) {
    for (const p of pick(people, 4 + Math.floor(rand() * 3))) {
      add(
        p,
        CAT.equipment,
        sydney(ago, 9, Math.floor(rand() * 20)),
        sydney(ago, 12, 15 + Math.floor(rand() * 40)),
      );
    }
  } else if (day === "Sun" && ago <= 7) {
    for (const p of pick(people, 9)) {
      add(
        p,
        CAT.storm,
        sydney(ago, 6, Math.floor(rand() * 30)),
        sydney(ago, 15, Math.floor(rand() * 50)),
        "Storm job — trees down",
      );
    }
  } else if (day === "Thu" && rand() < 0.6) {
    for (const p of pick(people, 2)) {
      add(
        p,
        ago % 2 ? CAT.admin : CAT.community,
        sydney(ago, 18, 30),
        sydney(ago, 20, 45),
      );
    }
  }
}

for (const p of periods) {
  await gql(
    `mutation($p: ID!, $l: ID!, $c: ID!, $s: Int!, $e: Int!, $m: String) {
       createPeriod(personId: $p, locationId: $l, categoryId: $c, startTime: $s, endTime: $e, comment: $m) { id } }`,
    {
      p: p.person,
      l: LOCATION,
      c: p.cat,
      s: p.start,
      e: p.end,
      m: p.comment ?? null,
    },
  );
}
console.log(`${periods.length} periods written`);
console.log(`people: ${JSON.stringify(Object.fromEntries(byNumber))}`);
