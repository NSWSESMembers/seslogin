//! `Query.wifiNetworks`: only users get the networks, and a client that already has the
//! current version isn't sent them again. Parsing `WIFI_NETWORKS` is unit-tested in
//! `src/wifi.rs`; it is unset here, so the list is empty.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
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

async fn run(auth: Option<AuthInfo>, query: &str) -> async_graphql::Response {
    let app: Arc<TestApp> = Arc::new(fake_app());
    let schema = graphql::build_schema(app.clone(), webauthn());
    let mut request = Request::new(query)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    if let Some(auth) = auth {
        request = request.data(auth);
    }
    schema.execute(request).await
}

fn user() -> AuthInfo {
    AuthInfo::User {
        id: "alice".to_string(),
        is_super: false,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: Some("grant".to_string()),
    }
}

fn query(known_version: Option<&str>) -> String {
    let arg = known_version.map_or(String::new(), |v| format!("(knownVersion: \"{v}\")"));
    format!("{{ wifiNetworks{arg} {{ version networks {{ ssid }} }} }}")
}

#[tokio::test]
async fn a_user_gets_the_list_and_then_only_its_version() {
    let response = run(Some(user()), &query(None)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let version = data["wifiNetworks"]["version"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(data["wifiNetworks"]["networks"], serde_json::json!([]));

    let response = run(Some(user()), &query(Some(&version))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["wifiNetworks"]["version"], version.as_str());
    assert!(data["wifiNetworks"]["networks"].is_null());
}

#[tokio::test]
async fn kiosks_api_tokens_and_anonymous_callers_are_refused() {
    let callers = [
        Some(AuthInfo::Session {
            id: "kiosk".to_string(),
            location: "loc-a".to_string(),
        }),
        Some(AuthInfo::ApiToken {
            id: "token".to_string(),
            location_grants: vec!["loc-a".to_string()],
            read_only: false,
        }),
        Some(AuthInfo::PeriodLink {
            period_id: "period".to_string(),
        }),
        None,
    ];
    for auth in callers {
        let response = run(auth, &query(None)).await;
        assert!(!response.errors.is_empty());
        assert_eq!(response.data, async_graphql::Value::Null);
    }
}
