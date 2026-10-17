//! Location tools: `list_locations`, `get_location`.
//!
//! `Query.location` only requires a signed-in caller, so `get_location` checks the
//! caller's grants itself (see [`ToolCtx::require_location_read`]).

use serde_json::{Value, json};

use super::super::args::{self, ToolError};
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
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_locations" => list_locations(ctx).await,
        "get_location" => get_location(ctx, a).await,
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
