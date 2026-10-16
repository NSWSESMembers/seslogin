//! Category tools: `list_categories`, `get_category`.
//!
//! Categories are a global list (the activity types a period is filed under), not
//! owned by a location, so GraphQL lets any signed-in caller read them and these
//! tools do the same. Creating and changing them is super-user administration and
//! is not part of this module.

use serde_json::{Value, json};

use super::super::args::{self, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A `Category`'s fields, as the Categories page shows them.
const CATEGORY_SELECTION: &str = "id name enabled isVirtual nitcGroupId nitcParticipantType \
     nitcGroup { id nitcType sesTags { id name } } createdAt updatedAt";

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
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_categories" => list_categories(ctx, a).await,
        "get_category" => get_category(ctx, a).await,
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
