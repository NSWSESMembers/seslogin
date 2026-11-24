//! The kiosk key writes against a real DynamoDB: DynamoDB Local, never AWS.
//!
//! `FakeDb` applies the same rules, but can't check what only DynamoDB can: condition
//! expression syntax, reserved words, `UpdatedOld` return values and the order of a
//! cancelled transaction's reasons. Opt-in, since CI has no DynamoDB Local:
//!
//! ```sh
//! make local-up local-tables
//! set -a && . ../local/local.env && set +a
//! cargo test --test kiosk_key_dynamodb_local -- --ignored
//! ```
//!
//! Refuses to run unless `AWS_ENDPOINT_URL_DYNAMODB` is on localhost. Each run uses
//! fresh fingerprints and deletes the sessions it made.

use seslogin::db::{
    self, Handler as _, KioskKeyStore as _, SessionKeyBindConflict as Conflict, SessionKeyTarget,
};

async fn local_handler() -> seslogin::dynamodb::Handler {
    let endpoint = std::env::var("AWS_ENDPOINT_URL_DYNAMODB").unwrap_or_default();
    assert!(
        endpoint.starts_with("http://localhost:") || endpoint.starts_with("http://127.0.0.1:"),
        "AWS_ENDPOINT_URL_DYNAMODB must point at DynamoDB Local (source local/local.env), got {endpoint:?}"
    );
    seslogin::dynamodb::Handler::new("seslogin_local", false).await
}

async fn ask(h: &seslogin::dynamodb::Handler, fingerprint: &str, now: u64) {
    use seslogin::db::EphemeralStateStore as _;
    let expires = now + seslogin::session_key::PENDING_ENROLLMENT_TTL_S;
    h.put_ephemeral_state(
        &seslogin::session_key::enroll_state_id(fingerprint),
        seslogin::session_key::ENROLL_STATE_KIND,
        "{}",
        expires,
    )
    .await
    .unwrap();
    h.record_kiosk_key_request(fingerprint, &format!("pk-{fingerprint}"), now, expires)
        .await
        .unwrap();
}

fn key(fingerprint: &str, now: u64) -> db::SessionKeyParams<'_> {
    db::SessionKeyParams {
        public_key: "pk",
        fingerprint,
        expires_at: now + 1000,
    }
}

#[tokio::test]
#[ignore = "needs DynamoDB Local; see the module docs"]
async fn binds_and_unbinds_mirror_into_kiosk_key() {
    let h = local_handler().await;
    let now = seslogin::clock::now_sec();
    let fp_a = format!("dynamodb-local-test-{now}-a");
    let fp_b = format!("dynamodb-local-test-{now}-b");
    let config = serde_json::Map::new();
    let new = |name| SessionKeyTarget::New {
        location_id: "TestAUnit001",
        name,
        config: &config,
        healthcheck_url: None,
    };
    let eid = seslogin::session_key::enroll_state_id;

    // A request on an unbound key, then a bind: the row is bound and the request used up.
    ask(&h, &fp_a, now).await;
    let request = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(request.session_id, None);
    assert!(request.is_requested(now));
    let s1 = h
        .bind_session_key(db::SessionKeyBind {
            enrollment_id: &eid(&fp_a),
            now,
            key: key(&fp_a, now),
            release: &[],
            target: new("one"),
        })
        .await
        .unwrap()
        .unwrap()
        .session;
    let bound = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(bound.session_id.as_deref(), Some(s1.id.as_str()));
    assert_eq!(bound.request_expires_at, None);
    assert_eq!(bound.version, 1);

    // A request on a bound key refreshes it and leaves the binding alone.
    h.record_kiosk_key_request(&fp_a, "other-pk", now, now + 60)
        .await
        .unwrap();
    let refreshed = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(refreshed.session_id.as_deref(), Some(s1.id.as_str()));
    assert_eq!(refreshed.public_key.as_deref(), Some("pk"));
    assert!(refreshed.is_requested(now));

    // The request was used up, so binding it again is refused, and the row is untouched.
    let again = h
        .bind_session_key(db::SessionKeyBind {
            enrollment_id: &eid(&fp_a),
            now,
            key: key(&fp_a, now),
            release: &[],
            target: new("two"),
        })
        .await
        .unwrap();
    assert_eq!(again.unwrap_err(), Conflict::Enrollment);
    assert_eq!(h.get_kiosk_key(&fp_a).await.unwrap().unwrap().version, 1);

    // Replace s2's key with fp_a, releasing s1: stale target, stale holder, then success.
    ask(&h, &fp_b, now).await;
    let s2 = h
        .bind_session_key(db::SessionKeyBind {
            enrollment_id: &eid(&fp_b),
            now,
            key: key(&fp_b, now),
            release: &[],
            target: new("two"),
        })
        .await
        .unwrap()
        .unwrap()
        .session;
    ask(&h, &fp_a, now).await;
    let stale_target = db::Session {
        version: 7,
        ..s2.clone()
    };
    let refused = h
        .bind_session_key(db::SessionKeyBind {
            enrollment_id: &eid(&fp_a),
            now,
            key: key(&fp_a, now),
            release: std::slice::from_ref(&s1),
            target: SessionKeyTarget::Existing(&stale_target),
        })
        .await
        .unwrap();
    assert_eq!(refused.unwrap_err(), Conflict::Target);
    let stale_holder = db::Session {
        version: 9,
        ..s1.clone()
    };
    let refused = h
        .bind_session_key(db::SessionKeyBind {
            enrollment_id: &eid(&fp_a),
            now,
            key: key(&fp_a, now),
            release: std::slice::from_ref(&stale_holder),
            target: SessionKeyTarget::Existing(&s2),
        })
        .await
        .unwrap();
    assert_eq!(refused.unwrap_err(), Conflict::Holder);
    assert_eq!(
        h.get_kiosk_key(&fp_b).await.unwrap().unwrap().session_id,
        Some(s2.id.clone()),
        "a refused bind changes nothing"
    );

    let moved = h
        .bind_session_key(db::SessionKeyBind {
            enrollment_id: &eid(&fp_a),
            now,
            key: key(&fp_a, now),
            release: std::slice::from_ref(&s1),
            target: SessionKeyTarget::Existing(&s2),
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(moved.released, vec![(s1.id.clone(), 2)]);
    assert_eq!(moved.session.version, 2);
    let a = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(a.session_id.as_deref(), Some(s2.id.as_str()));
    assert_eq!(a.version, 2);
    assert_eq!(h.get_kiosk_key(&fp_b).await.unwrap(), None, "s2's old key");

    // Deleting s2 returns the right version and drops its key row.
    let deleted = h
        .update_session(&s2.id, db::SessionUpdateShape::Delete)
        .await
        .unwrap();
    assert_eq!(deleted, Some(3));
    assert_eq!(h.get_kiosk_key(&fp_a).await.unwrap(), None);

    // s1 was released, so has no key; deleting it is just a delete.
    h.update_session(&s1.id, db::SessionUpdateShape::Delete)
        .await
        .unwrap();
}
