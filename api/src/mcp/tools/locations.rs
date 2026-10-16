//! Location tools: `list_locations`, `get_location`, `create_location`,
//! `update_location`.
//!
//! `Query.location` only requires a signed-in caller, so `get_location` checks the
//! caller's grants itself (see [`ToolCtx::require_location_read`]).
//!
//! `createLocation` and `updateLocation` are `SuperUser`-guarded. The admin Location
//! pages (`LocationNew`, `LocationEdit`) only ever set the name, `enabled`,
//! `nitcEnabled` and `nitcCompleteOnExport`; no mutation sets a location's SES
//! headquarters (location sync owns `sesApiHeadquartersId`), so there is nothing to
//! cover there. `updateLocation` takes every field and resets the ones it is not given,
//! so `update_location` fetches the location and overlays only the arguments passed.

use serde_json::{Value, json};

use super::super::args::{self, Field, Kind, TIME_FORMATS, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// The fields `list_locations` returns for each location.
const LOCATION_LIST_SELECTION: &str =
    "id name enabled viewerCanEdit nitcEnabled lastSuccessfulMemberSync";

/// `get_location` adds the rest of what the admin site's location pages show.
const LOCATION_SELECTION: &str = "id name enabled viewerCanEdit nitcEnabled nitcCompleteOnExport \
     sesApiHeadquartersId lastSuccessfulMemberSync createdAt updatedAt";

pub(super) fn catalogue() -> Vec<Value> {
    vec![
        json!({
            "name": "list_locations",
            "title": "List locations",
            "description": "List the seslogin locations (units) you can access: id, name, \
                whether it's enabled, whether you may make changes there (`viewerCanEdit`: \
                true for Admin, false for Read only), when NITC was enabled \
                (`nitcEnabled`, null if never) and the last successful member sync. A \
                super user sees every enabled location; anyone else sees the ones they \
                have an Admin or Read only grant for. Call this first to get the \
                location ids every other tool takes, and to resolve a location's name to \
                the id `create_user`/`update_user` expect in `locationGrants`.",
            "inputSchema": { "type": "object", "properties": {} },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "locations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "name": { "type": "string" },
                                "enabled": { "type": "boolean" },
                                "viewerCanEdit": { "type": "boolean" },
                                "nitcEnabled": { "type": ["integer", "null"] },
                                "lastSuccessfulMemberSync": { "type": ["integer", "null"] },
                            },
                        },
                    },
                },
                "required": ["locations"],
            },
            "annotations": { "title": "List locations", "readOnlyHint": true, "idempotentHint": true },
        }),
        tool(
            "get_location",
            "Get location",
            "Details of one location: name, enabled, `viewerCanEdit`, NITC settings, its \
             SES headquarters id, last successful member sync and created/updated times. \
             Does not include members (see `list_members`) or activity. Needs an Admin, \
             Read only or super-user grant for the location; get ids from `list_locations`.",
            input_schema(
                json!({ "id": id_property("The location's id, from `list_locations`.") }),
                &["id"],
            ),
            Effect::Read,
        ),
        tool(
            "create_location",
            "Create location",
            &format!(
                "Create a location (unit), enabled, with no members, kiosks or users yet \
                 (super user only). Optionally set `nitcEnabled`, the date NITC export \
                 starts from; leave it out for no export. Member sync fills in the SES \
                 headquarters link and members later. Grant users access with \
                 `update_user`'s `locationGrants`. {TIME_FORMATS}"
            ),
            input_schema(
                json!({
                    "name": { "type": "string", "description": "The location's name." },
                    "nitcEnabled": {
                        "type": ["string", "integer"],
                        "description": format!("When NITC export starts. {TIME_FORMATS}"),
                    },
                }),
                &["name"],
            ),
            Effect::Create,
        ),
        tool(
            "update_location",
            "Update location",
            &format!(
                "Change a location (super user only). Only the fields you pass change; \
                 anything omitted keeps its current value. `enabled: false` disables it \
                 (it drops out of `list_locations`' super-user list). `nitcEnabled` is \
                 when NITC export starts: pass a time to set it, `null` to turn export off. \
                 `nitcCompleteOnExport` is whether exported NITCs are marked completed in SES \
                 (false leaves them incomplete for review there). {TIME_FORMATS} Get ids from \
                 `list_locations`."
            ),
            input_schema(
                json!({
                    "id": id_property("The location's id, from `list_locations`."),
                    "name": { "type": "string", "description": "New name." },
                    "enabled": { "type": "boolean", "description": "Whether it is in use." },
                    "nitcEnabled": {
                        "type": ["string", "integer", "null"],
                        "description": format!(
                            "When NITC export starts; null turns export off. {TIME_FORMATS}"
                        ),
                    },
                    "nitcCompleteOnExport": {
                        "type": "boolean",
                        "description": "Mark exported NITCs completed in SES.",
                    },
                }),
                &["id"],
            ),
            Effect::Update,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_locations" => list_locations(ctx).await,
        "get_location" => get_location(ctx, a).await,
        "create_location" => create_location(ctx, a).await,
        "update_location" => update_location(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

async fn list_locations<A>(ctx: &ToolCtx<'_, A>) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    if ctx.is_super() {
        let doc = format!("query McpLocations {{ locations {{ {LOCATION_LIST_SELECTION} }} }}");
        ctx.graphql(&doc, json!({}))
            .await
            .map(|data| json!({ "locations": data["locations"] }))
    } else {
        let doc = format!(
            "query McpMyLocations {{ user {{ locations {{ {LOCATION_LIST_SELECTION} }} }} }}"
        );
        ctx.graphql(&doc, json!({}))
            .await
            .map(|data| json!({ "locations": data["user"]["locations"] }))
    }
}

async fn get_location<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.require_location_read(id)?;
    let doc =
        format!("query McpLocation($id: ID!) {{ location(id: $id) {{ {LOCATION_SELECTION} }} }}");
    rename_root(
        ctx.graphql(&doc, json!({ "id": id })).await,
        "location",
        "location",
    )
}

