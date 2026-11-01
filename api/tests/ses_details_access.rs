//! Coverage for the SES details exposed on `Person` (contact details, qualifications,
//! unit assignments) and `Location` (code, zone, coordinates, addresses, accreditations).
//!
//! These are personal and unit contact details, so they need access to the person's
//! *primary* location and are never served to a kiosk — unlike `Person`'s identity fields,
//! which a visiting member's kiosk must read for cross-unit sign-in.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

fn webauthn() -> Arc<webauthn_rs::prelude::Webauthn> {
    Arc::new(
        webauthn_rs::prelude::WebauthnBuilder::new(
            "localhost",
            &url::Url::parse("http://localhost:5173").unwrap(),
        )
        .unwrap()
        .rp_name("seslogin-test")
        .build()
        .unwrap(),
    )
}

fn user(admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

fn kiosk_at(location: &str) -> AuthInfo {
    AuthInfo::Session {
        id: "kiosk-a".to_string(),
        location: location.to_string(),
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_location(&app, "loc-z", "Zone Z");
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "10000002", Some("9"));
    {
        let mut persons = app.db.persons.lock().unwrap();
        let bob = persons.get_mut("bob").unwrap();
        bob.mobile = Some("+61400000123".into());
        bob.phones = Some(
            r#"[{"number":"+61400000123","types":["SMS"]},{"number":"+61255500142","types":["Home Telephone"]}]"#
                .into(),
        );
        bob.preferred_name = Some("Bobby".into());
        bob.capability_tags = Some(r#"[{"id":101,"name":"First Aid","description":"d"}]"#.into());
    }
    app.db.person_units.lock().unwrap().insert(
        "bob".to_string(),
        vec![
            db::PersonUnit {
                person_id: "bob".into(),
                entity_id: "2".into(),
                entity_name: Some("Zeta Cluster".into()),
                ..Default::default()
            },
            db::PersonUnit {
                person_id: "bob".into(),
                entity_id: "1".into(),
                entity_name: Some("Beta Unit".into()),
                ..Default::default()
            },
            db::PersonUnit {
                person_id: "bob".into(),
                entity_id: "3".into(),
                entity_name: Some("Alpha Unit".into()),
                location_id: Some("loc-a".into()),
                ranks: vec!["Team Leader".into()],
                is_primary: true,
                ..Default::default()
            },
        ],
    );
    {
        let mut locations = app.db.locations.lock().unwrap();
        let a = locations.get_mut("loc-a").unwrap();
        a.ses_details = db::LocationSesDetails {
            code: Some("TST-A".into()),
            latitude: Some(-33.5),
            longitude: Some(151.25),
            zone_location_id: Some("loc-z".into()),
        };
        a.directory_details = db::LocationDirectoryDetails {
            entity_id: Some("70001".into()),
            address: Some(db::LocationAddress {
                formatted: Some("1 Example Street, Exampleville NSW 2000".into()),
                ..Default::default()
            }),
            postal_address: None,
            email: Some("unit-a@example.com".into()),
            phone: Some("02 5550 0100".into()),
        };
        a.accreditations = Some(r#"[{"type":"Flood Rescue","status":"Accredited"}]"#.into());
    }
    Arc::new(app)
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn code_of(response: &async_graphql::Response) -> Option<String> {
    match response.errors.first()?.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn assert_forbidden(response: &async_graphql::Response) {
    assert_eq!(
        code_of(response).as_deref(),
        Some("FORBIDDEN"),
        "{:?}",
        response.errors
    );
}

const PERSON: &str = r#"{ person(id: "bob") {
    preferredName mobile
    phones { number types }
    capabilityTags { id name description }
    units { entityId entityName isPrimary ranks locationId }
} }"#;

const LOCATION: &str = r#"{ location(id: "loc-a") {
    code latitude longitude sesDirectoryEntityId
    zone { id name }
    address { formatted line1 } postalAddress { formatted }
    email phone
    accreditations { type status number note }
} }"#;

#[tokio::test]
async fn read_only_user_of_the_primary_location_reads_the_person_details() {
    let app = app();
    let response = run(&app, user(&[], &["loc-a"]), PERSON).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let p = &data["person"];
    assert_eq!(p["preferredName"], "Bobby");
    assert_eq!(p["mobile"], "+61400000123");
    assert_eq!(p["phones"][1]["number"], "+61255500142");
    assert_eq!(p["phones"][1]["types"][0], "Home Telephone");
    assert_eq!(p["capabilityTags"][0]["id"], "101");
    assert_eq!(p["capabilityTags"][0]["name"], "First Aid");
    // Primary first, then by name.
    let names: Vec<_> = p["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["entityName"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Alpha Unit", "Beta Unit", "Zeta Cluster"]);
    assert_eq!(p["units"][0]["isPrimary"], true);
    assert_eq!(p["units"][0]["ranks"][0], "Team Leader");
    assert_eq!(p["units"][0]["locationId"], "loc-a");
    assert!(p["units"][1]["locationId"].is_null());
}

