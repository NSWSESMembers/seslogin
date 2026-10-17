//! The tool registry: one file per domain, chained here.
//!
//! A domain module exposes `catalogue()` (its `tools/list` entries) and an async
//! `call()` that returns `None` for a tool name that isn't its own. Adding a domain
//! means a `mod` line and one line in each of [`catalogue`] and [`call`]; the
//! telemetry "known tool" check reads [`catalogue`], so it follows automatically.

use serde_json::{Value, json};

use super::{ToolCtx, ToolOutcome};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

mod categories;
mod kiosks;
mod locations;
mod members;
mod periods;
mod reports;
mod users;

/// Every tool's `tools/list` entry, in a stable order.
pub(super) fn catalogue() -> Vec<Value> {
    let mut tools = users::catalogue();
    tools.extend(locations::catalogue());
    tools.extend(members::catalogue());
    tools.extend(categories::catalogue());
    tools.extend(periods::catalogue());
    tools.extend(reports::catalogue());
    tools.extend(kiosks::catalogue());
    tools
}

/// Run the named tool, or `None` if no domain has it.
pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, args: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    if let Some(outcome) = users::call(ctx, name, args).await {
        return Some(outcome);
    }
    if let Some(outcome) = locations::call(ctx, name, args).await {
        return Some(outcome);
    }
    if let Some(outcome) = members::call(ctx, name, args).await {
        return Some(outcome);
    }
    if let Some(outcome) = categories::call(ctx, name, args).await {
        return Some(outcome);
    }
    if let Some(outcome) = periods::call(ctx, name, args).await {
        return Some(outcome);
    }
    if let Some(outcome) = reports::call(ctx, name, args).await {
        return Some(outcome);
    }
    kiosks::call(ctx, name, args).await
}

/// Which MCP annotations a tool carries (see the module docs of `mcp`).
#[derive(Clone, Copy)]
pub(super) enum Effect {
    /// Reads only: `readOnlyHint` + `idempotentHint`.
    Read,
    /// Creates something: no hints beyond the title.
    Create,
    /// Changes something, and repeating the same call changes nothing more.
    Update,
    /// Deletes or revokes: `destructiveHint`.
    Destructive,
    /// Reaches outside the system (sends email, queues work): `openWorldHint`.
    External,
}

/// Build a `tools/list` entry. New tools omit `outputSchema` (optional in the spec)
/// but still return `structuredContent` as an object with one root key.
pub(super) fn tool(
    name: &str,
    title: &str,
    description: &str,
    input_schema: Value,
    effect: Effect,
) -> Value {
    let annotations = match effect {
        Effect::Read => json!({ "title": title, "readOnlyHint": true, "idempotentHint": true }),
        Effect::Create => json!({ "title": title }),
        Effect::Update => json!({ "title": title, "idempotentHint": true }),
        Effect::Destructive => json!({ "title": title, "destructiveHint": true }),
        Effect::External => json!({ "title": title, "openWorldHint": true }),
    };
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema,
        "annotations": annotations,
    })
}

/// An `inputSchema` object from its properties and the names of the required ones.
pub(super) fn input_schema(properties: Value, required: &[&str]) -> Value {
    let mut schema = json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}

/// A JSON-Schema property for a record id.
pub(super) fn id_property(what: &str) -> Value {
    json!({ "type": "string", "description": what })
}
