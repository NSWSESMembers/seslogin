//! NITC tools: `list_nitc_groups`, `get_nitc_group`, `create_nitc_group`,
//! `update_nitc_group`, `delete_nitc_group`, plus the reference data the NITC and
//! category forms offer: `list_ses_nonincident_tags` and `list_ses_reference_data`.
//!
//! A NITC (non-incident activity) group says which SES non-incident type, and which
//! SES tags, a category's periods export as. Every resolver here is `SuperUser`-guarded
//! (`nitcGroups`, `nitcGroup`, `ses_nonincident_tags`, `ses_nonincident_types`,
//! `ses_participant_types` and the group mutations), so a tool only ever reaches them
//! with a super user's `AuthInfo`.
//!
//! The site's edit form sends the type and the whole tag list, and `updateNitcGroup`
//! takes both, so `update_nitc_group` fetches the group and overlays only what was
//! passed. The group's `sesTags` only lists tags still in the tag table, so a partial
//! update re-sends just those, as saving the site's form would.
//!
//! `ses_nonincident_types` and `ses_participant_types` ask the SES API (cached by the
//! server); the tag list is the locally loaded table. `deleteNitcGroup` does not check
//! whether categories still point at the group, so `list_nitc_groups` and
//! `get_nitc_group` show the `categories` using it and the delete description warns.

use serde_json::{Value, json};

