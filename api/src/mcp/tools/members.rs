//! Member tools: `list_members`, `get_member`, `find_member_by_number`,
//! `create_member`, `update_member`, `delete_member`, `sync_members`.
//!
//! Reads of a location's members and of a single person only require a signed-in
//! caller in GraphQL, so these tools check the caller's grants themselves
//! ([`ToolCtx::require_location_read`]). Writes rely on the resolvers, which require
//! an Admin grant (a Read only user is refused).

use serde_json::{Value, json};

use super::super::args::{self, Field, Kind, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A `Person`'s fields, as the Members page shows them plus ids and times.
const MEMBER_SELECTION: &str = "id firstName lastName memberNumber sesApiPersonId missingSince \
     deleted locationId createdAt updatedAt";

/// What member sync owns, repeated in every tool description that can change a member.
const SYNC_OWNED: &str = "Members with `sesApiPersonId` set (`syncManaged: true`) are owned by the \
    member sync from SES headquarters: the admin site offers no edit or delete for them, and the \
    next sync may overwrite or undo a change. The API does not block it, so leave them alone \
    unless asked.";

pub(super) fn catalogue() -> Vec<Value> {
    let member_id = || id_property("The member's record id, from `list_members`.");
    vec![
        tool(
            "list_members",
            "List members",
            &format!(
                "List the active (not deleted) members of a location: id, name, member \
                 number, `sesApiPersonId`, `missingSince` (set when member sync last saw them \
                 missing from SES) and `syncManaged`. `query` keeps only members whose first \
                 name, last name, full name or member number contains it (case-insensitive). \
                 Needs an Admin, Read only or super-user grant for the location; get ids from \
                 `list_locations`. {SYNC_OWNED}"
            ),
            input_schema(
                json!({
                    "locationId": id_property("The location's id, from `list_locations`."),
                    "query": { "type": "string", "description": "Optional name or member-number fragment to filter by." },
                }),
                &["locationId"],
            ),
            Effect::Read,
        ),
        tool(
            "get_member",
            "Get member",
            "One member's details, including `lastPeriod` (their most recent activity \
             period, or null). Needs a grant for the member's location. To find a member \
             from a member number use `find_member_by_number`.",
            input_schema(json!({ "id": member_id() }), &["id"]),
            Effect::Read,
        ),
        tool(
            "find_member_by_number",
            "Find member by number",
            "Look a member up by their member (registration) number, returning the same \
             details as `get_member`. Errors when no member has that number or you have no \
             grant for their location.",
            input_schema(
                json!({ "memberNumber": { "type": "string", "description": "The member number to look up." } }),
                &["memberNumber"],
            ),
            Effect::Read,
        ),
        tool(
            "create_member",
            "Create member",
            "Add a member to a location by hand. The member number must not already belong \
             to an active member. Needs an Admin (or super-user) grant for the location; \
             Read only users are refused.",
            input_schema(
                json!({
                    "locationId": id_property("The location's id, from `list_locations`."),
                    "firstName": { "type": "string" },
                    "lastName": { "type": "string" },
                    "memberNumber": { "type": "string" },
                }),
                &["locationId", "firstName", "lastName", "memberNumber"],
            ),
            Effect::Create,
        ),
        tool(
            "update_member",
            "Update member",
            &format!(
                "Change a member's name or member number. Only the fields you pass change; \
                 anything omitted keeps its current value. Needs an Admin (or super-user) \
                 grant for the member's location. {SYNC_OWNED}"
            ),
            input_schema(
                json!({
                    "id": member_id(),
                    "firstName": { "type": "string" },
                    "lastName": { "type": "string" },
                    "memberNumber": { "type": "string" },
                }),
                &["id"],
            ),
            Effect::Update,
        ),
        tool(
            "delete_member",
            "Delete member",
            &format!(
                "Delete (soft-delete) a member, hiding them from the location. Needs an \
                 Admin (or super-user) grant for the member's location. {SYNC_OWNED}"
            ),
            input_schema(json!({ "id": member_id() }), &["id"]),
            Effect::Destructive,
        ),
        tool(
            "sync_members",
            "Sync members",
            "Queue a member sync for a location, which pulls its roster from SES \
             headquarters in the background (it does not wait for the result; check \
             `lastSuccessfulMemberSync` on `get_location` afterwards). Needs an Admin (or \
             super-user) grant for the location.",
            input_schema(
                json!({ "locationId": id_property("The location's id, from `list_locations`.") }),
                &["locationId"],
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
        "list_members" => list_members(ctx, a).await,
        "get_member" => get_member(ctx, a).await,
        "find_member_by_number" => find_member_by_number(ctx, a).await,
        "create_member" => create_member(ctx, a).await,
        "update_member" => update_member(ctx, a).await,
        "delete_member" => delete_member(ctx, a).await,
        "sync_members" => sync_members(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

/// Add `syncManaged`: whether member sync owns this person (the admin site offers no
/// edit or delete for them).
fn decorate(member: &mut Value) {
    let managed = member["sesApiPersonId"].is_string();
    if let Some(obj) = member.as_object_mut() {
        obj.insert("syncManaged".into(), json!(managed));
    }
}

fn contains_ci(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_lowercase().contains(needle_lower)
}

async fn list_members<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    let query = args::opt_str_arg(a, "query")?
        .map(|q| q.trim().to_lowercase())
        .filter(|q| !q.is_empty());
    ctx.require_location_read(location_id)?;

    // `Location.people` returns only members that aren't deleted.
    let doc = format!(
        "query McpMembers($id: ID!) {{ location(id: $id) {{ people {{ {MEMBER_SELECTION} }} }} }}"
    );
    let data = ctx.graphql(&doc, json!({ "id": location_id })).await?;
    let mut members: Vec<Value> = data["location"]["people"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(q) = &query {
        members.retain(|m| {
            let first = m["firstName"].as_str().unwrap_or_default();
            let last = m["lastName"].as_str().unwrap_or_default();
            contains_ci(first, q)
                || contains_ci(last, q)
                || contains_ci(&format!("{first} {last}"), q)
                || contains_ci(m["memberNumber"].as_str().unwrap_or_default(), q)
        });
    }
    members.sort_by_key(|m| {
        (
            m["lastName"].as_str().unwrap_or_default().to_lowercase(),
            m["firstName"].as_str().unwrap_or_default().to_lowercase(),
        )
    });
    members.iter_mut().for_each(decorate);
    Ok(json!({ "members": members }))
}

/// Fetch a member with their last period and check the caller may see them.
async fn fetch_member<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!(
        "query McpMember($id: ID!) {{ person(id: $id) {{ {MEMBER_SELECTION} \
         lastPeriod {{ id startTime endTime categoryId category {{ id name }} comment }} }} }}"
    );
    let data = ctx.graphql(&doc, json!({ "id": id })).await?;
    let mut member = data["person"].clone();
    ctx.require_location_read(member["locationId"].as_str().unwrap_or_default())?;
    decorate(&mut member);
    Ok(member)
}

/// A member's location id, without the other details, for access checks.
async fn member_location<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<String, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let data = ctx
        .graphql(
            "query McpMemberLocation($id: ID!) { person(id: $id) { locationId } }",
            json!({ "id": id }),
        )
        .await?;
    Ok(data["person"]["locationId"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

async fn get_member<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let member = fetch_member(ctx, args::str_arg(a, "id")?).await?;
    Ok(json!({ "member": member }))
}

async fn find_member_by_number<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let number = args::str_arg(a, "memberNumber")?;
    // The same message whether there is no such member or the caller can't see them,
    // so a number can't be probed for existence at locations the caller has no grant for.
    let not_found = || vec![format!("No member found with member number \"{number}\"")];

    let data = ctx
        .graphql(
            "query McpFindMember($n: String!) { personByRegistrationNumber(memberNumber: $n) { id } }",
            json!({ "n": number }),
        )
        .await?;
    let id = data["personByRegistrationNumber"]["id"]
        .as_str()
        .ok_or_else(not_found)?
        .to_string();
    let location_id = member_location(ctx, &id).await?;
    ctx.require_location_read(&location_id)
        .map_err(|_| not_found())?;
    let member = fetch_member(ctx, &id).await?;
    Ok(json!({ "member": member }))
}

async fn create_member<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!(
        "mutation McpCreateMember($locationId: ID!, $firstName: String!, $lastName: String!, \
         $memberNumber: String!) {{ createPerson(locationId: $locationId, firstName: $firstName, \
         lastName: $lastName, memberNumber: $memberNumber) {{ {MEMBER_SELECTION} }} }}"
    );
    let vars = json!({
        "locationId": args::str_arg(a, "locationId")?,
        "firstName": args::str_arg(a, "firstName")?,
        "lastName": args::str_arg(a, "lastName")?,
        "memberNumber": args::str_arg(a, "memberNumber")?,
    });
    let mut result = rename_root(ctx.graphql(&doc, vars).await, "createPerson", "member")?;
    decorate(&mut result["member"]);
    Ok(result)
}

async fn update_member<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    // The mutation takes every field, so overlay the arguments on the current record.
    // The read is checked here; `updatePerson` itself requires an Admin grant.
    let current = fetch_member(ctx, id).await?;
    let mut vars = args::merge_update(
        &current,
        a,
        &[
            Field::new("firstName", Kind::Str),
            Field::new("lastName", Kind::Str),
            Field::new("memberNumber", Kind::Str),
        ],
    )?;
    vars.insert("id".into(), json!(id));
    let doc = format!(
        "mutation McpUpdateMember($id: ID!, $firstName: String!, $lastName: String!, \
         $memberNumber: String!) {{ updatePerson(id: $id, firstName: $firstName, \
         lastName: $lastName, memberNumber: $memberNumber) {{ {MEMBER_SELECTION} }} }}"
    );
    let mut result = rename_root(
        ctx.graphql(&doc, Value::Object(vars)).await,
        "updatePerson",
        "member",
    )?;
    decorate(&mut result["member"]);
    Ok(result)
}

async fn delete_member<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpDeleteMember($id: ID!) { deletePerson(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({ "deleted": true, "id": id }))
}

async fn sync_members<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let location_id = args::str_arg(a, "locationId")?;
    ctx.graphql(
        "mutation McpSyncMembers($id: ID!) { enqueueMemberSync(locationId: $id) }",
        json!({ "id": location_id }),
    )
    .await?;
    Ok(json!({ "queued": true, "locationId": location_id }))
}
