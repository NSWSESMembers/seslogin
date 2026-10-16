//! Audit log and the caller's own account: `get_audit_log`, `update_my_settings`,
//! `list_connected_apps`, `revoke_connected_app`, `list_passkeys`, `rename_passkey`,
//! `delete_passkey` and `submit_feedback`. They mirror the admin site's Audit log page,
//! Settings pages (daily email, activity display, connected apps, passkeys) and Help
//! form.
//!
//! **Audit log.** `Location.auditLog` is open to a user with Admin or Read only access
//! to that location (and super users) and refuses kiosks, API tokens and edit links;
//! `Query.auditLog` (every location plus global records) is `SuperUser`. The tool picks
//! between them on `locationId`, checks the caller's grant itself before the location
//! read (`Query.location` does not), and refuses a non-super caller who gives no
//! location with a clear message rather than the resolver's guard text. `ip` and
//! `actor.via` come back only to super users, as the resolvers decide.
//!
//! **Settings.** `updateMyEmailConfig` checks each location against the caller's
//! grants (it would otherwise be a push channel for other units' data) and
//! `updateMyDisaggregateVirtualPeriods` is a plain flag; both act on the caller only.
//!
//! **Connected apps and passkeys** are user-scoped. `user(id)` is self-or-super, and
//! `User.oauthGrants` repeats that check; `User.passkeys` relies on `user(id)` alone,
//! so a `userId` is only ever passed through to it. `revokeOauthGrant` lets a user
//! revoke their own grants (a super user anyone's) and answers a stranger's grant id
//! "not found"; `renamePasskey` and `deletePasskey` act only on the caller's own
//! passkeys, even for a super user. No passkey material is selected: the documents
//! name `id name createdAt lastUsedAt` and nothing else, and registration and login
//! ceremonies are not tools. Revoking the grant the calling AI client itself holds
//! ends its access, and the tool says so (`endedThisConnection`).
//!
//! **Feedback.** `submitFeedback` files a Toolbox ticket with the caller's own email
//! as sender. It fails when Toolbox is not configured, so the tool checks
//! `feedbackAvailable` first and points at support@seslogin.com, as the site's
//! email-only help dialog does.

use serde_json::{Value, json};

