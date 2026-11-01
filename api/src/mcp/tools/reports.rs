//! Report tools: `get_dashboard`, `summarize_activity`, `members_last_seen`.
//!
//! These mirror the admin site's reporting pages: *Home* is `get_dashboard`; the
//! *Totals*, *Breakdown*, *Daily breakdown* and *Heatmap* pages are the six
//! `summarize_activity` groupings; *Last seen* is `members_last_seen`. The *Reports*
//! page is only a period export, which `list_periods` over a date range already
//! covers, so it has no tool of its own.
//!
//! **Grants.** Only `Location.dashboardSummary` and `Person.lastPeriod` check the
//! caller's grant in GraphQL. `Query.location`, `Location.people` and all six
//! `Location.periodSummaryBy*` resolvers only require a signed-in caller, so every tool
//! here calls [`ToolCtx::require_location_read`] before running its document; it only
//! ever narrows access.
//!
//! **Units.** Every duration (`totalTime`, `avgCompletedDuration7D`, …) is in
//! seconds. Timestamps are Unix seconds and get a `...Sydney` sibling like everywhere
//! else.

use serde_json::{Value, json};

use super::super::args::{self, TIME_FORMATS, ToolError};
use super::super::{ToolCtx, ToolOutcome};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// The fields of a `Person` a report row shows: enough to read the report without
/// another lookup. The home unit matters because a location's activity includes
/// visitors, members who belong to none of its units. Never their email.
const PERSON: &str = "person { id firstName lastName memberNumber location { id name } }";
const CATEGORY: &str = "category { id name isVirtual }";

/// The longest range `periodSummaryByDayByMember` accepts (366 days, in seconds).
const MAX_DAY_MEMBER_RANGE: i64 = 366 * 24 * 60 * 60;

/// One `groupBy` value: the resolver it calls, the fields selected from it, whether the
/// resolver takes a `categories` filter, and the key the rows are returned under.
struct Grouping {
    group_by: &'static str,
    field: &'static str,
    takes_categories: bool,
    root: &'static str,
}

const GROUPINGS: &[Grouping] = &[
    Grouping {
        group_by: "member",
        field: "periodSummaryByMember",
        takes_categories: true,
        root: "members",
    },
    Grouping {
        group_by: "category",
        field: "periodSummaryByCategory",
        takes_categories: false,
        root: "categories",
    },
    Grouping {
        group_by: "member_category",
        field: "periodSummaryByMemberByCategory",
        takes_categories: false,
        root: "members",
    },
    Grouping {
        group_by: "category_member",
        field: "periodSummaryByCategoryByMember",
        takes_categories: false,
        root: "categories",
    },
    Grouping {
        group_by: "day_category_member",
        field: "periodSummaryByDayByCategoryByMember",
        takes_categories: false,
        root: "days",
    },
    Grouping {
        group_by: "day_member",
        field: "periodSummaryByDayByMember",
        takes_categories: true,
        root: "days",
    },
];

impl Grouping {
    /// The sub-selection for this grouping, matching the admin page that shows it.
    fn selection(&self) -> String {
        match self.group_by {
            "member" => format!("{PERSON} totalTime totalTimeVirtual"),
            "category" => format!("{CATEGORY} totalTime"),
            "member_category" => {
                format!("{PERSON} totalTime categories {{ {CATEGORY} totalTime }}")
            }
            "category_member" => {
                format!("{CATEGORY} totalTime members {{ {PERSON} totalTime }}")
            }
            "day_category_member" => format!(
                "date totalTime categories {{ {CATEGORY} totalTime members {{ {PERSON} totalTime }} }}"
            ),
            _ => format!("date members {{ {PERSON} totalTime periodCount }}"),
        }
    }
}

/// The dashboard's fields, as the Home page shows them. `categoryName` is already
/// resolved by the server.
const DASHBOARD_SELECTION: &str = "asOf totalMembers activeMembers24H activeMembers24HVirtual \
     activeMembers24HNonVirtual activeMembers30D checkIns24H checkIns24HVirtual checkIns7D \
     checkIns7DVirtual totalTime7D totalTime7DVirtual avgCompletedDuration7D \
     avgCompletedDuration7DVirtual avgCompletedDuration7DNonVirtual totalKiosks onlineKiosks \
     recentlyActiveKiosks lastSuccessfulMemberSync \
     dailyPeriods7D { dayStart periodCount totalTime periodCountVirtual totalTimeVirtual } \
     topCategories7D { categoryId categoryName periodCount totalTime isVirtual }";

/// A time argument's JSON-Schema property.
fn time_property(what: &str) -> Value {
    json!({ "type": ["integer", "string"], "description": format!("{what} {TIME_FORMATS}") })
}