use super::super::args::{self, Field, Kind, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A `NitcGroup`'s fields, as the NITC groups pages show them.
const GROUP_SELECTION: &str = "id nitcType sesTags { id name primaryActivityName } \
     createdAt updatedAt";

pub(super) fn catalogue() -> Vec<Value> {
    let group_id = || id_property("The NITC group's id, from `list_nitc_groups`.");
    let tag_ids = |description: &str| {
        json!({
            "type": "array",
            "items": { "type": "integer" },
            "description": description,
        })
    };
    vec![
        tool(
            "list_nitc_groups",
            "List NITC groups",
            "List the NITC (non-incident activity) groups, super user only: id, the SES \
             non-incident type (`nitcType`), its SES tags, and the `categories` (id and \
             name) that export as the group. A category points at a group with \
             `create_category`/`update_category`'s `nitcGroupId`.",
            input_schema(json!({}), &[]),
            Effect::Read,
        ),
        tool(
            "get_nitc_group",
            "Get NITC group",
            "One NITC group in the same shape as `list_nitc_groups`. Super user only.",
            input_schema(json!({ "id": group_id() }), &["id"]),
            Effect::Read,
        ),
        tool(
            "create_nitc_group",
            "Create NITC group",
            "Create a NITC group (super user only). `nitcType` must be one of \
             `list_ses_reference_data`'s `nonincidentTypes`; `nitcTagIds` are ids from \
             `list_ses_nonincident_tags` (may be empty). `id` is optional: leave it out \
             and one is generated.",
            input_schema(
                json!({
                    "id": { "type": "string", "description": "Optional id to give the group." },
                    "nitcType": {
                        "type": "string",
                        "description": "An SES non-incident type, from `list_ses_reference_data`.",
                    },
                    "nitcTagIds": tag_ids("SES tag ids, from `list_ses_nonincident_tags`."),
                }),
                &["nitcType", "nitcTagIds"],
            ),
            Effect::Create,
        ),
        tool(
            "update_nitc_group",
            "Update NITC group",
            "Change a NITC group (super user only). Only the fields you pass change; \
             omitted ones keep their current value. `nitcTagIds` replaces the whole tag \
             list, so start from `get_nitc_group`'s `sesTags`; `[]` removes every tag.",
            input_schema(
                json!({
                    "id": group_id(),
                    "nitcType": {
                        "type": "string",
                        "description": "An SES non-incident type, from `list_ses_reference_data`.",
                    },
                    "nitcTagIds": tag_ids(
                        "The complete new list of SES tag ids, from `list_ses_nonincident_tags`.",
                    ),
                }),
                &["id"],
            ),
            Effect::Update,
        ),
        tool(
            "delete_nitc_group",
            "Delete NITC group",
            "Delete a NITC group (super user only). The backend does not check whether \
             categories still use it, and those would be left pointing at a group that \
             no longer exists, so check `categories` in `get_nitc_group` and clear or \
             change each with `update_category` first. This cannot be undone from here.",
            input_schema(json!({ "id": group_id() }), &["id"]),
            Effect::Destructive,
        ),
        tool(
            "list_ses_nonincident_tags",
            "List SES non-incident tags",
            "List the SES non-incident tags a NITC group can carry: `id`, `name` and \
             `primaryActivityName`, by id. These are the ids `create_nitc_group` and \
             `update_nitc_group` take in `nitcTagIds`. Super user only.",
            input_schema(json!({}), &[]),
            Effect::Read,
        ),
        tool(
            "list_ses_reference_data",
            "List SES reference data",
            "The SES value lists the NITC and category forms offer: `nonincidentTypes` \
             (valid `nitcType` for a NITC group) and `participantTypes` (valid \
             `nitcParticipantType` for a category). Fetched from the SES API, so it can \
             fail when SES is unreachable. Super user only.",
            input_schema(json!({}), &[]),
            Effect::Read,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_nitc_groups" => list_nitc_groups(ctx).await,
        "get_nitc_group" => get_nitc_group(ctx, a).await,
        "create_nitc_group" => create_nitc_group(ctx, a).await,
        "update_nitc_group" => update_nitc_group(ctx, a).await,
        "delete_nitc_group" => delete_nitc_group(ctx, a).await,
        "list_ses_nonincident_tags" => list_ses_nonincident_tags(ctx).await,
        "list_ses_reference_data" => list_ses_reference_data(ctx).await,
        _ => return None,
    };
    Some(result.into())
}

/// Attach to each group the categories whose `nitcGroupId` is its id.
fn attach_categories(groups: &mut [Value], categories: &[Value]) {
    for group in groups {
        let used: Vec<Value> = categories
            .iter()
            .filter(|c| c["nitcGroupId"] == group["id"])
            .map(|c| json!({ "id": c["id"], "name": c["name"] }))
            .collect();
        group["categories"] = Value::Array(used);
    }
}

async fn list_nitc_groups<A>(ctx: &ToolCtx<'_, A>) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!(
        "query McpNitcGroups {{ nitcGroups {{ {GROUP_SELECTION} }} \
         categories {{ id name nitcGroupId }} }}"
    );
    let data = ctx.graphql(&doc, json!({})).await?;
    let mut groups = data["nitcGroups"].as_array().cloned().unwrap_or_default();
    let categories = data["categories"].as_array().cloned().unwrap_or_default();
    groups.sort_by_key(|g| g["id"].as_str().unwrap_or_default().to_string());
    attach_categories(&mut groups, &categories);
    Ok(json!({ "nitcGroups": groups }))
}

/// Fetch a group (super user only, by its resolver) with the categories using it.
async fn fetch_group<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!(
        "query McpNitcGroup($id: ID!) {{ nitcGroup(id: $id) {{ {GROUP_SELECTION} }} \
         categories {{ id name nitcGroupId }} }}"
    );
    let data = ctx.graphql(&doc, json!({ "id": id })).await?;
    let mut group = [data["nitcGroup"].clone()];
    let categories = data["categories"].as_array().cloned().unwrap_or_default();
    attach_categories(&mut group, &categories);
    let [group] = group;
    Ok(group)
}

async fn get_nitc_group<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let group = fetch_group(ctx, args::str_arg(a, "id")?).await?;
    Ok(json!({ "nitcGroup": group }))
}