use super::super::args::{self, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// The `AuditEntityType` enum's values, as GraphQL names them.
const ENTITY_TYPES: &[&str] = &[
    "USER",
    "PERSON",
    "PERIOD",
    "SESSION",
    "API_TOKEN",
    "LOCATION",
    "CATEGORY",
    "NITC_GROUP",
    "NITC_TAG",
    "USER_TOKEN",
    "OAUTH_GRANT",
    "WEBAUTHN_CREDENTIAL",
];

/// An audit page's default and maximum size (`graphql/audit_log.rs`).
const DEFAULT_AUDIT_PAGE: i64 = 50;
const MAX_AUDIT_PAGE: i64 = 200;

/// An `AuditEntry`, as the Audit log page shows it. `ip` and `actor.via` are null
/// unless the caller is a super user.
const ENTRY_SELECTION: &str = "id timestamp action entityType entityId entityLabel \
     location { id name } actor { kind id via label } ip changes { field before after }";

/// A `PasskeyInfo`: metadata only, never credential material.
const PASSKEY_SELECTION: &str = "id name createdAt lastUsedAt";

/// Said of every `userId` argument.
const OTHER_USER_NOTE: &str = "Omit for yourself; only a super user may give someone else's id.";

/// Where to send someone when feedback cannot be filed from here.
const SUPPORT_EMAIL: &str = "support@seslogin.com";

pub(super) fn catalogue() -> Vec<Value> {
    let user_id = |what: &str| {
        json!({
            "type": "string",
            "description": format!("{what} {OTHER_USER_NOTE}"),
        })
    };
    vec![
        tool(
            "get_audit_log",
            "Get audit log",
            &format!(
                "Read the record of changes (who changed what, and when), newest first. With \
                 `locationId` it is that location's log: members, periods, kiosks and so on \
                 changed there; any user with an Admin or Read only grant for it can read it \
                 (get ids from `list_locations`). Without `locationId` it is every location's \
                 log plus global records (users, categories, API tokens, locations, NITC), \
                 each write once; super user only. `entityTypes` limits it to any of: {}. Each \
                 entry has `timestamp`, `action` (CREATE, UPDATE, DELETE, RESTORE), `entityType`, \
                 `entityId`, `entityLabel` (the name at the time), the `location`, the `actor` \
                 (`kind`, `id`, `label`) and the field-level `changes` (`before`/`after`; \
                 secrets show as `[redacted]`). `ip` and `actor.via` are shown to super users \
                 only. Returns `entries` and `pageInfo` (pass `endCursor` as `after` for the \
                 next, older page).",
                ENTITY_TYPES.join(", ")
            ),
            input_schema(
                json!({
                    "locationId": id_property(
                        "The location's id, from `list_locations`. Required unless you are a super user."
                    ),
                    "entityTypes": {
                        "type": "array",
                        "items": { "type": "string", "enum": ENTITY_TYPES },
                        "description": "Only entries about any of these kinds of record. Omitted or empty means every kind.",
                    },
                    "first": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_AUDIT_PAGE,
                        "description": format!("Page size (default {DEFAULT_AUDIT_PAGE}, at most {MAX_AUDIT_PAGE})."),
                    },
                    "after": {
                        "type": "string",
                        "description": "Cursor: the previous page's `pageInfo.endCursor`, to get the next (older) page.",
                    },
                }),
                &[],
            ),
            Effect::Read,
        ),
        tool(
            "update_my_settings",
            "Update my settings",
            "Change your own settings; only the ones you pass change, and at least one is \
             required. `dailyEmailLocationIds` is the complete list of locations whose daily \
             activity summary email you receive (replacing the current list; empty turns \
             them all off); you must have access to each (see `list_locations`). \
             `disaggregateVirtualPeriods` is the Settings page's activity-display option: \
             whether virtual activity is shown separately from in-person. Both are read back \
             in `whoami` as `emailSummaryLocationIds` and `disaggregateVirtualPeriods`. \
             Returns the new values as `settings`.",
            input_schema(
                json!({
                    "dailyEmailLocationIds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Location ids to get the daily summary email for; replaces the current list.",
                    },
                    "disaggregateVirtualPeriods": {
                        "type": "boolean",
                        "description": "Show virtual periods separately in activity displays.",
                    },
                }),
                &[],
            ),
            Effect::Update,
        ),
        tool(
            "list_connected_apps",
            "List connected apps",
            &format!(
                "List the apps (AI clients such as this one) you have authorised to act as \
                 you, newest first: id, `clientName`, `redirectHost` (where it sends you back \
                 to; trust this more than the self-declared name), `scope`, `createdAt`, \
                 `lastUsedAt` and `refreshExpiresAt` (when it stops working if never used \
                 again). The one this very connection uses has `thisConnection: true`. \
                 Revoke with `revoke_connected_app`. `userId`: {OTHER_USER_NOTE}"
            ),
            input_schema(
                json!({ "userId": user_id("Whose connected apps to list.") }),
                &[],
            ),
            Effect::Read,
        ),
        tool(
            "revoke_connected_app",
            "Revoke connected app",
            "Revoke a connected app (an authorised AI client) for good: it stops working \
             immediately and must be authorised again from scratch. WARNING: if the id is the \
             grant this connection itself is using (`thisConnection: true` in \
             `list_connected_apps`), this ends your own access and the next call will be \
             refused; the result says `endedThisConnection`. You can revoke your own apps; a \
             super user can revoke anyone's. Anyone else's id is reported as not found.",
            input_schema(
                json!({ "id": id_property("The grant's id, from `list_connected_apps`.") }),
                &["id"],
            ),
            Effect::Destructive,
        ),
        tool(
            "list_passkeys",
            "List passkeys",
            &format!(
                "List the passkeys registered on your account (id, `name`, `createdAt`, \
                 `lastUsedAt`); never any key material. Passkeys can only be added in the \
                 browser, on the admin site's Passkeys settings page. `userId`: {OTHER_USER_NOTE}"
            ),
            input_schema(json!({ "userId": user_id("Whose passkeys to list.") }), &[]),
            Effect::Read,
        ),
        tool(
            "rename_passkey",
            "Rename passkey",
            "Rename one of your own passkeys (the label shown in the list). Works on your \
             own passkeys only, even for a super user; anyone else's is reported as not found.",
            input_schema(
                json!({
                    "id": id_property("The passkey's id, from `list_passkeys`."),
                    "name": { "type": "string", "description": "The new label; must not be empty." },
                }),
                &["id", "name"],
            ),
            Effect::Update,
        ),
        tool(
            "delete_passkey",
            "Delete passkey",
            "Delete one of your own passkeys. The device it belonged to can no longer be used \
             to sign in that way, and a new passkey can only be added again in the browser. \
             Your own passkeys only, even for a super user; anyone else's is reported as not found.",
            input_schema(
                json!({ "id": id_property("The passkey's id, from `list_passkeys`.") }),
                &["id"],
            ),
            Effect::Destructive,
        ),
        tool(
            "submit_feedback",
            "Submit feedback",
            &format!(
                "Send feedback or a help request to the seslogin team, as a support ticket \
                 that emails them from your own address; replies come to you by email, with the \
                 returned `reference` in the subject. This sends a real message that cannot be \
                 unsent: only call it when the user has asked for it. Not available on every \
                 server; if it is off the tool says so, and the user can email {SUPPORT_EMAIL} \
                 instead. Subject: one line, at most 200 characters. Message: at most 10,000."
            ),
            input_schema(
                json!({
                    "subject": { "type": "string", "description": "One-line subject." },
                    "message": { "type": "string", "description": "What to tell the team." },
                }),
                &["subject", "message"],
            ),
            Effect::External,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "get_audit_log" => get_audit_log(ctx, a).await,
        "update_my_settings" => update_my_settings(ctx, a).await,
        "list_connected_apps" => list_connected_apps(ctx, a).await,
        "revoke_connected_app" => revoke_connected_app(ctx, a).await,
        "list_passkeys" => list_passkeys(ctx, a).await,
        "rename_passkey" => rename_passkey(ctx, a).await,
        "delete_passkey" => delete_passkey(ctx, a).await,
        "submit_feedback" => submit_feedback(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

// ── Audit log ────────────────────────────────────────────────────────────────

/// `entityTypes` as GraphQL enum names, case-insensitively, each checked here so a typo
/// gets the list of valid values rather than a coercion error.
fn entity_types_arg(a: &Value) -> Result<Vec<String>, ToolError> {
    args::opt_str_array_arg(a, "entityTypes")?
        .unwrap_or_default()
        .iter()
        .map(|t| {
            let upper = t.trim().to_uppercase();
            if ENTITY_TYPES.contains(&upper.as_str()) {
                Ok(upper)
            } else {
                Err(vec![format!(
                    "Invalid \"entityTypes\" argument: \"{t}\" is not one of {}",
                    ENTITY_TYPES.join(", ")
                )])
            }
        })
        .collect()
}

async fn get_audit_log<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::opt_str_arg(a, "locationId")?;
    let entity_types = entity_types_arg(a)?;
    let first = args::opt_i64_arg(a, "first")?.unwrap_or(DEFAULT_AUDIT_PAGE);
    if !(1..=MAX_AUDIT_PAGE).contains(&first) {
        return Err(vec![format!(
            "Invalid \"first\" argument: must be between 1 and {MAX_AUDIT_PAGE}"
        )]);
    }
    let after = args::opt_str_arg(a, "after")?;
    let page = format!(
        "auditLog(first: $first, after: $after, entityTypes: $entityTypes) {{ \
         edges {{ node {{ {ENTRY_SELECTION} }} }} pageInfo {{ hasNextPage endCursor }} }}"
    );
    let variables = json!({
        "first": first, "after": after, "entityTypes": entity_types,
    });

    let mut connection = match location_id {
        Some(id) => {
            ctx.require_location_read(id)?;
            let doc = format!(
                "query McpLocationAuditLog($id: ID!, $first: Int, $after: String, \
                 $entityTypes: [AuditEntityType!]) {{ location(id: $id) {{ {page} }} }}"
            );
            let mut variables = variables;
            variables["id"] = json!(id);
            ctx.graphql(&doc, variables).await?["location"]["auditLog"].take()
        }
        None => {
            if !ctx.is_super() {
                return Err(vec![
                    "\"locationId\" is required: only a super user can read the log across \
                     all locations"
                        .to_string(),
                ]);
            }
            let doc = format!(
                "query McpAuditLog($first: Int, $after: String, \
                 $entityTypes: [AuditEntityType!]) {{ {page} }}"
            );
            ctx.graphql(&doc, variables).await?["auditLog"].take()
        }
    };
    let entries: Vec<Value> = connection["edges"]
        .as_array_mut()
        .map(|edges| edges.iter_mut().map(|e| e["node"].take()).collect())
        .unwrap_or_default();
    Ok(json!({ "entries": entries, "pageInfo": connection["pageInfo"].take() }))
}

// ── Settings ─────────────────────────────────────────────────────────────────

const SETTINGS_SELECTION: &str = "emailSummaryLocationIds disaggregateVirtualPeriods";

async fn update_my_settings<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    // Read both arguments before changing anything, so a bad second one cannot leave
    // the first half-applied.
    let email_ids = args::opt_str_array_arg(a, "dailyEmailLocationIds")?;
    let disaggregate = args::opt_bool_arg(a, "disaggregateVirtualPeriods")?;
    if email_ids.is_none() && disaggregate.is_none() {
        return Err(vec![
            "Nothing to change: pass \"dailyEmailLocationIds\" and/or \
             \"disaggregateVirtualPeriods\""
                .to_string(),
        ]);
    }
    let mut data = None;
    if let Some(ids) = email_ids {
        let doc = format!(
            "mutation McpUpdateMyEmailConfig($ids: [String!]!) {{ \
             updateMyEmailConfig(dailyLocationIds: $ids) {{ {SETTINGS_SELECTION} }} }}"
        );
        data = Some(rename_root(
            ctx.graphql(&doc, json!({ "ids": ids })).await,
            "updateMyEmailConfig",
            "settings",
        )?);
    }
    if let Some(value) = disaggregate {
        let doc = format!(
            "mutation McpUpdateMyDisaggregate($value: Boolean!) {{ \
             updateMyDisaggregateVirtualPeriods(value: $value) {{ {SETTINGS_SELECTION} }} }}"
        );
        data = Some(rename_root(
            ctx.graphql(&doc, json!({ "value": value })).await,
            "updateMyDisaggregateVirtualPeriods",
            "settings",
        )?);
    }
    // The last mutation ran after any other, so its result is the current state.
    Ok(data.expect("one of the two settings was given"))
}

// ── Connected apps ───────────────────────────────────────────────────────────

/// `user(id) { <field> }` for the optional `userId` argument (null = the caller).
async fn user_field<A>(
    ctx: &ToolCtx<'_, A>,
    a: &Value,
    field_selection: &str,
) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let user_id = args::opt_str_arg(a, "userId")?;
    let doc = format!("query McpUserField($id: ID) {{ user(id: $id) {{ id {field_selection} }} }}");
    Ok(ctx.graphql(&doc, json!({ "id": user_id })).await?["user"].take())
}

