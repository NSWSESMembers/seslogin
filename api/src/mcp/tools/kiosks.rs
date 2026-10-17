//! Kiosk tools: `list_kiosks`, `get_kiosk`, `update_kiosk`, `delete_kiosk`.
//!
//! A kiosk is a `Session` record: a device that signs members in and out at a
//! location. These mirror the admin Kiosks page (`SessionsList`, `SessionsEdit`).
//!
//! **Deliberately not exposed:** enrolling a kiosk, `pendingEnrollmentKey` and
//! `reactivateSession`. A device is only bound to a location by a person scanning its
//! QR code in the browser, so the descriptions point the user to the admin site's
//! Kiosks page for those.
//!
//! Access: `Location.sessions` and `Query.session(id)` both check the caller's grant
//! for the kiosk's location (`Query.session` also lets a kiosk read itself, but a
//! user must pass an id and is checked the same way). The tools check it again
//! ([`ToolCtx::require_location_read`]) before returning any kiosk data, so they stay
//! safe if a resolver ever changes. `updateSession` and `deleteSession` require an
//! Admin grant for the kiosk's location, so a Read only user is refused.
//!
//! The site's edit form sends every field, with `config` as a JSON string and an
//! empty string for "no health check URL"; `updateSession` clears whatever it is not
//! given. `update_kiosk` therefore fetches the kiosk and overlays only the arguments
//! passed, taking `config` as an object. The backend does not refuse to edit or delete
//! a leftover whose key was released (`keyReleasedAt` set); the site just hides Edit
//! for it, and these tools say so rather than add a rule of their own.

use serde_json::{Value, json};