pub(super) fn catalogue() -> Vec<Value> {
    let location_id = || id_property("The location's id, from `list_locations`.");
    let group_values: Vec<&str> = GROUPINGS.iter().map(|g| g.group_by).collect();
    vec![
        tool(
            "get_dashboard",
            "Get dashboard",
            "The admin Home page's numbers for a location: total members; members active in \
             the last 24 hours (and the virtual / non-virtual split) and 30 days; check-ins \
             in the last 24 hours and 7 days; total time and average completed duration over \
             7 days; kiosk counts (total, online, recently active); the last successful \
             member sync; `dailyPeriods7D` (periods and time per day) and `topCategories7D` \
             (with category names). All times and durations are in seconds. The 24 hour, 7 \
             and 30 day windows end at `asOf` (default: now). `dayStart` is the start of a \
             UTC day (the daily buckets are UTC days, not Sydney days), shown beside its \
             Sydney time. Guests are not counted, and a period counts on the day it \
             started. Needs an Admin, Read only or super-user grant for the location.",
            input_schema(
                json!({
                    "locationId": location_id(),
                    "asOf": time_property("Measure as at this time instead of now."),
                }),
                &["locationId"],
            ),
            Effect::Read,
        ),
        tool(
            "summarize_activity",
            "Summarize activity",
            "Total time members spent signed in at a location over a date range, grouped by \
             `groupBy`: `member` (one row per member: `members`, with `totalTime` and \
             `totalTimeVirtual`), `category` (`categories`), `member_category` (`members`, \
             each with their `categories`), `category_member` (`categories`, each with its \
             `members`), `day_category_member` (`days`, newest first, each with its \
             `categories` and their `members`) or `day_member` (`days`, oldest first, each \
             with its `members`, who also have `periodCount`; the heatmap data, limited to a \
             366 day range). These are the admin Activity totals, breakdown, daily \
             breakdown and heatmap pages. `start`/`end` select periods by when they \
             *started* (same rule as `list_periods`) and `start` must be before `end`. \
             Durations (`totalTime`, `totalTimeVirtual`) are in seconds and cover completed \
             periods only (a period still open counts only in `periodCount`, in \
             `day_member`). Periods are summed whole, so one that crosses midnight counts \
             entirely on the day it started. A day's `date` is a `YYYY-MM-DD` string in \
             Australia/Sydney (not a timestamp). Guests are left out of the member \
             groupings, and visitors (members who do not belong to the location, home or otherwise) are \
             included (`person.location` is the member's home unit). Only `member` and `day_member` take `categoryIds`; for any \
             other grouping passing it is an error. Needs an Admin, Read only or super-user \
             grant for the location. For the individual periods behind a report (the Reports \
             page export) use `list_periods`.",
            input_schema(
                json!({
                    "locationId": location_id(),
                    "start": time_property("Include periods that started at or after this time."),
                    "end": time_property("Include periods that started at or before this time."),
                    "groupBy": {
                        "type": "string",
                        "enum": group_values,
                        "description": "How to group the totals (see the tool description).",
                    },
                    "categoryIds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Only count periods in one of these categories (ids \
                            from `list_categories`). Only valid with `groupBy` `member` or \
                            `day_member`.",
                    },
                }),
                &["locationId", "start", "end", "groupBy"],
            ),
            Effect::Read,
        ),
        tool(
            "members_last_seen",
            "Members last seen",
            "When each active member of a location was last seen (the admin Last seen page): \
             `lastSeen` is the end of their most recent period, or its start if it is still \
             open, at any location; null if they have never signed in. Includes the \
             member's name and number, `lastPeriod` and `secondsAgo` (a duration in seconds, \
             measured now). Members of that unit only. `sort` is `recent` (default), \
             `least_recent` or `alphabetical`; never-seen members always come last except \
             in `alphabetical`. Needs an Admin, Read only or super-user grant for the \
             location.",
            input_schema(
                json!({
                    "locationId": location_id(),
                    "sort": {
                        "type": "string",
                        "enum": ["recent", "least_recent", "alphabetical"],
                        "description": "Order of the list (default `recent`).",
                    },
                }),
                &["locationId"],
            ),
            Effect::Read,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "get_dashboard" => get_dashboard(ctx, a).await,
        "summarize_activity" => summarize_activity(ctx, a).await,
        "members_last_seen" => members_last_seen(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

async fn get_dashboard<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    let as_of = args::opt_time_arg(a, "asOf")?;
    ctx.require_location_read(location_id)?;

    let doc = format!(
        "query McpDashboard($id: ID!, $asOf: Int) {{ location(id: $id) {{ id name \
         dashboardSummary(asOf: $asOf) {{ {DASHBOARD_SELECTION} }} }} }}"
    );
    let data = ctx
        .graphql(&doc, json!({ "id": location_id, "asOf": as_of }))
        .await?;
    let location = &data["location"];
    Ok(json!({
        "location": { "id": location["id"], "name": location["name"] },
        "dashboard": location["dashboardSummary"],
    }))
}

async fn summarize_activity<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    let start = args::time_arg(a, "start")?;
    let end = args::time_arg(a, "end")?;
    let group_by = args::str_arg(a, "groupBy")?;
    let grouping = GROUPINGS
        .iter()
        .find(|g| g.group_by == group_by)
        .ok_or_else(|| {
            let allowed: Vec<&str> = GROUPINGS.iter().map(|g| g.group_by).collect();
            vec![format!(
                "Invalid \"groupBy\" argument: \"{group_by}\" is not one of {}",
                allowed.join(", ")
            )]
        })?;
    // An empty list means "no filter", as the admin site treats it.
    let categories = args::opt_str_array_arg(a, "categoryIds")?.filter(|c| !c.is_empty());
    if categories.is_some() && !grouping.takes_categories {
        let supported: Vec<&str> = GROUPINGS
            .iter()
            .filter(|g| g.takes_categories)
            .map(|g| g.group_by)
            .collect();
        return Err(vec![format!(
            "\"categoryIds\" is not supported with groupBy \"{group_by}\" (only {}): leave it \
             out, or use `list_periods` with `categoryIds` for a category-filtered list",
            supported.join(", ")
        )]);
    }
    if start >= end {
        return Err(vec!["\"start\" must be before \"end\"".into()]);
    }
    if group_by == "day_member" && end - start > MAX_DAY_MEMBER_RANGE {
        return Err(vec![
            "The range for groupBy \"day_member\" must not exceed 366 days".into(),
        ]);
    }
    ctx.require_location_read(location_id)?;

    let (declaration, argument) = if grouping.takes_categories {
        (", $categories: [ID!]", ", categories: $categories")
    } else {
        ("", "")
    };
    let doc = format!(
        "query McpActivitySummary($id: ID!, $start: Int!, $end: Int!{declaration}) {{ \
         location(id: $id) {{ {field}(startTime: $start, endTime: $end{argument}) {{ \
         {selection} }} }} }}",
        field = grouping.field,
        selection = grouping.selection(),
    );
    let mut vars = json!({ "id": location_id, "start": start, "end": end });
    if grouping.takes_categories {
        vars["categories"] = json!(categories);
    }
    let mut data = ctx.graphql(&doc, vars).await?;
    Ok(json!({
        "groupBy": group_by,
        "startTime": start,
        "endTime": end,
        grouping.root: data["location"][grouping.field].take(),
    }))
}

async fn members_last_seen<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    let sort = args::opt_str_arg(a, "sort")?.unwrap_or("recent");
    if !["recent", "least_recent", "alphabetical"].contains(&sort) {
        return Err(vec![format!(
            "Invalid \"sort\" argument: \"{sort}\" is not one of recent, least_recent, alphabetical"
        )]);
    }
    // `Query.location` and `Location.people` don't check grants themselves.
    ctx.require_location_read(location_id)?;

    let doc = "query McpLastSeen($id: ID!) { location(id: $id) { people { id firstName lastName \
               memberNumber lastPeriod { id locationId startTime endTime } } } }";
    let data = ctx.graphql(doc, json!({ "id": location_id })).await?;
    let now = crate::clock::now_sec() as i64;
    let mut members: Vec<Value> = data["location"]["people"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for member in &mut members {
        let last = &member["lastPeriod"];
        let last_seen = last["endTime"]
            .as_i64()
            .or_else(|| last["startTime"].as_i64());
        member["lastSeen"] = json!(last_seen);
        member["secondsAgo"] = json!(last_seen.map(|t| (now - t).max(0)));
    }
    let name = |m: &Value| {
        (
            m["lastName"].as_str().unwrap_or_default().to_lowercase(),
            m["firstName"].as_str().unwrap_or_default().to_lowercase(),
        )
    };
    members.sort_by(|x, y| {
        let (lx, ly) = (x["lastSeen"].as_i64(), y["lastSeen"].as_i64());
        match sort {
            "alphabetical" => name(x).cmp(&name(y)),
            // Never seen (None) sorts last in both time orders.
            "least_recent" => match (lx, ly) {
                (Some(p), Some(q)) => p.cmp(&q),
                (None, None) => std::cmp::Ordering::Equal,
                (None, _) => std::cmp::Ordering::Greater,
                (_, None) => std::cmp::Ordering::Less,
            },
            _ => match (lx, ly) {
                (Some(p), Some(q)) => q.cmp(&p),
                (None, None) => std::cmp::Ordering::Equal,
                (None, _) => std::cmp::Ordering::Greater,
                (_, None) => std::cmp::Ordering::Less,
            },
        }
        .then_with(|| name(x).cmp(&name(y)))
    });
    Ok(json!({ "members": members }))
}
