//! The allowlist for a Tally sign-in grant ([`crate::oauth::TALLY_SIGN_IN_SCOPE`]).
//!
//! A member token minted from a Tally assertion is for attendance self sign-in and
//! nothing else, whatever the member could otherwise do. Rather than touch every
//! resolver, [`TallySignInScope`] is an extension that checks the whole parsed
//! document before anything runs and refuses it unless every operation stays inside
//! [`check_document`]'s allowlist. Deny by default: a field not named here is refused,
//! so new schema fields are unreachable from this scope until someone adds them.
//!
//! The allowlist is the whole tree of fields the Tally app asks for, not just the root
//! fields: `selfSignInStatus` returns a full `Person` and `Location`, and a `Period`
//! links on to sessions and NITC groups, so allowing a root would otherwise trust every
//! resolver beneath it to refuse a member.
//!
//! The check is on the document alone (no operation name is available at the hook), so
//! it is strict: *every* operation in it must pass, even ones that would not be run.
//! Fragments anywhere are refused outright rather than followed, which keeps the walk
//! trivially sound. Aliases are ignored; the real field name is checked.

use async_graphql::extensions::{Extension, ExtensionContext, ExtensionFactory, NextParseQuery};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{ServerError, ServerResult, Variables};
use std::sync::Arc;

use super::error::ErrorCode;
use crate::auth::AuthInfo;

const REFUSAL: &str = "This sign-in can only be used to sign in and out";

/// The fields a Tally sign-in may select at one level, each with the fields it may
/// select beneath it (empty for a scalar). `__typename` is allowed at every level.
type Fields = &'static [F];
/// A field and the fields it may select beneath it.
struct F(&'static str, Fields);

const SCALAR: Fields = &[];
const ID_NAME: Fields = &[F("id", SCALAR), F("name", SCALAR)];
const PERIOD: Fields = &[
    F("id", SCALAR),
    F("version", SCALAR),
    F("startTime", SCALAR),
    F("endTime", SCALAR),
];
const CATEGORY: Fields = &[
    F("id", SCALAR),
    F("name", SCALAR),
    F("enabled", SCALAR),
    F("groupName", SCALAR),
    F("shortName", SCALAR),
    F("icon", SCALAR),
];
const RECENT_CATEGORY: Fields = &[F("category", CATEGORY)];

/// Query root fields a Tally sign-in may read: what the app's Sign in tab asks for.
const QUERY_FIELDS: Fields = &[
    F(
        "selfSignInStatus",
        &[
            F(
                "person",
                &[
                    F("id", SCALAR),
                    F("firstName", SCALAR),
                    F("lastName", SCALAR),
                ],
            ),
            F("location", ID_NAME),
            F("openPeriod", PERIOD),
            F(
                "whosHere",
                &[
                    F("id", SCALAR),
                    F("name", SCALAR),
                    F("guest", SCALAR),
                    F("startTime", SCALAR),
                    F("isMe", SCALAR),
                ],
            ),
            F(
                "quickPick",
                &[
                    F("personCategories", RECENT_CATEGORY),
                    F("locationCategories", RECENT_CATEGORY),
                ],
            ),
        ],
    ),
    F("categories", CATEGORY),
    // Only what the unit picker needs: a location's other fields are its tasks, notes,
    // photos and the rest of the unit's Tally data.
    F(
        "tallyViewer",
        &[F("locations", ID_NAME), F("primaryLocationId", SCALAR)],
    ),
];
/// Mutation root fields a Tally sign-in may call.
const MUTATION_FIELDS: Fields = &[F("selfSignIn", PERIOD), F("selfSignOut", PERIOD)];

fn refuse(pos: async_graphql::Pos) -> ServerError {
    let mut err = ServerError::new(REFUSAL, Some(pos));
    err.extensions
        .get_or_insert_with(Default::default)
        .set("code", ErrorCode::Forbidden.as_str());
    err
}

/// Check every operation in `doc` against the Tally sign-in allowlist.
pub fn check_document(doc: &ExecutableDocument) -> ServerResult<()> {
    for (_, op) in doc.operations.iter() {
        let allowed = match op.node.ty {
            OperationType::Query => QUERY_FIELDS,
            OperationType::Mutation => MUTATION_FIELDS,
            OperationType::Subscription => return Err(refuse(op.pos)),
        };
        check_selection_set(&op.node.selection_set.node, allowed)?;
    }
    Ok(())
}

/// Check a selection set and everything beneath it: only fields named in `allowed`
/// (or `__typename`, which selects nothing), each checked against its own subfields,
/// and no fragments.
fn check_selection_set(set: &SelectionSet, allowed: Fields) -> ServerResult<()> {
    for item in &set.items {
        let Selection::Field(field) = &item.node else {
            return Err(refuse(item.pos));
        };
        let name = field.node.name.node.as_str();
        let sub = if name == "__typename" {
            SCALAR
        } else {
            match allowed.iter().find(|f| f.0 == name) {
                Some(f) => f.1,
                None => return Err(refuse(field.pos)),
            }
        };
        check_selection_set(&field.node.selection_set.node, sub)?;
    }
    Ok(())
}

/// Whether `auth` is a caller the allowlist applies to.
fn applies_to(auth: Option<&AuthInfo>) -> bool {
    auth.is_some_and(AuthInfo::is_tally_sign_in_only)
}

/// Extension enforcing the allowlist for Tally sign-in grants, registered in
/// [`super::build_schema`] so the local server and the Lambda both get it.
pub struct TallySignInScope;

impl ExtensionFactory for TallySignInScope {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(TallySignInScope)
    }
}