use super::super::args::{self, Field, Kind, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A `Session`'s fields, as the Kiosks page and its edit form show them. Never the
/// public key; the fingerprint is the short hash the kiosk itself displays.
const KIOSK_SELECTION: &str = "id name locationId lastContact clientVersion version \
     keyEnrolled keyFingerprint keyExpiresAt keyReleasedAt reactivatable \
     healthcheckUrl config createdAt updatedAt \
     clientInfo { env origin apiUrl profile userAgent screen displayMode timezone \
     clockSkewSecs uptimeSecs pendingVersion contactFailures updatedAt }";

/// Where to send the user for what these tools cannot do, repeated in descriptions.
const ENROL_ELSEWHERE: &str = "To add a kiosk, or bring back one that has expired, use the \
    admin site's Kiosks page: a device is only set up by a person scanning its QR code in \
    the browser, which these tools cannot do.";

pub(super) fn catalogue() -> Vec<Value> {
    let kiosk_id = || id_property("The kiosk's id, from `list_kiosks`.");
    vec![
        tool(
            "list_kiosks",
            "List kiosks",
            &format!(
                "List a location's kiosks (devices that sign members in and out), most \
                 recently in contact first: name, `lastContact`, client version, device-key \
                 `keyFingerprint` and `keyExpiresAt` (a key-enrolled kiosk stops working when \
                 it passes, after two weeks offline; null for an old setup-code kiosk), \
                 `reactivatable`, `healthcheckUrl`, `config` and what the kiosk last reported \
                 about itself (`clientInfo`). `keyReleasedAt` set means the device was set up \
                 again as a different kiosk, so this entry no longer works and is only a \
                 leftover to delete. Needs an Admin, Read only or super-user grant for the \
                 location; get ids from `list_locations`. {ENROL_ELSEWHERE}"
            ),
            input_schema(
                json!({ "locationId": id_property("The location's id, from `list_locations`.") }),
                &["locationId"],
            ),
            Effect::Read,
        ),
        tool(
            "get_kiosk",
            "Get kiosk",
            "One kiosk's details, in the same shape as `list_kiosks` (including `config`, \
             `healthcheckUrl` and `clientInfo`). Needs a grant for the kiosk's location.",
            input_schema(json!({ "id": kiosk_id() }), &["id"]),
            Effect::Read,
        ),
        tool(
            "update_kiosk",
            "Update kiosk",
            &format!(
                "Change a kiosk's name, `config` or `healthcheckUrl`. Only the fields you pass \
                 change; anything omitted keeps its current value. `config` is a JSON object \
                 (the kiosk's settings, e.g. `status`, `theme`, `guests`, `quickPickCategories`, \
                 `interfaceMode`) that replaces the whole current one, so start from `get_kiosk`'s \
                 `config`; `{{}}` resets it. `healthcheckUrl` must be an absolute http or https \
                 URL of at most 255 characters: passing `null` (or an empty string) clears it, \
                 leaving it out keeps it. A kiosk that is online picks the change up straight \
                 away; one that is not gets it when it next checks in. The site does not offer \
                 editing a leftover with `keyReleasedAt` set, since it can never work again. \
                 Needs an Admin (or super-user) grant for the kiosk's location; Read only users \
                 are refused. {ENROL_ELSEWHERE}"
            ),
            input_schema(
                json!({
                    "id": kiosk_id(),
                    "name": { "type": "string", "description": "New name." },
                    "config": {
                        "type": "object",
                        "description": "New kiosk settings, replacing the current config entirely.",
                    },
                    "healthcheckUrl": {
                        "type": ["string", "null"],
                        "description": "New health check URL; null clears it.",
                    },
                }),
                &["id"],
            ),
            Effect::Update,
        ),
        tool(
            "delete_kiosk",
            "Delete kiosk",
            "Delete (soft-delete) a kiosk. The computer using it can no longer sign anyone in \
             or out until it is set up again by scanning its QR code on the admin site's Kiosks \
             page; this cannot be undone from here. Also how to remove a leftover with \
             `keyReleasedAt` set. Needs an Admin (or super-user) grant for the kiosk's \
             location; Read only users are refused.",
            input_schema(json!({ "id": kiosk_id() }), &["id"]),
            Effect::Destructive,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_kiosks" => list_kiosks(ctx, a).await,
        "get_kiosk" => get_kiosk(ctx, a).await,
        "update_kiosk" => update_kiosk(ctx, a).await,
        "delete_kiosk" => delete_kiosk(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

async fn list_kiosks<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    ctx.require_location_read(location_id)?;
    let doc = format!(
        "query McpKiosks($id: ID!) {{ location(id: $id) {{ sessions {{ {KIOSK_SELECTION} }} }} }}"
    );
    let data = ctx.graphql(&doc, json!({ "id": location_id })).await?;
    let mut kiosks = data["location"]["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    // Most recent contact first, like the admin page; a kiosk never heard from goes last.
    kiosks.sort_by_key(|k| std::cmp::Reverse(k["lastContact"].as_i64()));
    Ok(json!({ "kiosks": kiosks }))
}

/// Fetch a kiosk and check the caller may see it.
async fn fetch_kiosk<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!("query McpKiosk($id: ID!) {{ session(id: $id) {{ {KIOSK_SELECTION} }} }}");
    let data = ctx.graphql(&doc, json!({ "id": id })).await?;
    let kiosk = data["session"].clone();
    ctx.require_location_read(kiosk["locationId"].as_str().unwrap_or_default())?;
    Ok(kiosk)
}

async fn get_kiosk<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let kiosk = fetch_kiosk(ctx, args::str_arg(a, "id")?).await?;
    Ok(json!({ "kiosk": kiosk }))
}

async fn update_kiosk<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    if args::has_arg(a, "config") && !a["config"].is_object() {
        return Err(vec![
            "Invalid \"config\" argument: must be a JSON object (use {} to reset it)".into(),
        ]);
    }
    // The mutation takes every field and clears the ones it is not given, so overlay the
    // arguments on the current record. The read is checked here; `updateSession` itself
    // requires an Admin grant.
    let current = fetch_kiosk(ctx, id).await?;
    let mut vars = args::merge_update(
        &current,
        a,
        &[
            Field::new("name", Kind::Str),
            Field::new("config", Kind::Json),
            Field::new("healthcheckUrl", Kind::Str).nullable(),
        ],
    )?;
    // The mutation takes `config` as a JSON string, though it returns it as an object.
    if let Some(config) = vars.get_mut("config").filter(|c| !c.is_null()) {
        *config = Value::String(config.to_string());
    }
    vars.insert("id".into(), json!(id));
    let doc = format!(
        "mutation McpUpdateKiosk($id: ID!, $name: String!, $config: String, \
         $healthcheckUrl: String) {{ updateSession(id: $id, name: $name, config: $config, \
         healthcheckUrl: $healthcheckUrl) {{ {KIOSK_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, Value::Object(vars)).await,
        "updateSession",
        "kiosk",
    )
}

async fn delete_kiosk<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpDeleteKiosk($id: ID!) { deleteSession(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({ "deleted": true, "id": id }))
}
