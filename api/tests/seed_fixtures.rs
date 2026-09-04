//! Invariants of the committed local-stack fixtures in `local/seed/`.
//!
//! These files are hand-edited, and several of their values are *paired* — a token's
//! plaintext with its sha256, a kiosk's private key with the public half on its session,
//! an id in a fixture with the same id documented in DEVELOPMENT.md. Nothing enforced any
//! of that, so any half could be changed on its own and the break would only surface as a
//! confusing 401 the next time somebody ran the stack.
//!
//! Needs no AWS and no network: it reads the JSON and checks it against itself.

use std::collections::HashSet;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The plaintext tokens DEVELOPMENT.md tells people to use. The fixtures store only their
/// hashes, exactly as production does, so this is the only place the two can be compared.
const USER_TOKENS: &[(&str, &str)] = &[
    ("TestSuperUsr", "slu_localdev0000000000000000000super"),
    ("TestUnitUser", "slu_localdev0000000000000000testunit"),
];

/// Identifiers the docs hand out as things to type or navigate to. Rename one and the docs
/// must be renamed with it. Deliberately not every id in the fixture: the docs identify
/// members by SES number rather than record id, so Bob's and the cross-unit member's ids
/// are theirs to change freely.
const DOCUMENTED: &[&str] = &[
    "TestAUnit001",
    "TestBUnit001",
    "TestAMember1",
    "TestAKiosk01",
    "TestAKiosk02",
    "10000001",
    "10000002",
    "20000001",
    "123456",
];

fn repo_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

fn read_json(rel: &str) -> Value {
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

fn rows<'a>(doc: &'a Value, table: &str) -> &'a [Value] {
    doc["tables"][table]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// The `{"S": "..."}` wire shape the fixtures are written in.
fn s(row: &Value, attr: &str) -> Option<String> {
    row[attr]["S"].as_str().map(str::to_owned)
}

fn id(row: &Value) -> String {
    s(row, "id").expect("every fixture row needs an id")
}

fn synthetic() -> Value {
    read_json("local/seed/synthetic.json")
}

#[test]
fn from_prod_holds_reference_data_only() {
    let doc = read_json("local/seed/from-prod.json");
    let tables: HashSet<&str> = doc["tables"]
        .as_object()
        .expect("tables object")
        .keys()
        .map(String::as_str)
        .collect();

    // Locations and members are invented in synthetic.json. If they reappear here, someone
    // has pointed the extract back at production rows.
    assert_eq!(
        tables,
        HashSet::from(["category", "nitc_group"]),
        "from-prod.json should carry reference data only"
    );

    // The extract refuses to write these; this is the belt to that pair of braces, and it
    // catches a fixture edited by hand as well as one that was generated.
    let text = serde_json::to_string(&doc).unwrap();
    for attr in ["email", "ses_api_person_id"] {
        assert!(
            !text.contains(attr),
            "from-prod.json contains {attr} — this repo is public"
        );
    }
}

#[test]
fn synthetic_references_resolve() {
    let doc = synthetic();
    let locations: HashSet<String> = rows(&doc, "location").iter().map(id).collect();
    let users: HashSet<String> = rows(&doc, "user").iter().map(id).collect();

    for person in rows(&doc, "person") {
        let loc = s(person, "location_id").expect("person.location_id");
        assert!(
            locations.contains(&loc),
            "person {} points at missing location {loc}",
            id(person)
        );
    }

    for session in rows(&doc, "session") {
        let loc = s(session, "location_id").expect("session.location_id");
        assert!(
            locations.contains(&loc),
            "session {} points at missing location {loc}",
            id(session)
        );
    }

    for token in rows(&doc, "user_token") {
        let user = s(token, "user_id").expect("user_token.user_id");
        assert!(
            users.contains(&user),
            "user_token {} points at missing user {user}",
            id(token)
        );
    }

    for user in rows(&doc, "user") {
        for grant in user["location_grants"]["SS"].as_array().unwrap_or(&vec![]) {
            let loc = grant.as_str().expect("location_grants entries are strings");
            assert!(
                locations.contains(loc),
                "user {} is granted missing location {loc}",
                id(user)
            );
        }
    }
}

#[test]
fn sessions_pick_exactly_one_enrolment_style() {
    for session in rows(&synthetic(), "session") {
        let by_code = s(session, "code").is_some();
        let by_key = s(session, "key_fingerprint").is_some();
        // `code` and `key_fingerprint` each back a GSI, so a session carries one or the
        // other. Both would make it findable two ways; neither leaves it unreachable.
        assert!(
            by_code ^ by_key,
            "session {} must have exactly one of code / key_fingerprint (code={by_code}, key={by_key})",
            id(session)
        );

        let config = s(session, "config").expect("session.config");
        serde_json::from_str::<Value>(&config)
            .unwrap_or_else(|e| panic!("session {} has an unparseable config: {e}", id(session)));
    }
}

