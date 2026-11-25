//! Make the `kiosk_key` table agree with the session rows (`plans/kiosk-key-binding.md`).
//!
//! Until that plan's phase 3 the session rows decide which session holds which key, and
//! `kiosk_key` is a shadow written alongside them. A shadow write can be missed: one
//! made before the mirroring code was deployed, by an environment still running older
//! code (prod, preprod and qa share a database), or by a best-effort write that failed.
//! This finds and repairs every disagreement, so that reads can move to `kiosk_key`.
//!
//! The session side is the truth here. Each repair is a transaction conditional on the
//! session row still saying what the plan read, so a kiosk being set up in the meantime
//! is left alone and the next run sees it. A key held by more than one live session is
//! the very failure the plan exists to prevent, and neither row says which is right, so
//! it is reported and never touched.

use std::collections::{BTreeMap, HashMap};

use tracing::{error, info, warn};

use crate::db::{self, Handler, KioskKey, KioskKeyStore, Session};

/// Rows examined per session scan request.
const SCAN_PAGE_LIMIT: i32 = 500;

/// Why a key row needs binding to its session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindReason {
    /// There is no row for the key.
    Missing,
    /// The row exists but is bound to nothing.
    Unbound,
    /// The row names another session.
    OtherSession(String),
    /// The row names this session, with a different public key.
    PublicKey,
}