#[async_graphql::async_trait::async_trait]
impl Extension for TallySignInScope {
    async fn parse_query(
        &self,
        ctx: &ExtensionContext<'_>,
        query: &str,
        variables: &Variables,
        next: NextParseQuery<'_>,
    ) -> ServerResult<ExecutableDocument> {
        let doc = next.run(ctx, query, variables).await?;
        if applies_to(ctx.data_opt::<AuthInfo>()) {
            check_document(&doc)?;
        }
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::parser::parse_query;

    fn check(query: &str) -> ServerResult<()> {
        check_document(&parse_query(query).unwrap())
    }

    fn assert_allowed(query: &str) {
        check(query).unwrap_or_else(|e| panic!("refused {query}: {e:?}"));
    }

    fn assert_refused(query: &str) {
        let err = check(query).expect_err(query);
        assert_eq!(err.message, REFUSAL);
        assert_eq!(
            err.extensions.unwrap().get("code"),
            Some(&async_graphql::Value::from("FORBIDDEN"))
        );
    }

    #[test]
    fn the_queries_tally_sends_are_allowed() {
        for q in [
            "query TallySignInStatus($locationId: ID) { selfSignInStatus(locationId: $locationId) { person { id firstName lastName } location { id name } openPeriod { id version startTime endTime } whosHere { id name guest startTime isMe } } }",
            "query TallyQuickPick($locationId: ID) { selfSignInStatus(locationId: $locationId) { quickPick { personCategories { category { id name enabled groupName shortName icon } } locationCategories { category { id name enabled groupName shortName icon } } } } }",
            "query TallyCategories { categories { id name enabled groupName shortName icon } }",
            "mutation TallySignIn($locationId: ID) { selfSignIn(locationId: $locationId) { id version startTime endTime } }",
            "mutation TallySignOut($id: ID!, $expectedVersion: Int!, $startTime: Int!, $endTime: Int!, $categoryId: ID!) { selfSignOut(id: $id, expectedVersion: $expectedVersion, startTime: $startTime, endTime: $endTime, categoryId: $categoryId) { id version startTime endTime } }",
            "query TallyUnits { tallyViewer { locations { id name } primaryLocationId } }",
            "{ __typename }",
            "{ a: selfSignInStatus { person { id } } }",
        ] {
            assert_allowed(q);
        }
    }

    #[test]
    fn other_root_fields_are_refused() {
        assert_refused("{ tasks { id } }");
        assert_refused("{ categories { id } tasks { id } }");
        assert_refused("mutation { deletePeriod(id: \"x\") }");
        assert_refused("mutation { selfSignIn { id } deletePeriod(id: \"x\") }");
        // A query field is not a mutation field and vice versa.
        assert_refused("mutation { categories { id } }");
        assert_refused("{ selfSignIn { id } }");
    }

    #[test]
    fn an_alias_does_not_hide_the_real_field() {
        assert_refused("{ categories: tasks { id } }");
        assert_allowed("{ tasks: categories { id } }");
    }

    #[test]
    fn tally_viewer_is_limited_to_the_unit_picker_fields() {
        assert_refused("{ tallyViewer { passkeys { id } } }");
        assert_refused("{ tallyViewer { member { id } } }");
        assert_refused("{ tallyViewer { signIns { id } } }");
        assert_refused("{ tallyViewer { locations { id } member { id } } }");
        assert_refused("{ tallyViewer { ...V } } fragment V on TallyViewer { passkeys { id } }");
        assert_refused("{ tallyViewer { ... on TallyViewer { passkeys { id } } } }");
    }

    #[test]
    fn fields_below_the_root_are_limited_too() {
        assert_refused("{ tallyViewer { locations { id notes { id } } } }");
        assert_refused("{ tallyViewer { locations { sharedContacts { id } } } }");
        assert_refused("{ selfSignInStatus { person { email } } }");
        assert_refused("{ selfSignInStatus { location { id people { id } } } }");
        assert_refused("{ selfSignInStatus { openPeriod { signedInSession { id } } } }");
        assert_refused("{ categories { nitcGroup { id } } }");
        assert_refused("mutation { selfSignIn { id person { id } } }");
        assert_refused("{ selfSignInStatus { person { ... on Person { email } } } }");
        assert_allowed("{ selfSignInStatus { person { __typename id } } }");
    }

    #[test]
    fn fragments_at_a_checked_level_are_refused() {
        assert_refused("{ ...Q } fragment Q on QueryRoot { tasks { id } }");
        assert_refused("{ ...Q } fragment Q on QueryRoot { categories { id } }");
        assert_refused("{ ... on QueryRoot { categories { id } } }");
        assert_refused("mutation { ... on MutationRoot { selfSignIn { id } } }");
    }

    #[test]
    fn introspection_is_refused() {
        assert_refused("{ __schema { types { name } } }");
        assert_refused("{ __type(name: \"QueryRoot\") { name } }");
    }

    #[test]
    fn every_operation_in_the_document_is_checked() {
        assert_refused("query A { categories { id } } query B { tasks { id } }");
        assert_refused("query A { tasks { id } } query B { categories { id } }");
        assert_allowed(
            "query A { categories { id } } query B { selfSignInStatus { person { id } } }",
        );
    }

    #[test]
    fn subscriptions_are_refused() {
        assert_refused("subscription { anything }");
    }

    fn member(scope: &str) -> AuthInfo {
        AuthInfo::Member {
            person_id: "p".into(),
            location_id: "l".into(),
            member_location_ids: vec![],
            user_id: None,
            is_super: false,
            user_location_ids: vec![],
            grant_id: "g".into(),
            scope: scope.into(),
        }
    }

    #[test]
    fn only_a_tally_sign_in_member_is_restricted() {
        use crate::oauth::{DEFAULT_SCOPE, TALLY_SIGN_IN_SCOPE};
        assert!(applies_to(Some(&member(TALLY_SIGN_IN_SCOPE))));
        assert!(!applies_to(Some(&member(DEFAULT_SCOPE))));
        assert!(!applies_to(Some(&AuthInfo::PeriodLink {
            period_id: "p".into()
        })));
        assert!(!applies_to(None));
    }
}
