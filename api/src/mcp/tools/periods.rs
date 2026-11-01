//! Period (activity) tools: `list_periods`, `list_member_periods`, `get_period`,
//! `create_period`, `update_period`, `delete_period`, `send_period_edit_link`.
//!
//! A period is one stretch of a member's (or a guest's) time at a location. These
//! mirror the admin Activity pages: *Current* is `list_periods` with
//! `onlyActive`, *Activity* is `list_periods`, a member's history is
//! `list_member_periods`, and the Reports page is a period export, which
//! `list_periods` over a date range covers.
//!
//! `Query.period` and `Location.periods` / `Person.periods` do check the caller's
//! grant for the period's location; the tools check it again here
//! ([`ToolCtx::require_location_read`]) before returning anything, so they stay
//! safe if a resolver ever changes. Writes rely on the resolvers, which require an
//! Admin grant (a Read only user is refused).
//!
//! `update_period` follows the admin edit page: a member period goes through
//! `updatePeriodTimeCategory` (times, category, comment) and a guest period through
//! `updateGuestPeriod` (name, times, comment). The full `updatePeriod`, which can
//! move a period to another member or location, is not used by the site and is not
//! exposed.

use serde_json::{Map, Value, json};

use super::super::args::{self, Field, Kind, TIME_FORMATS, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, members, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A `Period`'s fields, as the Activity pages show them. The person is trimmed to
/// name, member number and home unit (never their email), and the sessions to the
/// kiosk names.
const PERIOD_SELECTION: &str = "id personId guestName comment locationId categoryId startTime \
     endTime version createdAt updatedAt nitcExportStatus \
     person { id firstName lastName memberNumber locationId location { id name } } \
     category { id name isVirtual } \
     signedInSession { id name } signedOutSession { id name }";

/// How many periods a list returns when `first` is omitted, and the most it may ask for
/// (GraphQL's own limit).
const DEFAULT_PAGE: i64 = 50;
const MAX_PAGE: i64 = 1000;

/// A time argument's JSON-Schema property.
fn time_property(what: &str) -> Value {
    json!({ "type": ["integer", "string"], "description": format!("{what} {TIME_FORMATS}") })
}

fn paging_properties(properties: &mut Value) {
    properties["first"] = json!({
        "type": "integer",
        "minimum": 1,
        "maximum": MAX_PAGE,
        "description": format!("Page size (default {DEFAULT_PAGE}, at most {MAX_PAGE})."),
    });
    properties["after"] = json!({
        "type": "string",
        "description": "Cursor: the previous page's `pageInfo.endCursor`, to get the next page.",
    });
}

pub(super) fn catalogue() -> Vec<Value> {
    let period_id =
        || id_property("The period's id, from `list_periods` or `list_member_periods`.");
    let mut list_properties = json!({
        "locationId": id_property("The location's id, from `list_locations`."),
        "start": time_property(
            "Only periods that started at or after this time. Give `start` and `end` together.",
        ),
        "end": time_property("Only periods that started at or before this time."),
        "onlyActive": {
            "type": "boolean",
            "description": "Only periods still open, i.e. who is signed in now (the Current page).",
        },
        "categoryIds": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Only periods in one of these categories (ids from `list_categories`).",
        },
    });
    paging_properties(&mut list_properties);
    let mut member_properties = json!({
        "memberId": id_property("The member's record id, from `list_members`."),
        "categoryIds": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Only periods in one of these categories (ids from `list_categories`).",
        },
    });
    paging_properties(&mut member_properties);

    vec![
        tool(
            "list_periods",
            "List periods",
            "List the activity periods (sign-in to sign-out) at a location, newest first, with \
             the member's name and member number (or `guestName` for a guest), category, \
             comment and the sign-in and sign-out kiosk names. Includes visitors (members who do not \
             belong to the location, home or otherwise); `person.location` is the member's home unit. `onlyActive: true` lists who \
             is signed in right now; `endTime` is null for an open period. `start`/`end` \
             select by when a period *started* and must be given together. Durations are in \
             seconds. Returns `periods` and `pageInfo` (`hasNextPage`, `endCursor`; pass \
             `endCursor` as `after` for the next page). Needs an Admin, Read only or \
             super-user grant for the location.",
            input_schema(list_properties, &["locationId"]),
            Effect::Read,
        ),
        tool(
            "list_member_periods",
            "List a member's periods",
            "List one member's activity periods across every location they signed in at, \
             newest first, in the same shape as `list_periods`. Returns `periods` and \
             `pageInfo` (pass `endCursor` as `after` for the next page). Needs a grant for the \
             member's home location; get ids from `list_members`.",
            input_schema(member_properties, &["memberId"]),
            Effect::Read,
        ),
        tool(
            "get_period",
            "Get period",
            "One activity period's details, in the same shape as `list_periods`. Needs a \
             grant for the period's location.",
            input_schema(json!({ "id": period_id() }), &["id"]),
            Effect::Read,
        ),
        tool(
            "create_period",
            "Create period",
            "Add a finished activity period for a member by hand (it cannot create an open \
             period or a guest period). `startTime` must be before `endTime`. The member may \
             be a visitor, one who does not belong to `locationId`. Needs an Admin (or super-user) grant for \
             `locationId`; Read only users are refused. Use `list_members` and \
             `list_categories` for the ids.",
            input_schema(
                json!({
                    "memberId": id_property("The member's record id, from `list_members`."),
                    "locationId": id_property("The location the activity happened at."),
                    "categoryId": id_property("The category's id, from `list_categories`."),
                    "startTime": time_property("When the activity started."),
                    "endTime": time_property("When it ended; after `startTime`."),
                    "comment": { "type": "string", "description": "Optional free-text note." },
                }),
                &[
                    "memberId",
                    "locationId",
                    "categoryId",
                    "startTime",
                    "endTime",
                ],
            ),
            Effect::Create,
        ),
        tool(
            "update_period",
            "Update period",
            "Change a period's times, comment and (member periods) category, or (guest \
             periods) guest name. Only the fields you pass change; anything omitted keeps its \
             current value, and passing `comment: null` clears the comment while leaving \
             `comment` out keeps it. A member period takes `categoryId` and rejects \
             `guestName`; a guest period (no member, no category) takes `guestName` and \
             rejects `categoryId`. An open period needs an `endTime` (and, for a member, a \
             `categoryId`) to be edited, which closes it. `startTime` must be before \
             `endTime`. The member and location cannot be changed; delete and recreate for \
             that. Needs an Admin (or super-user) grant for the period's location; Read only \
             users are refused.",
            input_schema(
                json!({
                    "id": period_id(),
                    "startTime": time_property("New start time."),
                    "endTime": time_property("New end time."),
                    "categoryId": id_property(
                        "New category (member periods only), from `list_categories`.",
                    ),
                    "guestName": {
                        "type": "string",
                        "description": "New guest name (guest periods only).",
                    },
                    "comment": {
                        "type": ["string", "null"],
                        "description": "New comment; null clears it.",
                    },
                }),
                &["id"],
            ),
            Effect::Update,
        ),
        tool(
            "delete_period",
            "Delete period",
            "Delete (soft-delete) an activity period. Needs an Admin (or super-user) grant \
             for the period's location; Read only users are refused.",
            input_schema(json!({ "id": period_id() }), &["id"]),
            Effect::Destructive,
        ),
        tool(
            "send_period_edit_link",
            "Email a period edit link",
            "Email the member a link to check and correct this time entry themselves. Sends \
             a real email to the member's address on record, so only call it when asked. A \
             second link for the same period is refused until a cooldown has passed, a guest \
             period has no member to email, and a member with no email address on record \
             cannot be sent one. Returns the address it went to. Needs an Admin (or \
             super-user) grant for the period's location.",
            input_schema(json!({ "id": period_id() }), &["id"]),
            Effect::External,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_periods" => list_periods(ctx, a).await,
        "list_member_periods" => list_member_periods(ctx, a).await,
        "get_period" => get_period(ctx, a).await,
        "create_period" => create_period(ctx, a).await,
        "update_period" => update_period(ctx, a).await,
        "delete_period" => delete_period(ctx, a).await,
        "send_period_edit_link" => send_period_edit_link(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

/// `first` and `after` from the arguments, with `first` range-checked here so the error
/// names the argument.
fn page_args(a: &Value) -> Result<(i64, Option<&str>), ToolError> {
    let first = args::opt_i64_arg(a, "first")?.unwrap_or(DEFAULT_PAGE);
    if !(1..=MAX_PAGE).contains(&first) {
        return Err(vec![format!(
            "Invalid \"first\" argument: must be between 1 and {MAX_PAGE}"
        )]);
    }
    Ok((first, args::opt_str_arg(a, "after")?))
}

/// A connection's `nodes` as `periods`, beside its `pageInfo`.
fn period_page(mut connection: Value) -> Value {
    json!({
        "periods": connection["nodes"].take(),
        "pageInfo": connection["pageInfo"].take(),
    })
}

async fn list_periods<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    let start = args::opt_time_arg(a, "start")?;
    let end = args::opt_time_arg(a, "end")?;
    match (start, end) {
        (Some(_), None) | (None, Some(_)) => {
            return Err(vec![
                "Give \"start\" and \"end\" together, or neither".into(),
            ]);
        }
        (Some(s), Some(e)) if s >= e => {
            return Err(vec!["\"start\" must be before \"end\"".into()]);
        }
        _ => {}
    }
    let (first, after) = page_args(a)?;
    let only_active = args::opt_bool_arg(a, "onlyActive")?;
    let categories = args::opt_str_array_arg(a, "categoryIds")?;
    ctx.require_location_read(location_id)?;

    let doc = format!(
        "query McpPeriods($id: ID!, $onlyActive: Boolean, $start: Int, $end: Int, \
         $categories: [ID!], $first: Int, $after: String) {{ location(id: $id) {{ \
         periods(onlyActive: $onlyActive, startTime: $start, endTime: $end, \
         categories: $categories, first: $first, after: $after) {{ \
         nodes {{ {PERIOD_SELECTION} }} pageInfo {{ hasNextPage endCursor }} }} }} }}"
    );
    let vars = json!({
        "id": location_id,
        "onlyActive": only_active,
        "start": start,
        "end": end,
        "categories": categories,
        "first": first,
        "after": after,
    });
    let data = ctx.graphql(&doc, vars).await?;
    Ok(period_page(data["location"]["periods"].clone()))
}

async fn list_member_periods<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let member_id = args::str_arg(a, "memberId")?;
    let (first, after) = page_args(a)?;
    let categories = args::opt_str_array_arg(a, "categoryIds")?;
    // `Query.person` checks every unit the member belongs to; ask first so a refusal
    // comes before any periods are fetched.
    members::require_member_read(ctx, member_id).await?;

    let doc = format!(
        "query McpMemberPeriods($id: ID!, $categories: [ID!], $first: Int, $after: String) {{ \
         person(id: $id) {{ id firstName lastName memberNumber \
         periods(categories: $categories, first: $first, after: $after) {{ \
         nodes {{ {PERIOD_SELECTION} }} pageInfo {{ hasNextPage endCursor }} }} }} }}"
    );
    let vars = json!({
        "id": member_id,
        "categories": categories,
        "first": first,
        "after": after,
    });
    let mut data = ctx.graphql(&doc, vars).await?;
    let person = &mut data["person"];
    let mut result = period_page(person["periods"].take());
    result["member"] = json!({
        "id": person["id"],
        "firstName": person["firstName"],
        "lastName": person["lastName"],
        "memberNumber": person["memberNumber"],
    });
    Ok(result)
}

/// Fetch a period and check the caller may see it.
async fn fetch_period<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!("query McpPeriod($id: ID!) {{ period(id: $id) {{ {PERIOD_SELECTION} }} }}");
    let data = ctx.graphql(&doc, json!({ "id": id })).await?;
    let period = data["period"].clone();
    ctx.require_location_read(period["locationId"].as_str().unwrap_or_default())?;
    Ok(period)
}