/// Refuse tag ids that are not in the tag table. The site can only offer real tags;
/// the mutation would store any number.
async fn check_tag_ids<A>(ctx: &ToolCtx<'_, A>, ids: &[i32]) -> Result<(), ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    if ids.is_empty() {
        return Ok(());
    }
    let data = ctx
        .graphql(
            "query McpNitcTagIds { ses_nonincident_tags { id } }",
            json!({}),
        )
        .await?;
    let known: Vec<i64> = data["ses_nonincident_tags"]
        .as_array()
        .map(|tags| {
            tags.iter()
                .filter_map(|t| t["id"].as_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    match ids.iter().find(|id| !known.contains(&i64::from(**id))) {
        Some(bad) => Err(vec![format!(
            "Unknown SES tag id {bad}; see `list_ses_nonincident_tags`"
        )]),
        None => Ok(()),
    }
}

fn check_type(a: &Value) -> Result<(), ToolError> {
    match args::opt_str_arg(a, "nitcType")? {
        Some(t) if t.trim().is_empty() => Err(vec!["\"nitcType\" must not be empty".into()]),
        _ => Ok(()),
    }
}

async fn create_nitc_group<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let nitc_type = args::str_arg(a, "nitcType")?;
    check_type(a)?;
    let tag_ids = args::opt_i32_array_arg(a, "nitcTagIds")?
        .ok_or_else(|| vec!["Missing or invalid \"nitcTagIds\" argument".to_string()])?;
    check_tag_ids(ctx, &tag_ids).await?;
    let vars = json!({
        "id": args::opt_str_arg(a, "id")?,
        "nitcType": nitc_type,
        "nitcTagIds": tag_ids,
    });
    let doc = format!(
        "mutation McpCreateNitcGroup($id: String, $nitcType: String!, $nitcTagIds: [Int!]!) {{ \
         createNitcGroup(id: $id, nitcType: $nitcType, nitcTagIds: $nitcTagIds) \
         {{ {GROUP_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, vars).await,
        "createNitcGroup",
        "nitcGroup",
    )
}

async fn update_nitc_group<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    check_type(a)?;
    let new_tag_ids = args::opt_i32_array_arg(a, "nitcTagIds")?;
    if args::has_arg(a, "nitcTagIds") && new_tag_ids.is_none() {
        return Err(vec!["Invalid \"nitcTagIds\" argument".into()]);
    }
    let current = fetch_group(ctx, id).await?;
    let mut vars = args::merge_update(&current, a, &[Field::new("nitcType", Kind::Str)])?;
    let tag_ids = match new_tag_ids {
        Some(ids) => {
            check_tag_ids(ctx, &ids).await?;
            ids
        }
        // `sesTags` ids are strings of the numeric tag id.
        None => current["sesTags"]
            .as_array()
            .map(|tags| {
                tags.iter()
                    .filter_map(|t| t["id"].as_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default(),
    };
    vars.insert("id".into(), json!(id));
    vars.insert("nitcTagIds".into(), json!(tag_ids));
    let doc = format!(
        "mutation McpUpdateNitcGroup($id: ID!, $nitcType: String!, $nitcTagIds: [Int!]!) {{ \
         updateNitcGroup(id: $id, nitcType: $nitcType, nitcTagIds: $nitcTagIds) \
         {{ {GROUP_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, Value::Object(vars)).await,
        "updateNitcGroup",
        "nitcGroup",
    )
}

async fn delete_nitc_group<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpDeleteNitcGroup($id: ID!) { deleteNitcGroup(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({ "deleted": true, "id": id }))
}

async fn list_ses_nonincident_tags<A>(ctx: &ToolCtx<'_, A>) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let data = ctx
        .graphql(
            "query McpSesTags { ses_nonincident_tags { id name primaryActivityName } }",
            json!({}),
        )
        .await?;
    Ok(json!({ "tags": data["ses_nonincident_tags"] }))
}

async fn list_ses_reference_data<A>(ctx: &ToolCtx<'_, A>) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let data = ctx
        .graphql(
            "query McpSesReference { ses_nonincident_types ses_participant_types }",
            json!({}),
        )
        .await?;
    let sorted = |v: &Value| {
        let mut items: Vec<Value> = v.as_array().cloned().unwrap_or_default();
        items.sort_by_key(|i| i.as_str().unwrap_or_default().to_string());
        Value::Array(items)
    };
    Ok(json!({
        "referenceData": {
            "nonincidentTypes": sorted(&data["ses_nonincident_types"]),
            "participantTypes": sorted(&data["ses_participant_types"]),
        },
    }))
}
