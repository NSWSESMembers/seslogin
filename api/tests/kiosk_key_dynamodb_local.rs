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
    self, EphemeralStateStore as _, Handler as _, KioskKeyStore as _,
    SessionKeyBindConflict as Conflict, SessionKeyTarget,
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

/// Bind at `key_version`, as a page that read the row at that version would.
async fn bind_at(
    h: &seslogin::dynamodb::Handler,
    fingerprint: &str,
    key_version: u64,
    now: u64,
    release: Option<&db::Session>,
    target: SessionKeyTarget<'_>,
) -> Result<db::SessionKeyBound, Conflict> {
    h.bind_session_key(db::SessionKeyBind {
        now,
        key: key(fingerprint, now),
        key_version,
        release,
        target,
    })
    .await
    .unwrap()
}

/// Bind at the row's current version.
async fn bind(
    h: &seslogin::dynamodb::Handler,
    fingerprint: &str,
    now: u64,
    release: Option<&db::Session>,
    target: SessionKeyTarget<'_>,
) -> Result<db::SessionKeyBound, Conflict> {
    let version = h
        .get_kiosk_key(fingerprint)
        .await
        .unwrap()
        .map_or(0, |row| row.version);
    bind_at(h, fingerprint, version, now, release, target).await
}

#[tokio::test]
#[ignore = "needs DynamoDB Local; see the module docs"]
async fn kiosk_key_decides_binds_and_unbinds() {
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

    // A key that never asked can't be bound.
    assert_eq!(
        bind(&h, &fp_a, now, None, new("never")).await.unwrap_err(),
        Conflict::Enrollment
    );

    // A request on an unbound key, then a bind: the row is bound, the request used up,
    // and the old ephemeral_state request deleted.
    ask(&h, &fp_a, now).await;
    let request = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(request.session_id, None);
    assert!(request.is_requested(now));
    let s1 = bind(&h, &fp_a, now, None, new("one"))
        .await
        .unwrap()
        .session;
    let bound = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(bound.session_id.as_deref(), Some(s1.id.as_str()));
    assert_eq!(bound.request_expires_at, None);
    assert_eq!(bound.version, 1);
    assert_eq!(h.get_ephemeral_state(&eid(&fp_a)).await.unwrap(), None);

    // A second page still at the version it read is refused, and the row is untouched.
    assert_eq!(
        bind_at(&h, &fp_a, 0, now, None, new("two"))
            .await
            .unwrap_err(),
        Conflict::Enrollment
    );
    // So is a bind of the used-up request at the new version.
    assert_eq!(
        bind(&h, &fp_a, now, None, new("two")).await.unwrap_err(),
        Conflict::Enrollment
    );
    assert_eq!(h.get_kiosk_key(&fp_a).await.unwrap().unwrap().version, 1);

    // A request on a bound key refreshes it and leaves the binding alone.
    h.record_kiosk_key_request(&fp_a, "other-pk", now, now + 60)
        .await
        .unwrap();
    let refreshed = h.get_kiosk_key(&fp_a).await.unwrap().unwrap();
    assert_eq!(refreshed.session_id.as_deref(), Some(s1.id.as_str()));
    assert_eq!(refreshed.public_key.as_deref(), Some("pk"));
    assert_eq!(refreshed.version, 1);
    assert!(refreshed.is_requested(now));

    // Replace s2's key with fp_a, releasing s1: stale target, stale holder, then success.
    ask(&h, &fp_b, now).await;
    let s2 = bind(&h, &fp_b, now, None, new("two"))
        .await
        .unwrap()
        .session;
    ask(&h, &fp_a, now).await;
    let stale_target = db::Session {
        version: 7,
        ..s2.clone()
    };
    assert_eq!(
        bind(
            &h,
            &fp_a,
            now,
            Some(&s1),
            SessionKeyTarget::Existing(&stale_target)
        )
        .await
        .unwrap_err(),
        Conflict::Target
    );
    let stale_holder = db::Session {
        version: 9,
        ..s1.clone()
    };
    assert_eq!(
        bind(
            &h,
            &fp_a,
            now,
            Some(&stale_holder),
            SessionKeyTarget::Existing(&s2)
        )
        .await
        .unwrap_err(),
        Conflict::Holder
    );
    assert_eq!(
        h.get_kiosk_key(&fp_b).await.unwrap().unwrap().session_id,
        Some(s2.id.clone()),
        "a refused bind changes nothing"
    );

    let moved = bind(&h, &fp_a, now, Some(&s1), SessionKeyTarget::Existing(&s2))
        .await
        .unwrap();
    assert_eq!(moved.released, Some((s1.id.clone(), 2)));
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

    // Moving a kiosk off a key whose row is bound to another session is refused: the
    // kiosk isn't as it was read.
    let fp_d = format!("dynamodb-local-test-{now}-d");
    let fp_e = format!("dynamodb-local-test-{now}-e");
    ask(&h, &fp_d, now).await;
    let s4 = bind(&h, &fp_d, now, None, new("four"))
        .await
        .unwrap()
        .session;
    ask(&h, &fp_e, now).await;
    let s5 = bind(&h, &fp_e, now, None, new("five"))
        .await
        .unwrap()
        .session;
    let s4_claiming_e = db::Session {
        key_fingerprint: Some(fp_e.clone()),
        ..s4.clone()
    };
    ask(&h, &fp_d, now).await;
    assert_eq!(
        bind(
            &h,
            &fp_d,
            now,
            None,
            SessionKeyTarget::Existing(&s4_claiming_e)
        )
        .await
        .unwrap_err(),
        Conflict::Target
    );
    assert_eq!(
        h.get_kiosk_key(&fp_e).await.unwrap().unwrap().session_id,
        Some(s5.id.clone())
    );
    for s in [&s4, &s5] {
        h.update_session(&s.id, db::SessionUpdateShape::Delete)
            .await
            .unwrap();
    }

    // Moving a kiosk off a computer that's asking leaves it asking, unbound, so it can
    // be set up again straight away.
    let fp_i = format!("dynamodb-local-test-{now}-i");
    let fp_j = format!("dynamodb-local-test-{now}-j");
    ask(&h, &fp_i, now).await;
    let s9 = bind(&h, &fp_i, now, None, new("nine"))
        .await
        .unwrap()
        .session;
    ask(&h, &fp_i, now).await;
    ask(&h, &fp_j, now).await;
    bind(&h, &fp_j, now, None, SessionKeyTarget::Existing(&s9))
        .await
        .unwrap();
    let left = h.get_kiosk_key(&fp_i).await.unwrap().unwrap();
    assert_eq!(left.session_id, None);
    assert!(left.is_requested(now));
    let s10 = bind(&h, &fp_i, now, None, new("ten"))
        .await
        .unwrap()
        .session;
    for s in [&s9, &s10] {
        h.update_session(&s.id, db::SessionUpdateShape::Delete)
            .await
            .unwrap();
    }

    // Reconcile's repairs. Binding needs the session to hold the key, and unbinding
    // needs it not to; each is refused, writing nothing, when that isn't so.
    let fp_c = format!("dynamodb-local-test-{now}-c");
    ask(&h, &fp_c, now).await;
    let s3 = bind(&h, &fp_c, now, None, new("three"))
        .await
        .unwrap()
        .session;
    let bound_row = h.get_kiosk_key(&fp_c).await.unwrap().unwrap();
    assert!(
        !h.repair_kiosk_key_unbind(&bound_row, now).await.unwrap(),
        "s3 holds it"
    );
    assert!(
        h.repair_kiosk_key_binding(&fp_c, "pk", &s3.id)
            .await
            .unwrap()
    );
    assert_eq!(h.get_kiosk_key(&fp_c).await.unwrap().unwrap().version, 2);

    h.update_session(&s3.id, db::SessionUpdateShape::Delete)
        .await
        .unwrap();
    assert!(
        !h.repair_kiosk_key_binding(&fp_c, "pk", &s3.id)
            .await
            .unwrap()
    );
    assert_eq!(
        h.get_kiosk_key(&fp_c).await.unwrap(),
        None,
        "unbound by the delete"
    );
    // A plan read before the delete's unbind: the row is gone, so nothing to do.
    assert!(!h.repair_kiosk_key_unbind(&bound_row, now).await.unwrap());
    // The device asking again leaves an unbound row, which an unbind doesn't touch.
    h.record_kiosk_key_request(&fp_c, "pk", now, now + 600)
        .await
        .unwrap();
    let asking_plan = db::KioskKey {
        request_expires_at: Some(now + 600),
        ..bound_row
    };
    assert!(!h.repair_kiosk_key_unbind(&asking_plan, now).await.unwrap());
    assert!(
        h.get_kiosk_key(&fp_c)
            .await
            .unwrap()
            .unwrap()
            .is_requested(now)
    );
}
