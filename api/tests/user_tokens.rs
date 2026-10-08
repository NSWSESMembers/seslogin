//! `slu_` user tokens: found by the row id they embed, and legacy (pre-id)
//! tokens are refused.

mod common;

use sha2::{Digest, Sha256};

use common::{FakeDb, fake_app, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::{self, AuthError, AuthInfo};
use seslogin::client_info::ClientReport;
use seslogin::db::Handler as _;
use seslogin::{mockmail, mockqueue, mockrealtime};

type FakeApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

async fn verify(app: &FakeApp, token: &str) -> Result<AuthInfo, AuthError> {
    auth::verify_token(app, token, &ClientReport::default()).await
}

#[tokio::test]
async fn issued_token_embeds_its_row_id_and_verifies() {
    let app = fake_app();
    seed_user(&app, "user1", true);

    let token = auth::issue_user_token(&app, "user1").await.unwrap();
    let (id, _secret) = token
        .strip_prefix(auth::USER_TOKEN_PREFIX)
        .and_then(|rest| rest.split_once('.'))
        .expect("an id-bound slu_<id>.<secret> token");
    let row = app
        .db
        .get_user_token(id)
        .await
        .unwrap()
        .expect("row stored under the embedded id");
    assert_eq!(row.token_hash, sha256_hex(&token));

    match verify(&app, &token).await {
        Ok(AuthInfo::User { id, token_id, .. }) => {
            assert_eq!(id, "user1");
            assert_eq!(token_id.as_deref(), Some(row.id.as_str()));
        }
        Ok(_) => panic!("expected a user"),
        Err(e) => panic!("verify failed: {e}"),
    }
}

#[tokio::test]
async fn right_id_with_wrong_secret_is_rejected() {
    let app = fake_app();
    seed_user(&app, "user1", true);

    let token = auth::issue_user_token(&app, "user1").await.unwrap();
    let (id_part, _) = token.split_once('.').unwrap();
    let forged = format!("{id_part}.not-the-secret");
    assert!(matches!(
        verify(&app, &forged).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn unknown_id_is_rejected() {
    let app = fake_app();
    seed_user(&app, "user1", true);
    assert!(matches!(
        verify(&app, "slu_noSuchToken.whatever").await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn legacy_token_is_refused_even_if_its_row_survives() {
    let app = fake_app();
    seed_user(&app, "user1", true);

    let legacy = "slu_legacySecretWithNoDot0123456789abcdefghij";
    app.db
        .create_user_token(
            "LegacyRow001",
            &sha256_hex(legacy),
            "user1",
            seslogin::clock::now_sec() + 3600,
        )
        .await
        .unwrap();

    assert!(matches!(
        verify(&app, legacy).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn id_bound_token_expiry_still_slides() {
    let app = fake_app();
    seed_user(&app, "user1", true);

    let token = format!("{}SlidingRow01.secret", auth::USER_TOKEN_PREFIX);
    let expires_at = seslogin::clock::now_sec() + 3600;
    app.db
        .create_user_token("SlidingRow01", &sha256_hex(&token), "user1", expires_at)
        .await
        .unwrap();

    verify(&app, &token).await.unwrap();

    let row = app
        .db
        .get_user_token("SlidingRow01")
        .await
        .unwrap()
        .unwrap();
    assert!(row.expires_at > expires_at);
    assert!(row.last_used_at.is_some());
}