#[test]
fn both_kiosk_time_entry_branches_are_seeded() {
    let doc = synthetic();
    let easy: Vec<bool> = rows(&doc, "session")
        .iter()
        .map(|session| {
            let config: Value =
                serde_json::from_str(&s(session, "config").expect("config")).unwrap();
            config["easyTimeEntry"].as_bool().unwrap_or(false)
        })
        .collect();

    // ScanScreenAdjust picks the V2 or the legacy time picker on this flag, so the seed
    // keeps a kiosk on each side and neither branch needs a session edited by hand first.
    assert!(
        easy.iter().any(|&e| e) && easy.iter().any(|&e| !e),
        "seed should have a kiosk with easyTimeEntry on and one with it off, got {easy:?}"
    );
}

#[test]
fn user_token_hashes_match_their_documented_plaintexts() {
    let doc = synthetic();
    for (user_id, plaintext) in USER_TOKENS {
        let token = rows(&doc, "user_token")
            .iter()
            .find(|t| s(t, "user_id").as_deref() == Some(*user_id))
            .unwrap_or_else(|| panic!("no user_token seeded for {user_id}"));

        let want = hex::encode(Sha256::digest(plaintext.as_bytes()));
        assert_eq!(
            s(token, "token_hash").expect("token_hash"),
            want,
            "user_token for {user_id} does not hash the plaintext DEVELOPMENT.md documents"
        );
    }
}

#[test]
fn kiosk_signing_key_matches_its_session() {
    let key = read_json("local/seed/kiosk-signing-key.json");
    let doc = synthetic();

    let session_id = key["session_id"].as_str().expect("session_id");
    let session = rows(&doc, "session")
        .iter()
        .find(|s_| id(s_) == session_id)
        .unwrap_or_else(|| panic!("kiosk-signing-key.json names missing session {session_id}"));

    let spki_b64 = key["public_key_spki_b64"]
        .as_str()
        .expect("public_key_spki_b64");
    let spki = BASE64.decode(spki_b64).expect("public key is base64 DER");

    assert_eq!(
        hex::encode(Sha256::digest(&spki)),
        key["fingerprint"].as_str().expect("fingerprint"),
        "fingerprint is not sha256 of the SPKI"
    );
    assert_eq!(
        s(session, "public_key").as_deref(),
        Some(spki_b64),
        "session {session_id} carries a different public key than the key file"
    );
    assert_eq!(
        s(session, "key_fingerprint").as_deref(),
        key["fingerprint"].as_str(),
        "session {session_id} carries a different fingerprint than the key file"
    );

    // The strings agreeing is not the same as the halves being a pair: derive the public
    // key from the private one and compare. This is what actually fails if the key is
    // regenerated and only one of the two files is updated.
    use p256::ecdsa::{SigningKey, VerifyingKey};
    use p256::pkcs8::{DecodePrivateKey, EncodePublicKey};

    let pkcs8 = BASE64
        .decode(key["private_key_pkcs8_b64"].as_str().expect("private key"))
        .expect("private key is base64 DER");
    let signing = SigningKey::from_pkcs8_der(&pkcs8).expect("private key is a P-256 PKCS8 key");
    let derived = VerifyingKey::from(&signing)
        .to_public_key_der()
        .expect("encoding the derived public key");

    assert_eq!(
        derived.as_bytes(),
        spki.as_slice(),
        "the committed private key does not derive the committed public key"
    );
}

#[test]
fn documented_ids_still_exist() {
    let synthetic_text = std::fs::read_to_string(repo_root().join("local/seed/synthetic.json"))
        .expect("reading synthetic.json");
    let docs = ["DEVELOPMENT.md", "CLAUDE.md"].map(|f| {
        std::fs::read_to_string(repo_root().join(f)).unwrap_or_else(|e| panic!("reading {f}: {e}"))
    });

    for name in DOCUMENTED {
        assert!(
            synthetic_text.contains(name),
            "{name} is documented but no longer in synthetic.json"
        );
        assert!(
            docs.iter().any(|d| d.contains(name)),
            "{name} is in the fixtures but named in neither DEVELOPMENT.md nor CLAUDE.md"
        );
    }

    for (_, plaintext) in USER_TOKENS {
        assert!(
            docs[0].contains(plaintext),
            "DEVELOPMENT.md no longer documents the token {plaintext}"
        );
    }
}

#[test]
fn identifiers_hard_coded_in_the_examples_exist() {
    let synthetic_text = std::fs::read_to_string(repo_root().join("local/seed/synthetic.json"))
        .expect("reading synthetic.json");

    let dir = repo_root().join("local/examples");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("reading local/examples") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("mjs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("reading an example");

        // Seed ids (TestAUnit001, TestAKiosk02, ...) and SES numbers, which are what the
        // scripts print and navigate to. A fixture rename that misses them leaves a script
        // confidently telling you to scan a member who no longer exists.
        for token in text.split(|c: char| !c.is_ascii_alphanumeric()) {
            let looks_like_id = token.len() >= 10
                && token.starts_with("Test")
                && token.chars().all(|c| c.is_ascii_alphanumeric());
            let looks_like_member = token.len() == 8 && token.chars().all(|c| c.is_ascii_digit());
            if !(looks_like_id || looks_like_member) {
                continue;
            }
            assert!(
                synthetic_text.contains(token),
                "{} references {token}, which is not in synthetic.json",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "found no seed identifiers in local/examples — has the scan stopped matching?"
    );
}