async fn list_connected_apps<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let mut user = user_field(
        ctx,
        a,
        "oauthGrants { id clientName redirectHost scope createdAt lastUsedAt refreshExpiresAt }",
    )
    .await?;
    let mut grants = user["oauthGrants"].take();
    if let (Some(items), Some(current)) = (grants.as_array_mut(), ctx.grant_id()) {
        for grant in items {
            if grant["id"].as_str() == Some(current) {
                grant["thisConnection"] = json!(true);
            }
        }
    }
    Ok(json!({ "connectedApps": grants }))
}

async fn revoke_connected_app<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpRevokeConnectedApp($id: ID!) { revokeOauthGrant(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({
        "revoked": true,
        "id": id,
        "endedThisConnection": ctx.grant_id() == Some(id),
    }))
}

// ── Passkeys ─────────────────────────────────────────────────────────────────

async fn list_passkeys<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let mut user = user_field(ctx, a, &format!("passkeys {{ {PASSKEY_SELECTION} }}")).await?;
    Ok(json!({ "passkeys": user["passkeys"].take() }))
}

async fn rename_passkey<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    let name = args::str_arg(a, "name")?;
    let doc = format!(
        "mutation McpRenamePasskey($id: String!, $name: String!) {{ \
         renamePasskey(id: $id, name: $name) {{ {PASSKEY_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, json!({ "id": id, "name": name })).await,
        "renamePasskey",
        "passkey",
    )
}

async fn delete_passkey<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpDeletePasskey($id: String!) { deletePasskey(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({ "deleted": true, "id": id }))
}

// ── Feedback ─────────────────────────────────────────────────────────────────

async fn submit_feedback<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let subject = args::str_arg(a, "subject")?;
    let message = args::str_arg(a, "message")?;
    let available = ctx
        .graphql(
            "query McpFeedbackAvailable { feedbackAvailable }",
            json!({}),
        )
        .await?;
    if available["feedbackAvailable"] != json!(true) {
        return Err(vec![format!(
            "The help form isn't available on this server. Email {SUPPORT_EMAIL} instead."
        )]);
    }
    let result = ctx
        .graphql(
            "mutation McpSubmitFeedback($subject: String!, $message: String!) { \
             submitFeedback(subject: $subject, message: $message) { number reference } }",
            json!({ "subject": subject, "message": message }),
        )
        .await;
    rename_root(result, "submitFeedback", "ticket")
}