#[tokio::test]
async fn unreadable_json_yields_empty_lists_not_an_error() {
    let app = app();
    {
        let mut persons = app.db.persons.lock().unwrap();
        let bob = persons.get_mut("bob").unwrap();
        bob.phones = Some("not json".into());
        bob.capability_tags = Some("{}".into());
    }
    let response = run(&app, user(&[], &["loc-a"]), PERSON).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["person"]["phones"].as_array().unwrap().len(), 0);
    assert_eq!(
        data["person"]["capabilityTags"].as_array().unwrap().len(),
        0
    );
}

#[tokio::test]
async fn user_without_access_to_the_primary_location_is_refused_the_person_details() {
    let app = app();
    seed_person(&app, "carol", "loc-b", "Carol", "Cole", "20000009", None);
    // Another unit's member, reached by id: `Query.person` refuses first, so this also
    // covers the primary-location check the fields repeat.
    let query = r#"{ person(id: "carol") { mobile } }"#;
    assert_forbidden(&run(&app, user(&["loc-a"], &[]), query).await);
    assert_forbidden(&run(&app, user(&[], &["loc-a"]), query).await);
}

/// A kiosk is stopped by the field guard itself, before any location check.
fn assert_not_for_kiosks(response: &async_graphql::Response) {
    assert_eq!(
        code_of(response).as_deref(),
        Some("UNAUTHENTICATED"),
        "{:?}",
        response.errors
    );
}

#[tokio::test]
async fn kiosk_is_refused_the_person_details_even_at_the_primary_location() {
    let app = app();
    for field in [
        "preferredName",
        "mobile",
        "phones { number }",
        "capabilityTags { id }",
        "units { entityId }",
    ] {
        let query = format!(r#"{{ person(id: "bob") {{ {field} }} }}"#);
        assert_not_for_kiosks(&run(&app, kiosk_at("loc-a"), &query).await);
    }
}

#[tokio::test]
async fn kiosk_can_still_read_a_members_name_through_a_scan() {
    let app = app();
    let response = run(
        &app,
        kiosk_at("loc-a"),
        r#"mutation { scanRegister2(memberNumber: "10000002", quickPick: false) {
            period { person { firstName lastName } } } }"#,
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

#[tokio::test]
async fn read_only_user_reads_the_location_details() {
    let app = app();
    let response = run(&app, user(&[], &["loc-a"]), LOCATION).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let l = &data["location"];
    assert_eq!(l["code"], "TST-A");
    assert_eq!(l["latitude"], -33.5);
    assert_eq!(l["longitude"], 151.25);
    assert_eq!(l["sesDirectoryEntityId"], "70001");
    // The zone's name, though the caller has no access to the zone itself.
    assert_eq!(l["zone"]["id"], "loc-z");
    assert_eq!(l["zone"]["name"], "Zone Z");
    assert_eq!(
        l["address"]["formatted"],
        "1 Example Street, Exampleville NSW 2000"
    );
    assert!(l["postalAddress"].is_null());
    assert_eq!(l["email"], "unit-a@example.com");
    assert_eq!(l["phone"], "02 5550 0100");
    assert_eq!(l["accreditations"][0]["type"], "Flood Rescue");
    assert_eq!(l["accreditations"][0]["status"], "Accredited");
    assert!(l["accreditations"][0]["number"].is_null());
}

#[tokio::test]
async fn location_details_are_refused_to_other_locations_and_kiosks() {
    let app = app();
    // Both by `Query.location` and by `Person.location`, which skips `Query`'s check.
    assert_forbidden(&run(&app, user(&["loc-b"], &[]), LOCATION).await);
    assert_not_for_kiosks(&run(&app, kiosk_at("loc-a"), LOCATION).await);
    let via_person = r#"{ person(id: "bob") { id location { code } } }"#;
    assert_not_for_kiosks(&run(&app, kiosk_at("loc-a"), via_person).await);
}
