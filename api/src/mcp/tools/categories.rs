//! Category tools: `list_categories`, `get_category`, `create_category`,
//! `update_category`.
//!
//! Categories are a global list (the activity types a period is filed under), not
//! owned by a location, so GraphQL lets any signed-in caller read them and these
//! tools do the same. `createCategory` and `updateCategory` are `SuperUser`-guarded.
//!
//! The admin edit form sends every field and `updateCategory` takes them all, so
//! `update_category` fetches the category and overlays only the arguments passed.
//! The backend treats an empty `nitcGroupId` / `nitcParticipantType` as "none", so
//! `null` and `""` both clear them. The valid values come from `list_nitc_groups` and
//! `list_ses_reference_data` (the same lists the site's category form offers); like
//! the site, the backend does not check them, so a tool passing junk would store junk.

use serde_json::{Value, json};

use super::super::args::{self, Field, Kind, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A `Category`'s fields, as the Categories page shows them.
const CATEGORY_SELECTION: &str = "id name enabled isVirtual nitcGroupId nitcParticipantType \
     nitcGroup { id nitcType sesTags { id name } } createdAt updatedAt";

const NITC_GROUP_HELP: &str = "The NITC group the category exports as, from `list_nitc_groups`.";
const NITC_GROUP_HELP_NULLABLE: &str = "The NITC group the category exports as, from \
    `list_nitc_groups`; null clears it.";
const PARTICIPANT_HELP: &str = "The NITC participant type, from `list_ses_reference_data`.";
const PARTICIPANT_HELP_NULLABLE: &str =
    "The NITC participant type, from `list_ses_reference_data`; null clears it.";

pub(super) fn catalogue() -> Vec<Value> {
    vec![
        tool(
            "list_categories",
            "List categories",
            "List the activity categories a period can be filed under: id, name, `enabled`, \
             `isVirtual` (virtual activity, not at the unit), and the NITC group and \
             participant type the category exports as. Disabled categories are left out \
             unless `includeDisabled` is true; they still appear on old periods. Categories \
             are shared by every location, so any signed-in user may call this. Use the ids \
             with `create_period`, `update_period` and the `categoryIds` filters.",
            input_schema(
                json!({
                    "includeDisabled": {
                        "type": "boolean",
                        "description": "Also list disabled categories (default false).",
                    },
                }),
                &[],
            ),
            Effect::Read,
        ),
        tool(
            "get_category",
            "Get category",
            "One category's details, including its NITC group and that group's SES tags. \
             Any signed-in user may call this; get ids from `list_categories`.",
            input_schema(
                json!({ "id": id_property("The category's id, from `list_categories`.") }),
                &["id"],
            ),
            Effect::Read,
        ),
        tool(
            "create_category",
            "Create category",
            "Create an activity category (super user only). `name` is required; `isVirtual` \
             (default false) marks virtual activity not done at the unit. `nitcGroupId` (an \
             id from `list_nitc_groups`) and `nitcParticipantType` (a value from \
             `list_ses_reference_data`'s `participantTypes`) set how the category exports \
             to NITC; leave both out for a category that is not exported. New categories \
             are enabled.",
            input_schema(
                json!({
                    "name": { "type": "string", "description": "The category's name." },
                    "isVirtual": {
                        "type": "boolean",
                        "description": "Virtual activity, not at the unit (default false).",
                    },
                    "nitcGroupId": { "type": "string", "description": NITC_GROUP_HELP },
                    "nitcParticipantType": {
                        "type": "string",
                        "description": PARTICIPANT_HELP,
                    },
                }),
                &["name"],
            ),
            Effect::Create,
        ),
        tool(
            "update_category",
            "Update category",
            "Change a category (super user only). Only the fields you pass change; anything \
             omitted keeps its current value. `enabled: false` hides it from the pick lists \
             (it stays on old periods) and `true` brings it back. `nitcGroupId` and \
             `nitcParticipantType` can be cleared by passing `null`; leaving them out \
             keeps them. Get ids from `list_categories` (use `includeDisabled` to find a \
             disabled one).",
            input_schema(
                json!({
                    "id": id_property("The category's id, from `list_categories`."),
                    "name": { "type": "string", "description": "New name." },
                    "enabled": { "type": "boolean", "description": "Whether it can be used." },
                    "isVirtual": { "type": "boolean", "description": "Virtual activity." },
                    "nitcGroupId": {
                        "type": ["string", "null"],
                        "description": NITC_GROUP_HELP_NULLABLE,
                    },
                    "nitcParticipantType": {
                        "type": ["string", "null"],
                        "description": PARTICIPANT_HELP_NULLABLE,
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
        "list_categories" => list_categories(ctx, a).await,
        "get_category" => get_category(ctx, a).await,
        "create_category" => create_category(ctx, a).await,
        "update_category" => update_category(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

async fn list_categories<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let include_disabled = args::opt_bool_arg(a, "includeDisabled")?.unwrap_or(false);
    let doc = format!("query McpCategories {{ categories {{ {CATEGORY_SELECTION} }} }}");
    let data = ctx.graphql(&doc, json!({})).await?;
    let mut categories: Vec<Value> = data["categories"].as_array().cloned().unwrap_or_default();
    if !include_disabled {
        categories.retain(|c| c["enabled"].as_bool().unwrap_or(false));
    }
    categories.sort_by_key(|c| c["name"].as_str().unwrap_or_default().to_lowercase());
    Ok(json!({ "categories": categories }))
}

async fn get_category<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc =
        format!("query McpCategory($id: ID!) {{ category(id: $id) {{ {CATEGORY_SELECTION} }} }}");
    rename_root(
        ctx.graphql(&doc, json!({ "id": args::str_arg(a, "id")? }))
            .await,
        "category",
        "category",
    )
}

/// A category name must not be blank (the mutation itself does not check).
fn check_name(a: &Value) -> Result<(), ToolError> {
    match args::opt_str_arg(a, "name")? {
        Some(name) if name.trim().is_empty() => Err(vec!["\"name\" must not be empty".into()]),
        _ => Ok(()),
    }
}

async fn create_category<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let name = args::str_arg(a, "name")?;
    check_name(a)?;
    let vars = json!({
        "name": name,
        "isVirtual": args::opt_bool_arg(a, "isVirtual")?.unwrap_or(false),
        "nitcGroupId": args::opt_str_arg(a, "nitcGroupId")?,
        "nitcParticipantType": args::opt_str_arg(a, "nitcParticipantType")?,
    });
    let doc = format!(
        "mutation McpCreateCategory($name: String!, $isVirtual: Boolean!, \
         $nitcGroupId: String, $nitcParticipantType: String) {{ \
         createCategory(name: $name, isVirtual: $isVirtual, nitcGroupId: $nitcGroupId, \
         nitcParticipantType: $nitcParticipantType) {{ {CATEGORY_SELECTION} }} }}"
    );
    rename_root(ctx.graphql(&doc, vars).await, "createCategory", "category")
}

async fn update_category<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    check_name(a)?;
    // The mutation takes every field, so overlay the arguments on the current record.
    // `Query.category` is open to any signed-in caller, but the mutation is super-only.
    let doc =
        format!("query McpCategory($id: ID!) {{ category(id: $id) {{ {CATEGORY_SELECTION} }} }}");
    let current = ctx.graphql(&doc, json!({ "id": id })).await?;
    let mut vars = args::merge_update(
        &current["category"],
        a,
        &[
            Field::new("name", Kind::Str),
            Field::new("enabled", Kind::Bool),
            Field::new("isVirtual", Kind::Bool),
            Field::new("nitcGroupId", Kind::Str).nullable(),
            Field::new("nitcParticipantType", Kind::Str).nullable(),
        ],
    )?;
    vars.insert("id".into(), json!(id));
    let doc = format!(
        "mutation McpUpdateCategory($id: ID!, $name: String!, $enabled: Boolean!, \
         $isVirtual: Boolean!, $nitcGroupId: String, $nitcParticipantType: String) {{ \
         updateCategory(id: $id, name: $name, enabled: $enabled, isVirtual: $isVirtual, \
         nitcGroupId: $nitcGroupId, nitcParticipantType: $nitcParticipantType) \
         {{ {CATEGORY_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, Value::Object(vars)).await,
        "updateCategory",
        "category",
    )
}
