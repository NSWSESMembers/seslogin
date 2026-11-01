//! In-app notifications: `notifications::notify` (members, the acting member,
//! best-effort delivery) and the caller's inbox through GraphQL (newest-first paging,
//! unread count, marking read, isolation between members, access lost since, who may
//! use it).
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `location_share.rs`.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_person};
use seslogin::app::MyApp;
use seslogin::audit;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, NotificationRecord};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::notifications::{Audience, NewNotification, notify};

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

/// A member signed in to Tally, whose primary unit is `primary`.
fn member(person_id: &str, primary: &str) -> AuthInfo {
    AuthInfo::Member {
        person_id: person_id.to_string(),
        location_id: primary.to_string(),
        member_location_ids: vec![primary.to_string()],
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

/// Primary unit loc-a.
fn alice() -> AuthInfo {
    member("alice", "loc-a")
}

/// Primary unit loc-a.
fn bob() -> AuthInfo {
    member("bob", "loc-a")
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    seed_person(&app, "carol", "loc-b", "Carol", "Clark", "1003", None);
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

fn json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

fn assert_code(response: &async_graphql::Response, code: &str) {
    assert_eq!(
        code_of(response).as_deref(),
        Some(code),
        "{:?}",
        response.errors
    );
}

/// The sort key a notification created `n` seconds after a fixed point gets, so tests
/// control the order.
fn sk(n: u64) -> String {
    db::notification_sk(1_900_000_000_000 + n * 1000, &format!("n{n}"))
}

/// Store a notification for member `person_id` as if created `n` seconds after a fixed
/// point, unexpired.
fn put(app: &Arc<TestApp>, person_id: &str, n: u64, location: Option<&str>) -> String {
    let record = NotificationRecord {
        person_id: person_id.to_string(),
        sk: sk(n),
        notification_id: format!("n{n}"),
        source: "reservations".to_string(),
        kind: "reservation.for_you".to_string(),
        location_id: location.map(str::to_string),
        actor_person_id: None,
        title: format!("Title {n}"),
        body: None,
        target_type: None,
        target_id: None,
        created_at: 1_900_000_000 + n,
        read_at: None,
        expires_at: seslogin::clock::now_sec() + 3600,
    };
    app.db.notifications.lock().unwrap().push(record.clone());
    record.sk
}

fn stored(app: &Arc<TestApp>, person_id: &str) -> Vec<NotificationRecord> {
    let mut rows: Vec<_> = app
        .db
        .notifications
        .lock()
        .unwrap()
        .iter()
        .filter(|n| n.person_id == person_id)
        .cloned()
        .collect();
    rows.sort_by(|a, b| a.sk.cmp(&b.sk));
    rows
}

fn announcement() -> NewNotification {
    NewNotification::new("reservations", "reservation.for_you", "Truck booked")
        .location("loc-a")
        .body("Tanker 1 on Saturday")
        .target("reservation", "res-1")
}

async fn titles(app: &Arc<TestApp>, auth: AuthInfo, args: &str) -> Vec<String> {
    let response = run(
        app,
        auth,
        &format!("{{ myNotifications{args} {{ edges {{ node {{ title }} }} }} }}"),
    )
    .await;
    json(&response)["myNotifications"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["node"]["title"].as_str().unwrap().to_string())
        .collect()
}

async fn unread(app: &Arc<TestApp>, auth: AuthInfo) -> i64 {
    let response = run(app, auth, "{ myUnreadNotificationCount }").await;
    json(&response)["myUnreadNotificationCount"]
        .as_i64()
        .unwrap()
}

// ── notify ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn notify_gives_each_recipient_a_full_copy() {
    let app = app();
    let before = seslogin::clock::now_sec();
    notify(
        app.as_ref(),
        announcement(),
        Audience::Members(vec!["alice".to_string(), "bob".to_string()]),
    )
    .await;

    let (a, b) = (stored(&app, "alice"), stored(&app, "bob"));
    assert_eq!((a.len(), b.len()), (1, 1));
    let n = &a[0];
    assert_eq!(n.source, "reservations");
    assert_eq!(n.kind, "reservation.for_you");
    assert_eq!(n.location_id.as_deref(), Some("loc-a"));
    assert_eq!(n.title, "Truck booked");
    assert_eq!(n.body.as_deref(), Some("Tanker 1 on Saturday"));
    assert_eq!(n.target_type.as_deref(), Some("reservation"));
    assert_eq!(n.target_id.as_deref(), Some("res-1"));
    assert_eq!(n.read_at, None);
    assert!(n.created_at >= before);
    assert_eq!(n.expires_at, n.created_at + 90 * 24 * 60 * 60);
    // One event: the same ID and sort key for every recipient.
    assert_eq!(n.notification_id, b[0].notification_id);
    assert_eq!(n.sk, b[0].sk);
    assert!(n.sk.ends_with(&format!("#{}", n.notification_id)));
    assert_eq!(n.sk.split('#').next().unwrap().len(), 13);
    assert!(stored(&app, "carol").is_empty());
}

#[tokio::test]
async fn deleted_and_unknown_members_and_repeats_are_skipped() {
    let app = app();
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bob")
        .unwrap()
        .deleted = Some(1);
    notify(
        app.as_ref(),
        announcement(),
        Audience::Members(
            ["alice", "alice", "bob", "nobody"]
                .map(str::to_string)
                .to_vec(),
        ),
    )
    .await;
    assert_eq!(stored(&app, "alice").len(), 1);
    assert_eq!(app.db.notifications.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_acting_member_is_not_notified_of_their_own_action() {
    let app = app();
    let acting = alice();
    audit::scope_for_request(
        Some(&acting),
        None,
        notify(
            app.as_ref(),
            announcement(),
            Audience::Members(vec!["alice".to_string(), "bob".to_string()]),
        ),
    )
    .await;
    assert!(stored(&app, "alice").is_empty());
    let for_bob = stored(&app, "bob");
    assert_eq!(for_bob.len(), 1);
    assert_eq!(for_bob[0].actor_person_id.as_deref(), Some("alice"));

    // A job or a kiosk is not a member, so nobody is left out.
    audit::scope(
        audit::AuditContext::system("sync"),
        notify(
            app.as_ref(),
            announcement(),
            Audience::Members(vec!["alice".to_string()]),
        ),
    )
    .await;
    let for_alice = stored(&app, "alice");
    assert_eq!(for_alice.len(), 1);
    assert_eq!(for_alice[0].actor_person_id, None);
}

#[tokio::test]
async fn a_failed_notification_is_swallowed() {
    let app = app();
    app.db
        .fail_notification_writes
        .store(true, Ordering::SeqCst);
    // Returns normally: the write that prompted it must not be reported as failed.
    notify(
        app.as_ref(),
        announcement(),
        Audience::Members(vec!["alice".to_string()]),
    )
    .await;
    assert!(app.db.notifications.lock().unwrap().is_empty());
}

// ── Reading the inbox ───────────────────────────────────────────────────────

#[tokio::test]
async fn the_inbox_lists_newest_first_with_every_field() {
    let app = app();
    put(&app, "alice", 1, None);
    put(&app, "alice", 3, Some("loc-a"));
    put(&app, "alice", 2, None);
    put(&app, "bob", 4, None);
    assert_eq!(
        titles(&app, alice(), "").await,
        ["Title 3", "Title 2", "Title 1"]
    );

    {
        let mut all = app.db.notifications.lock().unwrap();
        let n = all.iter_mut().find(|n| n.sk == sk(3)).unwrap();
        n.body = Some("The body".to_string());
        n.target_type = Some("reservation".to_string());
        n.target_id = Some("res-9".to_string());
        n.read_at = Some(1_900_000_100);
    }
    let response = run(
        &app,
        alice(),
        "{ myNotifications(first: 1) { edges { cursor node { id source kind title body locationId targetType targetId createdAt readAt read } } pageInfo { hasNextPage endCursor } } }",
    )
    .await;
    let page = &json(&response)["myNotifications"];
    let edge = &page["edges"][0];
    let node = &edge["node"];
    assert_eq!(node["id"], sk(3).as_str());
    assert_eq!(edge["cursor"], sk(3).as_str());
    assert_eq!(node["source"], "reservations");
    assert_eq!(node["kind"], "reservation.for_you");
    assert_eq!(node["title"], "Title 3");
    assert_eq!(node["body"], "The body");
    assert_eq!(node["locationId"], "loc-a");
    assert_eq!(node["targetType"], "reservation");
    assert_eq!(node["targetId"], "res-9");
    assert_eq!(node["createdAt"], 1_900_000_003_i64);
    assert_eq!(node["readAt"], 1_900_000_100_i64);
    assert_eq!(node["read"], true);
    assert_eq!(page["pageInfo"]["hasNextPage"], true);
    assert_eq!(page["pageInfo"]["endCursor"], sk(3).as_str());
}

#[tokio::test]
async fn the_inbox_pages_by_cursor() {
    let app = app();
    for n in 1..=5 {
        put(&app, "alice", n, None);
    }
    assert_eq!(
        titles(&app, alice(), "(first: 2)").await,
        ["Title 5", "Title 4"]
    );
    let after = format!(r#"(first: 2, after: "{}")"#, sk(4));
    assert_eq!(titles(&app, alice(), &after).await, ["Title 3", "Title 2"]);
    let last = format!(r#"(first: 2, after: "{}")"#, sk(2));
    let response = run(
        &app,
        alice(),
        &format!(
            "{{ myNotifications{last} {{ edges {{ node {{ title }} }} pageInfo {{ hasNextPage }} }} }}"
        ),
    )
    .await;
    let page = &json(&response)["myNotifications"];
    assert_eq!(page["edges"].as_array().unwrap().len(), 1);
    assert_eq!(page["pageInfo"]["hasNextPage"], false);

    assert_code(
        &run(
            &app,
            alice(),
            "{ myNotifications(first: 201) { edges { cursor } } }",
        )
        .await,
        "BAD_REQUEST",
    );
}

#[tokio::test]
async fn unread_only_and_the_unread_count() {
    let app = app();
    for n in 1..=3 {
        put(&app, "alice", n, None);
    }
    put(&app, "bob", 4, None);
    assert_eq!(unread(&app, alice()).await, 3);
    assert_eq!(unread(&app, bob()).await, 1);

    let response = run(
        &app,
        alice(),
        &format!(
            r#"mutation {{ markNotificationsRead(ids: ["{}"]) {{ unreadCount }} }}"#,
            sk(2)
        ),
    )
    .await;
    assert_eq!(json(&response)["markNotificationsRead"]["unreadCount"], 2);
    assert_eq!(
        titles(&app, alice(), "(unreadOnly: true)").await,
        ["Title 3", "Title 1"]
    );
    assert_eq!(titles(&app, alice(), "").await.len(), 3);
}

#[tokio::test]
async fn the_unread_count_stops_at_100() {
    let app = app();
    for n in 1..=105 {
        put(&app, "alice", n, None);
    }
    assert_eq!(unread(&app, alice()).await, 100);
}

#[tokio::test]
async fn marking_read_is_idempotent_and_only_touches_the_callers_rows() {
    let app = app();
    put(&app, "alice", 1, None);
    put(&app, "bob", 1, None);
    let mark = |ids: String| {
        format!(r#"mutation {{ markNotificationsRead(ids: [{ids}]) {{ unreadCount }} }}"#)
    };

    // Bob names the sort key both rows share, and one that exists nowhere.
    let response = run(&app, bob(), &mark(format!(r#""{}", "bogus""#, sk(1)))).await;
    assert_eq!(json(&response)["markNotificationsRead"]["unreadCount"], 0);
    assert_eq!(stored(&app, "alice")[0].read_at, None);
    let read_at = stored(&app, "bob")[0].read_at.unwrap();

    // Again: still read, and the original time is kept.
    json(&run(&app, bob(), &mark(format!(r#""{}""#, sk(1)))).await);
    assert_eq!(stored(&app, "bob")[0].read_at, Some(read_at));
    assert_eq!(unread(&app, alice()).await, 1);
}

#[tokio::test]
async fn too_many_ids_to_mark_at_once_is_refused() {
    let app = app();
    let ids = (0..201).map(|n| format!(r#""id{n}""#)).collect::<Vec<_>>();
    let doc = format!(
        "mutation {{ markNotificationsRead(ids: [{}]) {{ unreadCount }} }}",
        ids.join(",")
    );
    assert_code(&run(&app, alice(), &doc).await, "BAD_REQUEST");
}

#[tokio::test]
async fn mark_all_reads_everything_and_nobody_elses() {
    let app = app();
    for n in 1..=250 {
        put(&app, "alice", n, None);
    }
    put(&app, "bob", 1, None);
    let response = run(
        &app,
        alice(),
        "mutation { markAllNotificationsRead { unreadCount } }",
    )
    .await;
    assert_eq!(
        json(&response)["markAllNotificationsRead"]["unreadCount"],
        0
    );
    assert!(stored(&app, "alice").iter().all(|n| n.read_at.is_some()));
    assert_eq!(unread(&app, bob()).await, 1);
    // Read ones still list.
    assert_eq!(titles(&app, alice(), "(first: 200)").await.len(), 200);
    assert!(titles(&app, alice(), "(unreadOnly: true)").await.is_empty());
}

#[tokio::test]
async fn a_notification_about_a_location_the_caller_lost_is_hidden() {
    let app = app();
    put(&app, "alice", 1, Some("loc-a"));
    put(&app, "alice", 2, Some("loc-gone"));
    put(&app, "alice", 3, None);
    assert_eq!(titles(&app, alice(), "").await, ["Title 3", "Title 1"]);
    assert_eq!(titles(&app, bob(), "").await, Vec::<String>::new());
    put(&app, "bob", 4, Some("loc-a"));
    assert_eq!(titles(&app, bob(), "").await, ["Title 4"]);
    // Moved to another unit, so only the notification about nowhere shows...
    let lapsed = member("alice", "loc-b");
    assert_eq!(titles(&app, lapsed.clone(), "").await, ["Title 3"]);
    // ...though the count, which never reads the rows, still includes the others.
    assert_eq!(unread(&app, lapsed).await, 3);
    // A member whose user is a super user sees everything.
    let boss = AuthInfo::Member {
        person_id: "alice".to_string(),
        location_id: "loc-b".to_string(),
        member_location_ids: vec!["loc-b".to_string()],
        user_id: Some("alice-user".to_string()),
        is_super: true,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    };
    assert_eq!(titles(&app, boss, "").await.len(), 3);
}

#[tokio::test]
async fn hidden_rows_do_not_leave_a_short_page() {
    let app = app();
    for n in 1..=6 {
        put(&app, "alice", n, Some("loc-gone"));
    }
    put(&app, "alice", 7, None);
    put(&app, "alice", 0, None);
    // Six hidden rows sit between the two visible ones.
    assert_eq!(
        titles(&app, alice(), "(first: 2)").await,
        ["Title 7", "Title 0"]
    );
}

#[tokio::test]
async fn expired_notifications_are_not_listed() {
    let app = app();
    put(&app, "alice", 1, None);
    put(&app, "alice", 2, None);
    app.db
        .notifications
        .lock()
        .unwrap()
        .iter_mut()
        .find(|n| n.sk == sk(2))
        .unwrap()
        .expires_at = seslogin::clock::now_sec() - 1;
    assert_eq!(titles(&app, alice(), "").await, ["Title 1"]);
}

#[tokio::test]
async fn only_members_have_an_inbox() {
    let app = app();
    put(&app, "alice", 1, None);
    let kiosk = AuthInfo::Session {
        id: "kiosk".to_string(),
        location: "loc-a".to_string(),
    };
    let token = AuthInfo::ApiToken {
        id: "tok".to_string(),
        location_grants: vec!["loc-a".to_string()],
        read_only: false,
    };
    let link = AuthInfo::PeriodLink {
        period_id: "p".to_string(),
    };
    // A user on their own, even through a connected app, is not a Tally member.
    let connected = AuthInfo::User {
        id: "alice".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: Some("grant".to_string()),
    };
    for auth in [kiosk, token, link, connected] {
        for doc in [
            "{ myNotifications { edges { cursor } } }",
            "{ myUnreadNotificationCount }",
            r#"mutation { markNotificationsRead(ids: ["x"]) { unreadCount } }"#,
            "mutation { markAllNotificationsRead { unreadCount } }",
        ] {
            let response = run(&app, auth.clone(), doc).await;
            assert!(!response.errors.is_empty(), "{doc}");
        }
    }
    assert_eq!(stored(&app, "alice")[0].read_at, None);
    assert_eq!(unread(&app, alice()).await, 1);
}