async fn get_period<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let period = fetch_period(ctx, args::str_arg(a, "id")?).await?;
    Ok(json!({ "period": period }))
}

async fn create_period<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!(
        "mutation McpCreatePeriod($personId: ID!, $locationId: ID!, $categoryId: ID!, \
         $startTime: Int!, $endTime: Int!, $comment: String) {{ \
         createPeriod(personId: $personId, locationId: $locationId, categoryId: $categoryId, \
         startTime: $startTime, endTime: $endTime, comment: $comment) {{ {PERIOD_SELECTION} }} }}"
    );
    let vars = json!({
        "personId": args::str_arg(a, "memberId")?,
        "locationId": args::str_arg(a, "locationId")?,
        "categoryId": args::str_arg(a, "categoryId")?,
        "startTime": args::time_arg(a, "startTime")?,
        "endTime": args::time_arg(a, "endTime")?,
        "comment": args::opt_str_arg(a, "comment")?,
    });
    rename_root(ctx.graphql(&doc, vars).await, "createPeriod", "period")
}

/// Refuse a merged value that is still null, naming what the caller must pass.
fn require_value(vars: &Map<String, Value>, name: &str, why: &str) -> Result<(), ToolError> {
    if vars[name].is_null() {
        Err(vec![format!("Pass \"{name}\": {why}")])
    } else {
        Ok(())
    }
}