async fn create_location<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let name = args::str_arg(a, "name")?;
    if name.trim().is_empty() {
        return Err(vec!["\"name\" must not be empty".into()]);
    }
    let vars = json!({ "name": name, "nitcEnabled": args::opt_time_arg(a, "nitcEnabled")? });
    let doc = format!(
        "mutation McpCreateLocation($name: String!, $nitcEnabled: Int) {{ \
         createLocation(name: $name, nitcEnabled: $nitcEnabled) {{ {LOCATION_SELECTION} }} }}"
    );
    rename_root(ctx.graphql(&doc, vars).await, "createLocation", "location")
}

async fn update_location<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    if args::opt_str_arg(a, "name")?.is_some_and(|n| n.trim().is_empty()) {
        return Err(vec!["\"name\" must not be empty".into()]);
    }
    // `Query.location` only needs a sign-in, so check the grant before reading; the
    // mutation then refuses anyone who is not a super user.
    ctx.require_location_read(id)?;
    let doc =
        format!("query McpLocation($id: ID!) {{ location(id: $id) {{ {LOCATION_SELECTION} }} }}");
    let current = ctx.graphql(&doc, json!({ "id": id })).await?;
    let mut vars = args::merge_update(
        &current["location"],
        a,
        &[
            Field::new("name", Kind::Str),
            Field::new("enabled", Kind::Bool),
            Field::new("nitcEnabled", Kind::Time).nullable(),
            Field::new("nitcCompleteOnExport", Kind::Bool),
        ],
    )?;
    vars.insert("id".into(), json!(id));
    let doc = format!(
        "mutation McpUpdateLocation($id: ID!, $name: String!, $enabled: Boolean!, \
         $nitcEnabled: Int, $nitcCompleteOnExport: Boolean) {{ \
         updateLocation(id: $id, name: $name, enabled: $enabled, nitcEnabled: $nitcEnabled, \
         nitcCompleteOnExport: $nitcCompleteOnExport) {{ {LOCATION_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, Value::Object(vars)).await,
        "updateLocation",
        "location",
    )
}
