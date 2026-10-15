//! `slgn_` API tokens: id-bound tokens are found by primary key, and legacy
//! (pre-id) tokens keep working through `token_hash-index` until replaced.

mod common;

use sha2::{Digest, Sha256};

use common::{FakeDb, fake_app};
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

async fn store(app: &FakeApp, id: &str, token_hash: &str) {
    app.db
        .create_api_token(
            id,
            "test token",
            token_hash,
            vec!["Loc1".into()],
            true,
            None,
            "admin",
        )
        .await
        .unwrap();
}

fn expect_api_token(result: Result<AuthInfo, AuthError>, want_id: &str) {
    match result {
        Ok(AuthInfo::ApiToken {
            id,
            location_grants,
            read_only,
        }) => {
            assert_eq!(id, want_id);
            assert_eq!(location_grants, vec!["Loc1".to_string()]);
            assert!(read_only);
        }
        Ok(_) => panic!("expected an API token"),
        Err(e) => panic!("verify failed: {e}"),
    }
}

#[tokio::test]
async fn generated_token_embeds_its_row_id_and_verifies() {
    let app = fake_app();
    let (secret, hash) = auth::generate_api_token_secret("ApiRow000001");
    assert!(secret.starts_with("slgn_ApiRow000001."));
    assert_eq!(hash, sha256_hex(&secret));
    store(&app, "ApiRow000001", &hash).await;

    expect_api_token(verify(&app, &secret).await, "ApiRow000001");
    let row = app.db.get_api_token("ApiRow000001").await.unwrap().unwrap();
    assert!(row.last_used_at.is_some());
}

#[tokio::test]
async fn right_id_with_wrong_secret_is_rejected() {
    let app = fake_app();
    let (_secret, hash) = auth::generate_api_token_secret("ApiRow000001");
    store(&app, "ApiRow000001", &hash).await;

    assert!(matches!(
        verify(&app, "slgn_ApiRow000001.not-the-secret").await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn legacy_token_still_verifies() {
    let app = fake_app();
    let legacy = "slgn_legacySecretWithNoDot0123456789abcdefghij";
    store(&app, "LegacyApi001", &sha256_hex(legacy)).await;

    expect_api_token(verify(&app, legacy).await, "LegacyApi001");
}

#[tokio::test]
async fn malformed_token_is_rejected_without_a_lookup() {
    let app = fake_app();
    for token in ["slgn_.secret", "slgn_id.", "slgn_."] {
        assert!(
            matches!(verify(&app, token).await, Err(AuthError::Permanent(_))),
            "{token} should be rejected"
        );
    }
}