async fn update_period<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    // Both mutations take every field, so overlay the arguments on the current record.
    // The read is checked here; the mutations themselves require an Admin grant.
    let current = fetch_period(ctx, id).await?;
    let is_guest = current["guestName"].is_string();

    let (document, fields, root): (&str, Vec<Field>, &str) = if is_guest {
        if args::has_arg(a, "categoryId") {
            return Err(vec![
                "A guest period has no category: leave out \"categoryId\"".into(),
            ]);
        }
        (
            "mutation McpUpdateGuestPeriod($id: ID!, $guestName: String!, $startTime: Int!, \
             $endTime: Int!, $comment: String)",
            vec![
                Field::new("guestName", Kind::Str),
                Field::new("startTime", Kind::Time),
                Field::new("endTime", Kind::Time).nullable(),
                Field::new("comment", Kind::Str).nullable(),
            ],
            "updateGuestPeriod",
        )
    } else {
        if args::has_arg(a, "guestName") {
            return Err(vec![
                "A member period has no guest name: leave out \"guestName\"".into(),
            ]);
        }
        (
            "mutation McpUpdatePeriod($id: ID!, $startTime: Int!, $endTime: Int!, \
             $categoryId: ID!, $comment: String)",
            vec![
                Field::new("startTime", Kind::Time),
                Field::new("endTime", Kind::Time).nullable(),
                Field::new("categoryId", Kind::Str).nullable(),
                Field::new("comment", Kind::Str).nullable(),
            ],
            "updatePeriodTimeCategory",
        )
    };

    let mut vars = args::merge_update(&current, a, &fields)?;
    // An open period has neither an end nor (until sign-out) a category; the mutations
    // need both, and a null passed on purpose reaches the same check.
    require_value(
        &vars,
        "endTime",
        "this period is still open, so give the time it ended",
    )?;
    if !is_guest {
        require_value(
            &vars,
            "categoryId",
            "this period has no category yet, so choose one (see `list_categories`)",
        )?;
    }
    vars.insert("id".into(), json!(id));

    let selection = format!("{{ {PERIOD_SELECTION} }}");
    let doc = if is_guest {
        format!(
            "{document} {{ {root}(id: $id, guestName: $guestName, startTime: $startTime, \
             endTime: $endTime, comment: $comment) {selection} }}"
        )
    } else {
        format!(
            "{document} {{ {root}(id: $id, startTime: $startTime, endTime: $endTime, \
             categoryId: $categoryId, comment: $comment) {selection} }}"
        )
    };
    rename_root(ctx.graphql(&doc, Value::Object(vars)).await, root, "period")
}

async fn delete_period<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpDeletePeriod($id: ID!) { deletePeriod(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({ "deleted": true, "id": id }))
}

async fn send_period_edit_link<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    let data = ctx
        .graphql(
            "mutation McpSendPeriodLink($id: ID!) { sendPeriodEditLink(id: $id) }",
            json!({ "id": id }),
        )
        .await?;
    Ok(json!({ "sent": true, "id": id, "sentTo": data["sendPeriodEditLink"] }))
}