impl BindReason {
    pub fn describe(&self) -> String {
        match self {
            Self::Missing => "no row".to_string(),
            Self::Unbound => "row unbound".to_string(),
            Self::OtherSession(id) => format!("row names {id}"),
            Self::PublicKey => "public key differs".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bind {
    pub fingerprint: String,
    pub public_key: String,
    pub session_id: String,
    pub reason: BindReason,
}

/// What a run would change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Key rows to bind to the session that holds them.
    pub bind: Vec<Bind>,
    /// Key rows naming a session that doesn't hold their key.
    pub unbind: Vec<KioskKey>,
    /// Keys more than one live session holds, with those sessions' ids. Not repaired.
    pub duplicates: Vec<(String, Vec<String>)>,
    /// Sessions holding a fingerprint with no public key. Not repaired.
    pub keyless: Vec<String>,
}

impl Plan {
    pub fn is_clean(&self) -> bool {
        self.bind.is_empty()
            && self.unbind.is_empty()
            && self.duplicates.is_empty()
            && self.keyless.is_empty()
    }
}

/// Compare the session rows with the key rows. `sessions` may include deleted rows.
pub fn plan(sessions: &[Session], keys: &[KioskKey]) -> Plan {
    let mut holders: BTreeMap<&str, Vec<&Session>> = BTreeMap::new();
    for session in sessions.iter().filter(|s| s.active) {
        if let Some(fingerprint) = session.key_fingerprint.as_deref() {
            holders.entry(fingerprint).or_default().push(session);
        }
    }
    let rows: HashMap<&str, &KioskKey> = keys.iter().map(|k| (k.fingerprint.as_str(), k)).collect();

    let mut plan = Plan::default();
    for (&fingerprint, sessions) in &holders {
        let [session] = sessions.as_slice() else {
            let mut ids: Vec<String> = sessions.iter().map(|s| s.id.clone()).collect();
            ids.sort();
            plan.duplicates.push((fingerprint.to_string(), ids));
            continue;
        };
        let Some(public_key) = session.public_key.clone() else {
            plan.keyless.push(session.id.clone());
            continue;
        };
        let reason = match rows.get(fingerprint) {
            None => BindReason::Missing,
            Some(row) => match row.session_id.as_deref() {
                None => BindReason::Unbound,
                Some(id) if id != session.id => BindReason::OtherSession(id.to_string()),
                Some(_) if row.public_key.as_deref() != Some(public_key.as_str()) => {
                    BindReason::PublicKey
                }
                Some(_) => continue,
            },
        };
        plan.bind.push(Bind {
            fingerprint: fingerprint.to_string(),
            public_key,
            session_id: session.id.clone(),
            reason,
        });
    }
    let mut unbind: Vec<KioskKey> = keys
        .iter()
        .filter(|k| k.session_id.is_some() && !holders.contains_key(k.fingerprint.as_str()))
        .cloned()
        .collect();
    unbind.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
    plan.unbind = unbind;
    plan
}

/// What a run did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub sessions_scanned: usize,
    pub keys_scanned: usize,
    /// Rows of either table that couldn't be read. They are left out of the plan.
    pub unreadable: usize,
    pub plan: Plan,
    /// Repairs made. Always 0 on a dry run.
    pub repaired: usize,
    /// Repairs skipped because the rows changed since they were read.
    pub skipped_changed: usize,
    pub failed: usize,
}

impl Summary {
    /// Whether a human needs to look: unreadable rows, failed repairs, or keys the run
    /// can't repair.
    pub fn needs_attention(&self) -> bool {
        self.unreadable > 0
            || self.failed > 0
            || !self.plan.duplicates.is_empty()
            || !self.plan.keyless.is_empty()
    }
}

/// Read both tables, plan, and (unless `dry_run`) repair. Fails only if a scan does.
pub async fn run<D: Handler + KioskKeyStore>(
    db: &D,
    now: u64,
    dry_run: bool,
) -> db::Result<Summary> {
    let mut summary = Summary::default();
    let mut sessions = Vec::new();
    let mut cursor = None;
    loop {
        let page = db.scan_sessions(cursor, SCAN_PAGE_LIMIT).await?;
        for row in page.rows {
            summary.sessions_scanned += 1;
            match row {
                Ok(session) => sessions.push(session),
                Err(e) => {
                    error!("Skipping unreadable session row: {e}");
                    summary.unreadable += 1;
                }
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    let mut keys = Vec::new();
    for row in db.list_kiosk_keys().await? {
        summary.keys_scanned += 1;
        match row {
            Ok(key) => keys.push(key),
            Err(e) => {
                error!("Skipping unreadable kiosk_key row: {e}");
                summary.unreadable += 1;
            }
        }
    }

    summary.plan = plan(&sessions, &keys);
    for (fingerprint, ids) in &summary.plan.duplicates {
        error!("Kiosk key {fingerprint} is held by more than one live session: {ids:?}");
    }
    for id in &summary.plan.keyless {
        error!("Kiosk session {id} has a key fingerprint but no public key");
    }
    if dry_run {
        return Ok(summary);
    }

    for bind in &summary.plan.bind {
        match db
            .repair_kiosk_key_binding(&bind.fingerprint, &bind.public_key, &bind.session_id)
            .await
        {
            Ok(true) => {
                info!(
                    "Bound kiosk key {} to session {} ({})",
                    bind.fingerprint,
                    bind.session_id,
                    bind.reason.describe()
                );
                summary.repaired += 1;
            }
            Ok(false) => summary.skipped_changed += 1,
            Err(e) => {
                warn!("Failed to bind kiosk key {}: {e}", bind.fingerprint);
                summary.failed += 1;
            }
        }
    }
    for row in &summary.plan.unbind {
        match db.repair_kiosk_key_unbind(row, now).await {
            Ok(true) => {
                info!(
                    "Unbound kiosk key {} from session {:?}, which doesn't hold it",
                    row.fingerprint, row.session_id
                );
                summary.repaired += 1;
            }
            Ok(false) => summary.skipped_changed += 1,
            Err(e) => {
                warn!("Failed to unbind kiosk key {}: {e}", row.fingerprint);
                summary.failed += 1;
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, fingerprint: Option<&str>, active: bool) -> Session {
        Session {
            id: id.to_string(),
            name: id.to_string(),
            location_id: "loc".to_string(),
            active,
            last_contact: None,
            client_version: None,
            client_info: None,
            client_info_updated_at: None,
            config: serde_json::Map::new(),
            healthcheck_url: None,
            public_key: fingerprint.map(|f| format!("pk-{f}")),
            key_fingerprint: fingerprint.map(str::to_string),
            key_expires_at: None,
            key_released_at: None,
            retired_at: None,
            version: 1,
            created_at: None,
            updated_at: None,
        }
    }

    fn key(fingerprint: &str, session_id: Option<&str>) -> KioskKey {
        KioskKey {
            fingerprint: fingerprint.to_string(),
            public_key: Some(format!("pk-{fingerprint}")),
            session_id: session_id.map(str::to_string),
            ..KioskKey::default()
        }
    }

    #[test]
    fn agreeing_tables_plan_nothing() {
        let plan = plan(
            &[
                session("s1", Some("f1"), true),
                session("gone", None, false),
            ],
            &[key("f1", Some("s1")), key("asking", None)],
        );
        assert!(plan.is_clean(), "{plan:?}");
    }

    #[test]
    fn each_disagreement_is_planned() {
        let mut wrong_pk = key("f4", Some("s4"));
        wrong_pk.public_key = Some("old".to_string());
        let plan = plan(
            &[
                session("s1", Some("f1"), true),
                session("s2", Some("f2"), true),
                session("s3", Some("f3"), true),
                session("s4", Some("f4"), true),
                session("deleted", Some("f5"), false),
            ],
            &[
                key("f2", None),
                key("f3", Some("other")),
                wrong_pk,
                key("f5", Some("deleted")),
                key("f6", Some("missing-session")),
            ],
        );
        let reasons: Vec<(&str, BindReason)> = plan
            .bind
            .iter()
            .map(|b| (b.fingerprint.as_str(), b.reason.clone()))
            .collect();
        assert_eq!(
            reasons,
            vec![
                ("f1", BindReason::Missing),
                ("f2", BindReason::Unbound),
                ("f3", BindReason::OtherSession("other".to_string())),
                ("f4", BindReason::PublicKey),
            ]
        );
        let unbind: Vec<&str> = plan.unbind.iter().map(|k| k.fingerprint.as_str()).collect();
        assert_eq!(unbind, vec!["f5", "f6"]);
        assert!(plan.duplicates.is_empty());
    }

    /// Neither session row says which is right, so the key's row is left alone too.
    #[test]
    fn a_key_two_live_sessions_hold_is_reported_not_repaired() {
        let plan = plan(
            &[
                session("s1", Some("f1"), true),
                session("s2", Some("f1"), true),
            ],
            &[key("f1", Some("s3"))],
        );
        assert_eq!(
            plan.duplicates,
            vec![("f1".to_string(), vec!["s1".to_string(), "s2".to_string()])]
        );
        assert!(plan.bind.is_empty());
        assert!(plan.unbind.is_empty());
    }
}
