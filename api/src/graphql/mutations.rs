#![allow(clippy::too_many_arguments)]
use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use async_graphql::Context;
use async_graphql::Enum;
use async_graphql::ID;
use async_graphql::MaybeUndefined;
use async_graphql::Object;
use async_graphql::SimpleObject;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

use crate::app::App;
use crate::app::HasDb;
use crate::app::HasSqs;
use crate::auth;
use crate::auth::AuthInfo;
use crate::db;
use crate::db::Handler;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hex;

use super::auth::{
    AuthGuard, AuthRequirement, require_location_access, require_period_access, require_writable,
};
use super::error::{ApiError, ErrorCode};
use super::query::{QuickPick, build_quick_pick};
use super::{ApiToken, Category, Location, NitcGroup, PasskeyInfo, Period, Person, Session, User};

/// Longest entry a member-facing edit link may set. The admin form only *warns*
/// past 24h, but an admin can be trusted to mean it; a link holder correcting
/// their own attendance cannot plausibly need longer, so this is a hard stop.
const LINK_EDIT_MAX_DURATION_S: i64 = 24 * 60 * 60;

/// Bound the span an edit link may set. Callers check `start_time < end_time`
/// first, so this only has to catch the upper end.
fn validate_link_edit_duration(start_time: i64, end_time: i64) -> Result<()> {
    if end_time - start_time > LINK_EDIT_MAX_DURATION_S {
        return Err(anyhow!("A time entry cannot be longer than 24 hours"));
    }
    Ok(())
}

pub(crate) const PERIOD_REMINDER_SUBJECT: &str = "Please check your SES activity time entry";

/// Render a timestamp in Sydney local time, matching the activity summary email.
fn format_reminder_datetime(ts: u64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono_tz::Australia::Sydney)
        .format("%a %-d %b %Y, %H:%M")
        .to_string()
}

/// Body of the "please check your time entry" email.
///
/// Plain text on purpose: it renders identically everywhere, and a bare URL is
/// easier to trust than a styled button in a message asking someone to click a
/// link. Kept as a pure function so the wording and the still-signed-in case are
/// unit-testable without SES.
fn build_period_reminder_email(
    first_name: &str,
    location_name: &str,
    category_name: Option<&str>,
    start_time: u64,
    end_time: Option<u64>,
    url: &str,
) -> String {
    let greeting = if first_name.trim().is_empty() {
        "Hello,".to_string()
    } else {
        format!("Hi {},", first_name.trim())
    };
    let end_line = match end_time {
        Some(end) => format!("  Finished: {}\n", format_reminder_datetime(end)),
        // An entry with no end time is someone still signed in; say so rather
        // than printing a blank field they can't explain.
        None => "  Finished: still signed in\n".to_string(),
    };

    format!(
        "{greeting}\n\n\
         Please check the following activity recorded for you at {location_name}:\n\n\
         {}  Started:  {}\n{end_line}\n\
         If that isn't right, you can correct the times and the activity here:\n\n\
         {url}\n\n\
         This link works for the next 48 hours and only opens this one entry. \
         If the details above are already correct, you don't need to do anything.\n\n\
         Thanks,\n\
         SES Activity administrators\n",
        category_name
            .map(|c| format!("  Activity: {c}\n"))
            .unwrap_or_default(),
        format_reminder_datetime(start_time),
    )
}

fn parse_session_config_json(
    config: Option<&str>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let Some(config) = config else {
        return Ok(serde_json::Map::new());
    };
    if config.trim().is_empty() {
        return Ok(serde_json::Map::new());
    }

    match serde_json::from_str::<serde_json::Value>(config)? {
        serde_json::Value::Object(obj) => Ok(obj),
        _ => Err(anyhow!("Session config must be a JSON object")),
    }
}

fn normalize_healthcheck_url(healthcheck_url: Option<&str>) -> Result<Option<String>> {
    let Some(healthcheck_url) = healthcheck_url.map(str::trim) else {
        return Ok(None);
    };

    if healthcheck_url.is_empty() {
        return Ok(None);
    }

    if healthcheck_url.len() > 255 {
        return Err(anyhow!("Health check URL must be 255 characters or fewer"));
    }

    let parsed = reqwest::Url::parse(healthcheck_url)
        .map_err(|_| anyhow!("Health check URL must be a valid absolute URL"))?;

    match parsed.scheme() {
        "http" | "https" => Ok(Some(healthcheck_url.to_string())),
        _ => Err(anyhow!("Health check URL must use http or https")),
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
enum RegisterState {
    NotFound,
    SignedIn,
    SignOutPending,
}

#[derive(SimpleObject)]
struct RegisterResult<A: App + HasDb + Send + Sync + 'static> {
    state: RegisterState,
    period: Option<Period<A>>,
    /// Shortcuts for the sign-out category screen, saving the kiosk a round trip.
    ///
    /// Only ever populated when `state` is `SIGN_OUT_PENDING` and the caller asked
    /// for it via `quickPick: true` — in any other case there is no sign-out screen
    /// to fill in, and computing it would be wasted reads on the scan path. Null is
    /// also what a failed build returns, so the kiosk falls back to the full
    /// category tree rather than failing the scan.
    quick_pick: Option<QuickPick<A>>,
}

#[derive(SimpleObject)]
struct CreateApiTokenResult {
    /// The newly-created api token record (no secret).
    token: ApiToken,
    /// The plaintext secret. Returned only once at creation — never again.
    secret: String,
}

pub struct MutationRoot<A: App + HasDb + HasSqs + Send + Sync> {
    pub(super) app: Arc<A>,
}

impl<A: App + HasDb + HasSqs + Send + Sync + 'static> MutationRoot<A> {
    /// Enqueue a Phase 1 (period) NITC export for a mutated period.
    ///
    /// `old_nitc_event_id` is the event the period was assigned to *before* this mutation (read
    /// from the pre-update record). If set, we bump that event's version synchronously here.
    /// Phase 2 (event_export) messages carry a 60s delay and are guarded only by the event
    /// version, so without this bump an already-queued event_export could fire after this period
    /// was mutated but before its Phase 1 reassignment runs — syncing an inconsistent snapshot to
    /// SES. Bumping the version now makes any in-flight event_export for that event stale; Phase 1
    /// will enqueue a fresh one reflecting the settled state.
    async fn enqueue_nitc_export(
        &self,
        period_id: &str,
        old_nitc_event_id: Option<&str>,
    ) -> Result<()> {
        // The bump must not be best-effort: if it fails the in-flight event_export for this
        // event would not be invalidated, so an inconsistent snapshot could reach SES. Fail
        // the whole mutation so the caller retries rather than silently leaving the race open.
        if let Some(event_id) = old_nitc_event_id {
            self.app
                .db()
                .bump_nitc_event_version(event_id)
                .await
                .with_context(|| {
                    format!(
                        "bumping NITC event {} version for mutated period {}",
                        event_id, period_id
                    )
                })?;
        }
        let sqs = &self.app.sqs().nitc_export;
        if let Err(e) =
            crate::sqs_dispatch::enqueue_period_nitc_export(&sqs.client, &sqs.queue_url, period_id)
                .await
        {
            warn!(
                "Failed to enqueue NITC export for period {}: {}",
                period_id, e
            );
        }
        Ok(())
    }

    /// Reject the mutation if any non-deleted person already holds `registration_number`.
    ///
    /// Registration numbers (member numbers) are intended to be globally unique. DynamoDB cannot
    /// enforce uniqueness on a non-key attribute, so we check at the application layer before
    /// writing. Soft-deleted holders do not block reuse — only active duplicates are the problem.
    ///
    /// `exclude_person_id` is the id of the record being edited, so that re-saving a person with
    /// its own unchanged number succeeds.
    async fn ensure_registration_number_available(
        &self,
        registration_number: &str,
        exclude_person_id: Option<&str>,
    ) -> Result<()> {
        let candidate_ids: Vec<String> = self
            .app
            .db()
            .get_person_id_by_registration_number(registration_number)
            .await?
            .into_iter()
            .filter(|id| Some(id.as_str()) != exclude_person_id)
            .collect();

        if candidate_ids.is_empty() {
            return Ok(());
        }

        let has_active_holder = self
            .app
            .db()
            .get_persons(&candidate_ids)
            .await?
            .into_iter()
            .flatten()
            .any(|person| person.deleted.is_none());

        if has_active_holder {
            return Err(anyhow!(
                "A member with member number {registration_number} already exists"
            ));
        }

        Ok(())
    }
}

#[Object]
impl<A: App + HasDb + HasSqs + Send + Sync + 'static> MutationRoot<A> {
    async fn auth_session(&self, code: String) -> Option<String> {
        let res = auth::issue_token_for_scan_code(&*self.app, &code).await;

        match res {
            Ok(token) => Some(token),
            Err(e) => {
                info!("Auth failed for code {}: {}", code, e);

                // hide details of auth error from user
                None
            }
        }
    }

    /// Publish a kiosk's public key as a pending enrollment (public-key/QR flow). The
    /// kiosk re-submits every ~10 min while unenrolled; an admin then scans the kiosk's
    /// QR code to complete enrollment via [`enroll_session`]. Returns the server-computed
    /// key fingerprint (hex SHA-256 of the SPKI DER) that the QR code carries.
    ///
    /// Intentionally unauthenticated and cheap to serve: the input is strictly validated
    /// (must be a well-formed P-256 SPKI ≤200 bytes) and writes are keyed by fingerprint,
    /// so a flood from one key only ever touches one item. A short-lived record already
    /// present is not rewritten (write-suppression below).
    async fn submit_enrollment_key(&self, public_key: String) -> Result<String> {
        let (_, fingerprint) = crate::session_key::validate_public_key_spki_b64(&public_key)
            .map_err(|e| anyhow!("Invalid public key: {e:#}"))?;
        let id = crate::session_key::enroll_state_id(&fingerprint);
        let now = crate::clock::now_sec();

        // Write-suppression: if a fresh record already exists (written in roughly the last
        // 10 min, i.e. >20 min TTL remaining), skip the write. The kiosk polls every
        // ~10 min, so steady state is about one write per 20 min per kiosk.
        let fresh_threshold = now + crate::session_key::PENDING_ENROLLMENT_TTL_S - 10 * 60;
        if let Some(existing) = self.app.db().get_ephemeral_state(&id).await?
            && existing.kind == crate::session_key::ENROLL_STATE_KIND
            && existing.expires_at > fresh_threshold
        {
            return Ok(fingerprint);
        }

        let payload = serde_json::to_string(&crate::session_key::EnrollPayload {
            public_key,
            submitted_at: now,
        })?;
        let expires_at = now + crate::session_key::PENDING_ENROLLMENT_TTL_S;
        self.app
            .db()
            .put_ephemeral_state(
                &id,
                crate::session_key::ENROLL_STATE_KIND,
                &payload,
                expires_at,
            )
            .await?;
        Ok(fingerprint)
    }

    /// Request an email login code. Always returns true to avoid email enumeration.
    /// Requires a valid Cloudflare Turnstile token.
    async fn request_auth_code(&self, email: String, turnstile_token: String) -> bool {
        use sha2::{Digest, Sha256};

        match crate::turnstile::verify(&turnstile_token).await {
            Ok(true) => {}
            Ok(false) => {
                info!("Turnstile challenge failed for request_auth_code");
                return true;
            }
            Err(e) => {
                warn!("Turnstile error in request_auth_code: {:#}", e);
                return true;
            }
        }

        let lookup = self
            .app
            .db()
            .get_user_id_by_email(&email)
            .await
            .and_then(|ids| db::at_most_one(ids, || format!("Multiple users share email {email}")));
        let user_id = match lookup {
            Ok(Some(id)) => id,
            Ok(None) => return true,
            Err(e) => {
                warn!("DB error looking up user in request_auth_code: {:#}", e);
                return true;
            }
        };

        match self.app.db().get_users(&[&user_id]).await {
            Ok(users) => match users.into_iter().next().flatten() {
                Some(user) if user.enabled => {}
                _ => {
                    info!("request_auth_code: user disabled or missing id={}", user_id);
                    return true;
                }
            },
            Err(e) => {
                warn!(
                    "DB error checking user enabled in request_auth_code: {:#}",
                    e
                );
                return true;
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Rate limit: at most one code per 30 seconds per email
        if let Ok(Some(existing)) = self.app.db().get_login_code(&email).await
            && now < existing.last_sent_at + 30
        {
            info!("Rate limit hit for request_auth_code email={}", email);
            return true;
        }

        let code = crate::nonce::generate_code(6);

        // log only in debug builds to avoid putting codes in production logs
        #[cfg(debug_assertions)]
        info!("Email login code for email={}: {}", email, code);
        let code_hash = {
            let mut hasher = Sha256::new();
            hasher.update(code.as_bytes());
            hex::encode(hasher.finalize())
        };
        let expires_at = now + 10 * 60;

        if let Err(e) = self
            .app
            .db()
            .put_login_code(&email, &code_hash, expires_at, now)
            .await
        {
            warn!("Failed to store login code: {:#}", e);
            return true;
        }

        let subject = "Your seslogin login code";
        let body = format!(
            "Your login code is: {}\n\nThis code expires in 10 minutes. Do not share it.\n\nIf you did not request this code, you can ignore this email.",
            code
        );

        tracing::info!(user_id = %user_id, "Sending login code to {}", email);
        if let Err(e) = crate::mail::send_plain_text(&email, subject, &body).await {
            warn!("Failed to send login code email to {}: {:#}", email, e);
        }

        true
    }

    /// Verify an email login code and return an opaque session token on success.
    async fn verify_auth_code(&self, email: String, code: String) -> Option<String> {
        use sha2::{Digest, Sha256};

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let record = match self.app.db().get_login_code(&email).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                info!("verify_auth_code: no code for email={}", email);
                return None;
            }
            Err(e) => {
                warn!("DB error in verify_auth_code: {:#}", e);
                return None;
            }
        };

        if now >= record.expires_at {
            let _ = self.app.db().delete_login_code(&email).await;
            info!("verify_auth_code: expired code for email={}", email);
            return None;
        }

        if record.attempts >= 5 {
            let _ = self.app.db().delete_login_code(&email).await;
            info!("verify_auth_code: too many attempts for email={}", email);
            return None;
        }

        let _ = self.app.db().increment_login_code_attempts(&email).await;

        let expected_hash = {
            let mut hasher = Sha256::new();
            hasher.update(code.as_bytes());
            hex::encode(hasher.finalize())
        };

        if record.code_hash != expected_hash {
            info!("verify_auth_code: wrong code for email={}", email);
            return None;
        }

        let _ = self.app.db().delete_login_code(&email).await;

        let lookup = self
            .app
            .db()
            .get_user_id_by_email(&email)
            .await
            .and_then(|ids| db::at_most_one(ids, || format!("Multiple users share email {email}")));
        let user_id = match lookup {
            Ok(Some(id)) => id,
            Ok(None) => {
                warn!("verify_auth_code: user not found for email={}", email);
                return None;
            }
            Err(e) => {
                warn!("DB error fetching user in verify_auth_code: {:#}", e);
                return None;
            }
        };

        match self.app.db().get_users(&[&user_id]).await {
            Ok(users) => match users.into_iter().next().flatten() {
                Some(user) if user.enabled => {}
                _ => {
                    info!("verify_auth_code: user disabled or missing id={}", user_id);
                    return None;
                }
            },
            Err(e) => {
                warn!(
                    "DB error checking user enabled in verify_auth_code: {:#}",
                    e
                );
                return None;
            }
        }

        match auth::issue_user_token(&*self.app, &user_id).await {
            Ok(token) => {
                info!("Issued user token for user_id={}", user_id);
                Some(token)
            }
            Err(e) => {
                warn!("Failed to issue user token: {:#}", e);
                None
            }
        }
    }

    /// Revoke the current user's opaque session token (no-op for JWT sessions).
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn logout(&self, ctx: &Context<'_>) -> Result<bool> {
        if let Some(AuthInfo::User {
            token_id: Some(token_id),
            ..
        }) = ctx.data_opt::<AuthInfo>()
        {
            self.app.db().delete_user_token(token_id).await?;
        }
        Ok(true)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn create_user(
        &self,
        email: String,
        is_super: bool,
        location_grants: Vec<String>,
    ) -> Result<User<A>> {
        if !location_grants.is_empty() {
            let found = self
                .app
                .db()
                .get_locations(location_grants.as_slice())
                .await?;
            for (id, loc) in location_grants.iter().zip(found.iter()) {
                if loc.is_none() {
                    return Err(anyhow!("Location {:?} not found", id));
                }
            }
        }
        let rec = self
            .app
            .db()
            .create_user(&email, is_super, location_grants)
            .await?;

        // TODO: email user with setup instructions

        Ok(User::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn update_user(
        &self,
        id: ID,
        email: String,
        is_super: bool,
        is_dev: bool,
        enabled: bool,
        location_grants: Vec<String>,
    ) -> Result<User<A>> {
        if !location_grants.is_empty() {
            let found = self
                .app
                .db()
                .get_locations(location_grants.as_slice())
                .await?;
            for (id, loc) in location_grants.iter().zip(found.iter()) {
                if loc.is_none() {
                    return Err(anyhow!("Location {:?} not found", id));
                }
            }
        }
        self.app
            .db()
            .update_user(
                &id,
                db::UserUpdateShape::Fields {
                    email: &email,
                    is_super,
                    is_dev,
                    enabled,
                    location_grants,
                },
            )
            .await
            .map_err(|e| {
                warn!("db error: {:?}", e);
                e
            })?;

        let rec = self.app.db().get_users(&[&id]).await?;

        Ok(User::new(
            rec.into_iter()
                .next()
                .flatten()
                .ok_or_else(|| anyhow!("User with ID {:?} missing", id))?,
        ))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn create_person(
        &self,
        ctx: &Context<'_>,
        location_id: ID,
        first_name: String,
        last_name: String,
        #[graphql(name = "memberNumber")] registration_number: String,
    ) -> Result<Person<A>> {
        require_writable(ctx)?;
        require_location_access(ctx, &location_id)?;
        self.app
            .db()
            .get_locations(&[&location_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Location {:?} not found", location_id))?;

        self.ensure_registration_number_available(&registration_number, None)
            .await?;

        let rec = self
            .app
            .db()
            .create_person(&location_id, &first_name, &last_name, &registration_number)
            .await?;

        Ok(Person::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn update_person(
        &self,
        ctx: &Context<'_>,
        id: ID,
        first_name: String,
        last_name: String,
        #[graphql(name = "memberNumber")] registration_number: String,
    ) -> Result<Person<A>> {
        require_writable(ctx)?;
        let existing = self
            .app
            .db()
            .get_persons(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {:?} missing", id))?;
        require_location_access(ctx, &existing.location_id)?;

        self.ensure_registration_number_available(&registration_number, Some(id.as_str()))
            .await?;

        self.app
            .db()
            .update_person(
                &id,
                db::PersonUpdateShape::Fields {
                    first_name: &first_name,
                    last_name: &last_name,
                    registration_number: &registration_number,
                },
            )
            .await?;

        let mut rec = self.app.db().get_persons(&[&id]).await?;
        Ok(Person::new(rec.pop().flatten().ok_or_else(|| {
            anyhow!("Person with ID {:?} missing", id)
        })?))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn delete_person(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_writable(ctx)?;
        let existing = self
            .app
            .db()
            .get_persons(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {:?} missing", id))?;
        require_location_access(ctx, &existing.location_id)?;

        self.app
            .db()
            .update_person(&id, db::PersonUpdateShape::Delete)
            .await?;
        Ok(true)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::UserOrApiToken)")]
    async fn create_period(
        &self,
        ctx: &Context<'_>,
        person_id: ID,
        location_id: ID,
        category_id: ID,
        start_time: i64,
        end_time: i64,
        comment: Option<String>,
    ) -> Result<Period<A>> {
        require_writable(ctx)?;
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        require_location_access(ctx, &location_id)?;
        self.app
            .db()
            .get_locations(&[&location_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Location {:?} not found", location_id))?;
        self.app
            .db()
            .get_persons(&[&person_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Person {:?} not found", person_id))?;
        self.app
            .db()
            .get_categories(&[&category_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Category {:?} not found", category_id))?;

        let rec = self
            .app
            .db()
            .create_period(
                &person_id,
                &location_id,
                &category_id,
                start_time as u64,
                end_time as u64,
                comment.as_deref(),
            )
            .await?;

        // Newly created period is not yet assigned to any NITC event.
        self.enqueue_nitc_export(&rec.id, None).await?;
        Ok(Period::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn update_period(
        &self,
        ctx: &Context<'_>,
        id: ID,
        person_id: ID,
        location_id: ID,
        category_id: ID,
        start_time: i64,
        end_time: i64,
        // Three-state: omit to leave the comment unchanged, pass `null` to clear
        // it, or pass a string to set it. Omitting keeps existing callers working.
        comment: MaybeUndefined<String>,
    ) -> Result<Period<A>> {
        require_writable(ctx)?;
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let existing = self
            .app
            .db()
            .get_periods(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;
        if existing.guest_name.is_some() {
            return Err(anyhow!("Cannot edit a guest period"));
        }
        require_location_access(ctx, &existing.location_id)?;
        require_location_access(ctx, &location_id)?;
        self.app
            .db()
            .get_locations(&[&location_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Location {:?} not found", location_id))?;
        self.app
            .db()
            .get_persons(&[&person_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Person {:?} not found", person_id))?;
        self.app
            .db()
            .get_categories(&[&category_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Category {:?} not found", category_id))?;

        let comment = match &comment {
            MaybeUndefined::Undefined => None,
            MaybeUndefined::Null => Some(None),
            MaybeUndefined::Value(comment) => Some(Some(comment.as_str())),
        };
        self.app
            .db()
            .update_period(
                &id,
                db::PeriodUpdateShape::Fields {
                    person_id: &person_id,
                    location_id: &location_id,
                    category_id: &category_id,
                    start_time,
                    end_time,
                    comment,
                },
            )
            .await?;

        let rec = self.app.db().get_periods(&[&id]).await?;
        let period = rec
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;

        self.enqueue_nitc_export(&period.id, existing.nitc_event_id.as_deref())
            .await?;
        Ok(Period::new(period))
    }

    // Also reachable with an `slp_` edit link, which authorises exactly one period.
    // Ordered `PeriodLink` first only so the failure message for everyone else stays
    // the familiar "Must provide user token".
    #[graphql(
        guard = "AuthGuard::new(AuthRequirement::PeriodLink).or(AuthGuard::new(AuthRequirement::User))"
    )]
    async fn update_period_time_category(
        &self,
        ctx: &Context<'_>,
        id: ID,
        start_time: i64,
        end_time: i64,
        category_id: ID,
        // Three-state: omit to leave the comment unchanged, pass `null` to clear
        // it, or pass a string to set it. Omitting keeps existing callers working.
        comment: MaybeUndefined<String>,
    ) -> Result<Period<A>> {
        require_writable(ctx)?;
        if start_time >= end_time {
            return Err(anyhow!("start_time must be before end_time"));
        }
        let existing = self
            .app
            .db()
            .get_periods(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;
        if existing.guest_name.is_some() {
            return Err(anyhow!("Cannot edit a guest period"));
        }
        require_period_access(ctx, &existing)?;

        // A member-facing edit link is deliberately narrower than an admin edit: it
        // corrects the times and category of its own entry, nothing else.
        let via_link = matches!(
            ctx.data_opt::<AuthInfo>(),
            Some(AuthInfo::PeriodLink { .. })
        );
        if via_link {
            if !matches!(comment, MaybeUndefined::Undefined) {
                return Err(anyhow!("An edit link cannot change the comment"));
            }
            validate_link_edit_duration(start_time, end_time)?;
        }

        let category = self
            .app
            .db()
            .get_categories(&[&category_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Category {:?} not found", category_id))?;
        // Admins may keep a retired category (it may already be the period's own);
        // a member picking from the link's list may only choose a current one.
        if via_link && !category.enabled {
            return Err(anyhow!("Category {:?} is not available", category_id));
        }

        let comment = match &comment {
            MaybeUndefined::Undefined => None,
            MaybeUndefined::Null => Some(None),
            MaybeUndefined::Value(comment) => Some(Some(comment.as_str())),
        };
        self.app
            .db()
            .update_period(
                &id,
                db::PeriodUpdateShape::TimeCategory {
                    start_time,
                    end_time,
                    category_id: &category_id,
                    // Admin edit, not a kiosk sign-out: leave the session reference untouched.
                    signed_out_session_id: None,
                    comment,
                },
            )
            .await?;

        let rec = self.app.db().get_periods(&[&id]).await?;
        let period = rec
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;

        self.enqueue_nitc_export(&period.id, existing.nitc_event_id.as_deref())
            .await?;
        Ok(Period::new(period))
    }

    /// Email the member a reminder to check one of their time entries, with a
    /// short-lived link that lets them correct it themselves.
    ///
    /// Issues a fresh `slp_` token each time, so a member who lost or expired an
    /// earlier link just needs another press. Returns the address it was sent to
    /// (useful confirmation for the admin who pressed the button).
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn send_period_edit_link(&self, ctx: &Context<'_>, id: ID) -> Result<String> {
        require_writable(ctx)?;
        let period = self
            .app
            .db()
            .get_periods(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;
        require_location_access(ctx, &period.location_id)?;
        // Checked before anything else costly, and before a token is minted, so a
        // rejected press leaves no trace.
        crate::period_link::check_reminder_cooldown(self.app.db(), &id).await?;

        let Some(person_id) = period.person_id.clone() else {
            return Err(anyhow!(
                "This entry belongs to a guest, who has no member record to email"
            ));
        };
        let person = self
            .app
            .db()
            .get_persons(&[&person_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Person with ID {person_id} missing"))?;
        let email = person
            .email
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} {} has no email address on record",
                    person.first_name,
                    person.last_name
                )
            })?
            .to_string();

        let location = self
            .app
            .db()
            .get_locations(&[&period.location_id])
            .await?
            .into_iter()
            .next()
            .flatten();
        let location_name = location
            .as_ref()
            .map(|l| l.name.as_str())
            .unwrap_or("your unit");

        let category = match &period.category_id {
            Some(category_id) => self
                .app
                .db()
                .get_categories(&[category_id])
                .await?
                .into_iter()
                .next()
                .flatten(),
            None => None,
        };

        let token = crate::period_link::issue_period_link_token(self.app.db(), &id).await?;
        let url = crate::period_link::edit_link_url(&token);
        let body = build_period_reminder_email(
            &person.first_name,
            location_name,
            category.as_ref().map(|c| c.name.as_str()),
            period.start_time,
            period.end_time,
            &url,
        );

        info!(period_id = %id.as_str(), "Sending period edit link to {}", email);
        crate::mail::send_plain_text(&email, PERIOD_REMINDER_SUBJECT, &body)
            .await
            .map_err(|e| anyhow!("Couldn't send the email: {e:#}"))?;

        // The email has already gone, so a failure to stamp the cooldown must not
        // fail the mutation — that would tell the admin it didn't send and invite a
        // second one. Worst case the cooldown doesn't apply to this send.
        if let Err(e) = crate::period_link::record_reminder_sent(self.app.db(), &id).await {
            warn!(
                "Failed to record reminder cooldown for period {}: {:#}",
                id.as_str(),
                e
            );
        }

        Ok(email)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn delete_period(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_writable(ctx)?;
        let existing = self
            .app
            .db()
            .get_periods(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;
        require_location_access(ctx, &existing.location_id)?;

        self.app
            .db()
            .update_period(&id, db::PeriodUpdateShape::Delete)
            .await?;
        self.enqueue_nitc_export(&id, existing.nitc_event_id.as_deref())
            .await?;
        Ok(true)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn create_session(
        &self,
        ctx: &Context<'_>,
        name: String,
        location_id: ID,
        config: Option<String>,
        healthcheck_url: Option<String>,
    ) -> Result<Session<A>> {
        require_writable(ctx)?;
        require_location_access(ctx, &location_id)?;
        self.app
            .db()
            .get_locations(&[&location_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Location {:?} not found", location_id))?;

        let config = parse_session_config_json(config.as_deref())?;
        let healthcheck_url = normalize_healthcheck_url(healthcheck_url.as_deref())?;
        let item = self
            .app
            .db()
            .create_session(
                &location_id,
                &name,
                &config,
                healthcheck_url.as_deref(),
                None,
            )
            .await?;

        Ok(Session::new(item))
    }

    /// Complete a public-key kiosk enrollment: create a session bound to the public key a
    /// kiosk previously published via [`submit_enrollment_key`], identified by its
    /// `key_fingerprint`. The kiosk then authenticates every request by signing it (no
    /// 6-digit code, no JWT). Reached from the admin SessionEnroll page after scanning
    /// the kiosk's QR code.
    ///
    /// Works whatever state the device's current enrollment is in — live, expired, or
    /// deleted. Any session still holding the key has it released (`key_released_at`
    /// stamped, key material dropped) before the new one takes it, so a kiosk set up as
    /// the wrong kiosk, or at the wrong location, is fixed by re-enrolling the device
    /// itself. The released record stays listed at its own location so the admins there
    /// can see the kiosk was replaced; nothing about it is reported back to the caller,
    /// who may have no access to that location.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn enroll_session(
        &self,
        ctx: &Context<'_>,
        name: String,
        location_id: ID,
        config: Option<String>,
        healthcheck_url: Option<String>,
        key_fingerprint: String,
    ) -> Result<Session<A>> {
        require_writable(ctx)?;
        require_location_access(ctx, &location_id)?;
        self.app
            .db()
            .get_locations(&[&location_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Location {:?} not found", location_id))?;

        let config = parse_session_config_json(config.as_deref())?;
        let healthcheck_url = normalize_healthcheck_url(healthcheck_url.as_deref())?;

        // Look up the pending enrollment record the kiosk published (and is keeping alive
        // by re-submitting while it displays the QR code).
        let now = crate::clock::now_sec();
        let state_id = crate::session_key::enroll_state_id(&key_fingerprint);
        let pending = self
            .app
            .db()
            .get_ephemeral_state(&state_id)
            .await?
            .filter(|s| s.kind == crate::session_key::ENROLL_STATE_KIND && s.expires_at > now)
            .ok_or_else(|| {
                anyhow!(
                    "Enrollment request not found or expired — make sure the kiosk is still showing its QR code"
                )
            })?;
        let payload: crate::session_key::EnrollPayload = serde_json::from_str(&pending.payload)
            .map_err(|e| anyhow!("Corrupt enrollment record: {e}"))?;

        // Deal with any session already holding this fingerprint. Enrollment never
        // dead-ends here: a soft-deleted holder has already given its key up, and any
        // other one — a stale record from a kiosk that sat offline past KEY_LIFETIME_S,
        // or a working kiosk being re-enrolled because it was set up as the wrong kiosk
        // or at the wrong location — has its key released so this device can enroll
        // afresh. Only one session may hold a fingerprint at a time (`auth` rejects the
        // key outright if two ever do), so this must happen before the create below.
        //
        // No location check on the old record, and nothing about it reported back: it may
        // belong to a location this admin cannot see, and requiring rights there would
        // block the very case this exists for. Releasing is safe regardless of location —
        // only the device holding the matching private key ever authenticated as that
        // record, and that device is the one in front of the admin asking to enroll. The
        // record is left in place rather than deleted, so its own location's admins can
        // see what became of it.
        let existing_ids = self
            .app
            .db()
            .get_session_id_by_key_fingerprint(&key_fingerprint)
            .await?;
        for existing in self
            .app
            .db()
            .get_sessions(&existing_ids)
            .await?
            .into_iter()
            .flatten()
        {
            let holder = crate::session_key::classify_fingerprint_holder(
                existing.active,
                existing.key_expires_at,
                now,
            );
            if !holder.needs_release() {
                continue;
            }
            match self
                .app
                .db()
                .update_session(
                    &existing.id,
                    db::SessionUpdateShape::ReleaseKey {
                        fingerprint: &key_fingerprint,
                    },
                )
                .await
            {
                Ok(()) => info!(
                    "Released {} key {} from kiosk session {} ({:?}) at location {} so its device \
                     could enroll as {:?} at location {}",
                    match holder {
                        crate::session_key::FingerprintHolder::ReleaseLive => "live",
                        _ => "expired",
                    },
                    key_fingerprint,
                    existing.id,
                    existing.name,
                    existing.location_id,
                    name,
                    *location_id
                ),
                // The conditional write failed: the row stopped holding this fingerprint
                // between our read and our write, so another enrollment of the same device
                // got there first. Nothing sensible to take over any more.
                Err(db::Error::NotFound(_)) => {
                    return Err(ApiError::new(
                        ErrorCode::Conflict,
                        "This device was enrolled by someone else while you were setting it up — rescan its QR code and try again",
                    )
                    .into());
                }
                Err(e) => return Err(e.into()),
            }
        }

        let item = self
            .app
            .db()
            .create_session(
                &location_id,
                &name,
                &config,
                healthcheck_url.as_deref(),
                Some(db::SessionKeyParams {
                    public_key: &payload.public_key,
                    fingerprint: &key_fingerprint,
                    expires_at: now + crate::session_key::KEY_LIFETIME_S,
                }),
            )
            .await?;

        // Best-effort cleanup; the record TTLs out anyway if this fails.
        if let Err(e) = self.app.db().delete_ephemeral_state(&state_id).await {
            warn!(
                "Failed to delete pending enrollment record {}: {}",
                state_id, e
            );
        }

        Ok(Session::new(item))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn create_api_token(
        &self,
        ctx: &Context<'_>,
        name: String,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<i64>,
    ) -> Result<CreateApiTokenResult> {
        require_writable(ctx)?;
        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("Super user auth required")),
        };
        if name.trim().is_empty() {
            return Err(anyhow!("name is required"));
        }
        if !location_grants.is_empty() {
            let found = self
                .app
                .db()
                .get_locations(location_grants.as_slice())
                .await?;
            for (id, loc) in location_grants.iter().zip(found.iter()) {
                if loc.is_none() {
                    return Err(anyhow!("Location {:?} not found", id));
                }
            }
        }
        let expires_at = expires_at
            .and_then(|ts| u64::try_from(ts).ok())
            .filter(|&ts| ts > 0);

        let (secret, token_hash) = auth::generate_api_token_secret();
        let rec = self
            .app
            .db()
            .create_api_token(
                &name,
                &token_hash,
                location_grants,
                read_only,
                expires_at,
                &user_id,
            )
            .await?;

        Ok(CreateApiTokenResult {
            token: ApiToken::new(rec),
            secret,
        })
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn update_api_token(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: String,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<i64>,
    ) -> Result<ApiToken> {
        require_writable(ctx)?;
        if name.trim().is_empty() {
            return Err(anyhow!("name is required"));
        }
        if !location_grants.is_empty() {
            let found = self
                .app
                .db()
                .get_locations(location_grants.as_slice())
                .await?;
            for (id, loc) in location_grants.iter().zip(found.iter()) {
                if loc.is_none() {
                    return Err(anyhow!("Location {:?} not found", id));
                }
            }
        }
        let expires_at = expires_at
            .and_then(|ts| u64::try_from(ts).ok())
            .filter(|&ts| ts > 0);

        self.app
            .db()
            .update_api_token(
                &id,
                db::ApiTokenUpdateShape::Fields {
                    name: &name,
                    location_grants,
                    read_only,
                    expires_at,
                },
            )
            .await?;

        let rec = self
            .app
            .db()
            .get_api_token(&id)
            .await?
            .ok_or_else(|| anyhow!("ApiToken with ID {:?} missing", id))?;
        Ok(ApiToken::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn revoke_api_token(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_writable(ctx)?;
        self.app
            .db()
            .update_api_token(&id, db::ApiTokenUpdateShape::Revoke)
            .await?;
        Ok(true)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn update_session(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: String,
        config: Option<String>,
        healthcheck_url: Option<String>,
    ) -> Result<Session<A>> {
        require_writable(ctx)?;
        let existing = self
            .app
            .db()
            .get_sessions(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Session with ID {:?} missing", id))?;
        require_location_access(ctx, &existing.location_id)?;

        let config = parse_session_config_json(config.as_deref())?;
        let healthcheck_url = normalize_healthcheck_url(healthcheck_url.as_deref())?;
        self.app
            .db()
            .update_session(
                &id,
                db::SessionUpdateShape::Fields {
                    name: &name,
                    config: &config,
                    healthcheck_url: healthcheck_url.as_deref(),
                },
            )
            .await?;

        let rec = self.app.db().get_sessions(&[&id]).await?;
        Ok(Session::new(rec.into_iter().next().flatten().ok_or_else(
            || anyhow!("Session with ID {:?} missing", id),
        )?))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn delete_session(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        require_writable(ctx)?;
        let existing = self
            .app
            .db()
            .get_sessions(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Session with ID {:?} missing", id))?;
        require_location_access(ctx, &existing.location_id)?;

        self.app
            .db()
            .update_session(&id, db::SessionUpdateShape::Delete)
            .await?;
        Ok(true)
    }

    /// Bring back a QR-enrolled kiosk that expired after two weeks offline, without
    /// deleting and re-enrolling it — it keeps its name, config and healthcheck URL.
    /// Kiosks set up with a 6-digit code cannot be reactivated.
    ///
    /// Only succeeds while the kiosk is switched on and showing its enrollment QR screen,
    /// and only grants a short window: the kiosk must come back with one signed request,
    /// which restores its normal two-week window. Otherwise the grant simply lapses.
    // Implementation: the "switched on" test is a live pending-enrollment record (the
    // kiosk republishes its key every 10 min), and the short window is
    // session_key::REACTIVATE_GRACE_S, widened back to KEY_LIFETIME_S by
    // auth::touch_session on the redeeming request.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn reactivate_session(&self, ctx: &Context<'_>, id: ID) -> Result<Session<A>> {
        require_writable(ctx)?;
        let existing = self
            .app
            .db()
            .get_sessions(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Session with ID {:?} missing", id))?;
        require_location_access(ctx, &existing.location_id)?;

        if !existing.active {
            return Err(anyhow!("This kiosk has been deleted."));
        }
        // A record whose key was released has been stripped like a code-enrolled one, so
        // answer it before the message below, which would be wrong about why.
        if existing.key_released_at.is_some() {
            return Err(anyhow!(
                "This kiosk's computer has since been set up again by scanning its QR code, so this entry can't be brought back. Delete it, or set the computer up again here."
            ));
        }
        // Code-enrolled kiosks have nothing to revive: their single-use code was wiped the
        // first time it was entered.
        let (Some(key_fingerprint), Some(_)) = (
            existing.key_fingerprint.as_deref(),
            existing.public_key.as_deref(),
        ) else {
            return Err(anyhow!(
                "This kiosk was set up with a setup code, so it can't be reactivated — delete it and set it up again."
            ));
        };

        // The kiosk republishes its key every 10 minutes while it sits on the enrollment
        // screen, so a live record means the device is switched on and asking to come back.
        let now = crate::clock::now_sec();
        self.app
            .db()
            .get_ephemeral_state(&crate::session_key::enroll_state_id(key_fingerprint))
            .await?
            .filter(|s| s.kind == crate::session_key::ENROLL_STATE_KIND && s.expires_at > now)
            .ok_or_else(|| {
                anyhow!(
                    "The kiosk isn't currently asking to be re-enrolled — switch it on, wait for the QR code screen, then try again."
                )
            })?;

        let expires_at = crate::session_key::reactivated_key_expiry(existing.key_expires_at, now);
        self.app
            .db()
            .update_session(&id, db::SessionUpdateShape::ExtendKey { expires_at })
            .await?;
        info!(
            "Reactivated kiosk session {} at location {} until {}",
            existing.id, existing.location_id, expires_at
        );

        let rec = self.app.db().get_sessions(&[&id]).await?;
        Ok(Session::new(rec.into_iter().next().flatten().ok_or_else(
            || anyhow!("Session with ID {:?} missing", id),
        )?))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn create_location(
        &self,
        name: String,
        nitc_enabled: Option<i64>,
    ) -> Result<Location<A>> {
        let nitc_enabled = nitc_enabled
            .and_then(|ts| u64::try_from(ts).ok())
            .filter(|&ts| ts > 0);
        let rec = self
            .app
            .db()
            .create_location(&name, nitc_enabled, None)
            .await?;

        Ok(Location::new_db(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn update_location(
        &self,
        id: ID,
        name: String,
        enabled: bool,
        nitc_enabled: Option<i64>,
        // Omitting nitc_complete_on_export leaves the location's current preference unchanged,
        // so a client that doesn't know about the field can't silently reset it.
        nitc_complete_on_export: Option<bool>,
    ) -> Result<Location<A>> {
        let nitc_enabled = nitc_enabled
            .and_then(|ts| u64::try_from(ts).ok())
            .filter(|&ts| ts > 0);
        self.app
            .db()
            .update_location(
                &id,
                db::LocationUpdateShape::Fields {
                    name: &name,
                    enabled,
                    nitc_enabled,
                    nitc_complete_on_export,
                },
            )
            .await?;

        let rec = self.app.db().get_locations(&[&id]).await?;
        Ok(Location::new_db(
            rec.into_iter()
                .next()
                .flatten()
                .ok_or_else(|| anyhow!("Location with ID {:?} missing", id))?,
        ))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn create_category(
        &self,
        name: String,
        is_virtual: bool,
        nitc_group_id: Option<String>,
        nitc_participant_type: Option<String>,
    ) -> Result<Category<A>> {
        let nitc_group_id = nitc_group_id.as_deref().filter(|s| !s.is_empty());
        let nitc_participant_type = nitc_participant_type.as_deref().filter(|s| !s.is_empty());
        let item = self
            .app
            .db()
            .create_category(&name, is_virtual, nitc_group_id, nitc_participant_type)
            .await?;
        Ok(Category::new(item))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn update_category(
        &self,
        id: ID,
        name: String,
        enabled: bool,
        is_virtual: bool,
        nitc_group_id: Option<String>,
        nitc_participant_type: Option<String>,
    ) -> Result<Category<A>> {
        let nitc_group_id = nitc_group_id.as_deref().filter(|s| !s.is_empty());
        let nitc_participant_type = nitc_participant_type.as_deref().filter(|s| !s.is_empty());
        self.app
            .db()
            .update_category(
                &id,
                &name,
                enabled,
                is_virtual,
                nitc_group_id,
                nitc_participant_type,
            )
            .await?;

        let rec = self.app.db().get_categories(&[&id]).await?;
        Ok(Category::new(rec.into_iter().next().flatten().ok_or_else(
            || anyhow!("Category with ID {:?} missing", id),
        )?))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn create_nitc_group(
        &self,
        id: Option<String>,
        nitc_type: String,
        nitc_tag_ids: Vec<i32>,
    ) -> Result<NitcGroup<A>> {
        let id_ref = id.as_deref().filter(|s| !s.is_empty());
        let rec = self
            .app
            .db()
            .create_nitc_group(id_ref, &nitc_type, &nitc_tag_ids)
            .await?;
        Ok(NitcGroup::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn update_nitc_group(
        &self,
        id: ID,
        nitc_type: String,
        nitc_tag_ids: Vec<i32>,
    ) -> Result<NitcGroup<A>> {
        self.app
            .db()
            .update_nitc_group(&id, &nitc_type, &nitc_tag_ids)
            .await?;
        let rec = self
            .app
            .db()
            .get_nitc_group(&id)
            .await?
            .ok_or_else(|| anyhow!("NitcGroup with ID {:?} missing", id))?;
        Ok(NitcGroup::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::SuperUser)")]
    async fn delete_nitc_group(&self, id: ID) -> Result<bool> {
        self.app.db().delete_nitc_group(&id).await?;
        Ok(true)
    }

    // scan functions
    #[graphql(guard = "AuthGuard::new(AuthRequirement::Session)")]
    async fn scan_register2(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "memberNumber")] registration_number: String,
        #[graphql(
            desc = "Also compute the sign-out quick-pick shortcuts (see RegisterResult.quickPick). \
                    Opt-in: it costs extra reads, and a kiosk with the quick-pick screen turned \
                    off has no use for them."
        )]
        quick_pick: Option<bool>,
    ) -> Result<RegisterResult<A>> {
        require_writable(ctx)?;

        let registration_number = registration_number.trim();
        if registration_number.is_empty() {
            return Err(anyhow!("registration_number cannot be empty"));
        }

        let auth = ctx.data_opt::<AuthInfo>();
        let (session_id, location_id) = match auth {
            Some(AuthInfo::Session { id, location }) => (id, location),
            _ => {
                return Err(anyhow!("Cannot call scan_register2 without session auth"));
            }
        };

        let matches = self
            .app
            .db()
            .get_person_id_by_registration_number(registration_number)
            .await?;
        let Some(person_id) = db::at_most_one(matches, || {
            format!("Multiple people share registration number {registration_number}")
        })?
        else {
            return Ok(RegisterResult {
                state: RegisterState::NotFound,
                period: None,
                quick_pick: None,
            });
        };

        // lookup most recent unfinished period for this person scoped to this session's location
        let existing_unfinished_period = self
            .app
            .db()
            .list_periods_for_person(
                &person_id,
                Some(location_id),
                Some(true),
                None,
                db::ListPeriodsPage {
                    after: None,
                    before: None,
                    limit: 10,
                    descending: true,
                },
            )
            .await?
            .into_iter()
            .next();

        if let Some(period) = existing_unfinished_period {
            // The person is signed in *here*, which is the whole authorization for the
            // quick-pick: it is only ever built for someone standing at this kiosk, so
            // their own history is fair game even when they belong to another unit.
            let quick_pick = if quick_pick.unwrap_or(false) {
                match build_quick_pick(self.app.as_ref(), location_id, &person_id).await {
                    Ok(qp) => Some(qp),
                    // Never fail a sign-out over a shortcut list. The kiosk treats a null
                    // quick-pick as "nothing to suggest" and shows the full category tree.
                    Err(e) => {
                        warn!("failed to build quick pick for person {person_id}: {e:?}");
                        None
                    }
                }
            } else {
                None
            };

            // already signed in — return pending state without modifying the period
            Ok(RegisterResult {
                state: RegisterState::SignOutPending,
                period: Some(Period::new(period)),
                quick_pick,
            })
        } else {
            // no existing unfinished period, so sign them in
            let rec = self
                .app
                .db()
                .start_period_for_person_location(&person_id, location_id, session_id)
                .await?;

            Ok(RegisterResult {
                state: RegisterState::SignedIn,
                period: Some(Period::new(rec)),
                quick_pick: None,
            })
        }
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::Session)")]
    async fn scan_sign_out(
        &self,
        ctx: &Context<'_>,
        id: ID,
        start_time: i64,
        end_time: i64,
        category_id: ID,
    ) -> Result<Period<A>> {
        require_writable(ctx)?;
        if start_time >= end_time {
            // Surfaced verbatim on the kiosk transaction log, so keep it readable.
            return Err(anyhow!("start time must be before end time"));
        }
        let session_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::Session { id, .. }) => id.clone(),
            _ => return Err(anyhow!("Cannot call scan_sign_out without session auth")),
        };
        let rec = self.app.db().get_periods(&[&id]).await?;
        let mut rec = rec
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;
        require_location_access(ctx, &rec.location_id)?;
        self.app
            .db()
            .get_categories(&[&category_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Category {:?} not found", category_id))?;
        self.app
            .db()
            .update_period(
                &rec.id,
                db::PeriodUpdateShape::TimeCategory {
                    start_time,
                    end_time,
                    category_id: &category_id,
                    signed_out_session_id: Some(&session_id),
                    // Kiosk sign-out: leave any existing comment untouched.
                    comment: None,
                },
            )
            .await?;
        rec.start_time = start_time as u64;
        rec.end_time = Some(end_time as u64);
        rec.category_id = Some(category_id.to_string());
        rec.signed_out_session_id = Some(session_id);

        // rec is the pre-update record (only local field copies were changed above), so
        // rec.nitc_event_id is still the event the period was assigned to before this sign-out.
        self.enqueue_nitc_export(&rec.id, rec.nitc_event_id.as_deref())
            .await?;
        Ok(Period::new(rec))
    }

    /// Sign in a guest (non-member) at the kiosk. Creates an open period with no
    /// person and no category, so it never enters per-person views or the NITC export.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::Session)")]
    async fn scan_guest_sign_in(
        &self,
        ctx: &Context<'_>,
        name: String,
        reason: Option<String>,
    ) -> Result<Period<A>> {
        require_writable(ctx)?;

        let (session_id, location_id) = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::Session { id, location }) => (id, location),
            _ => {
                return Err(anyhow!(
                    "Cannot call scan_guest_sign_in without session auth"
                ));
            }
        };

        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("Guest name is required"));
        }
        if name.chars().count() > 100 {
            return Err(anyhow!("Guest name is too long"));
        }
        let reason = reason.as_deref().map(str::trim).filter(|r| !r.is_empty());
        if let Some(reason) = reason
            && reason.chars().count() > 500
        {
            return Err(anyhow!("Reason is too long"));
        }

        let rec = self
            .app
            .db()
            .start_guest_period(location_id, name, reason, session_id)
            .await?;
        Ok(Period::new(rec))
    }

    /// Sign out a guest period from the kiosk. Only closes guest periods (no person,
    /// no category), so it can never be used to close a member period.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::Session)")]
    async fn scan_guest_sign_out(&self, ctx: &Context<'_>, id: ID) -> Result<Period<A>> {
        require_writable(ctx)?;

        let session_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::Session { id, .. }) => id.clone(),
            _ => {
                return Err(anyhow!(
                    "Cannot call scan_guest_sign_out without session auth"
                ));
            }
        };

        let rec = self
            .app
            .db()
            .get_periods(&[&id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Period with ID {:?} missing", id))?;

        require_location_access(ctx, &rec.location_id)?;
        if rec.guest_name.is_none() {
            return Err(anyhow!("Not a guest period"));
        }
        if rec.deleted.is_some() {
            return Err(anyhow!("Guest period has been deleted"));
        }
        if rec.end_time.is_some() {
            return Err(anyhow!("Guest already signed out"));
        }

        let updated = self.app.db().end_period(&rec, Some(&session_id)).await?;
        Ok(Period::new(updated))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn enqueue_member_sync(&self, ctx: &Context<'_>, location_id: ID) -> Result<bool> {
        require_writable(ctx)?;
        require_location_access(ctx, &location_id)?;
        self.app
            .db()
            .get_locations(&[&location_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("Location {:?} not found", location_id))?;
        let sqs = &self.app.sqs().member_sync;
        crate::sqs_dispatch::enqueue_location_sync(&sqs.client, &sqs.queue_url, &location_id)
            .await?;
        Ok(true)
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn update_my_email_config(
        &self,
        ctx: &Context<'_>,
        daily_location_ids: Vec<String>,
    ) -> Result<User<A>> {
        require_writable(ctx)?;
        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("User auth required")),
        };

        let mut email_config = serde_json::Map::new();
        for loc_id in daily_location_ids {
            // Only allow configuring summaries for locations the caller can access,
            // otherwise this becomes a push channel for cross-tenant data.
            require_location_access(ctx, &loc_id)?;
            let mut inner = serde_json::Map::new();
            inner.insert(
                "daily".to_string(),
                serde_json::Value::String("1".to_string()),
            );
            email_config.insert(loc_id, serde_json::Value::Object(inner));
        }

        self.app
            .db()
            .update_user(&user_id, db::UserUpdateShape::EmailConfig { email_config })
            .await?;

        let rec = self
            .app
            .db()
            .get_users(&[&user_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("User missing after update"))?;
        Ok(User::new(rec))
    }

    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn update_my_disaggregate_virtual_periods(
        &self,
        ctx: &Context<'_>,
        value: bool,
    ) -> Result<User<A>> {
        require_writable(ctx)?;
        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("User auth required")),
        };

        self.app
            .db()
            .update_user(
                &user_id,
                db::UserUpdateShape::DisaggregateVirtualPeriods { value },
            )
            .await?;

        let rec = self
            .app
            .db()
            .get_users(&[&user_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| anyhow!("User missing after update"))?;
        Ok(User::new(rec))
    }

    // ── Passkey (WebAuthn) mutations ─────────────────────────────────────────

    /// Start passkey registration for the authenticated user.
    /// Returns a JSON challenge to pass to the browser's WebAuthn API.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn begin_passkey_registration(&self, ctx: &Context<'_>) -> Result<PasskeyChallenge> {
        use webauthn_rs::prelude::*;

        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("Not authenticated")),
        };

        let count = self
            .app
            .db()
            .count_webauthn_credentials_by_user(&user_id)
            .await?;
        if count >= 10 {
            return Err(anyhow!("Maximum of 10 passkeys allowed"));
        }

        let existing = self
            .app
            .db()
            .list_webauthn_credentials_by_user(&user_id)
            .await?;

        let webauthn = ctx.data_unchecked::<Arc<Webauthn>>();

        // The user handle stays tied to the (immutable) user id so a passkey
        // keeps working if the user's email changes. Only the display name —
        // what the OS/password manager shows — uses the email.
        let user_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, user_id.as_bytes());
        let display_name = self
            .app
            .db()
            .get_users(&[&user_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .map(|u| u.email)
            .unwrap_or_else(|| user_id.clone());

        let existing_cred_ids: Vec<CredentialID> = existing
            .iter()
            .filter_map(|c| {
                serde_json::from_str::<Passkey>(&c.passkey_json)
                    .ok()
                    .map(|pk| pk.cred_id().clone())
            })
            .collect();

        let exclude = if existing_cred_ids.is_empty() {
            None
        } else {
            Some(existing_cred_ids)
        };

        let (ccr, reg_state) = webauthn.start_passkey_registration(
            user_uuid,
            &display_name,
            &display_name,
            exclude,
        )?;

        // Force the credential to be discoverable (a resident key). webauthn-rs
        // 0.4 only emits the legacy `requireResidentKey: false` and no modern
        // `residentKey` field, so platform authenticators make it discoverable
        // but security keys may not — and a non-discoverable credential can't be
        // used by our usernameless login. Inject `residentKey: "required"` into
        // the options before handing them to the browser. (finish_* doesn't
        // validate residency, so there's no verification mismatch.)
        let mut options_value = serde_json::to_value(&ccr.public_key)
            .map_err(|e| anyhow!("Failed to serialize registration options: {}", e))?;
        if let Some(sel) = options_value
            .get_mut("authenticatorSelection")
            .and_then(|v| v.as_object_mut())
        {
            sel.insert("residentKey".to_string(), serde_json::json!("required"));
            sel.insert("requireResidentKey".to_string(), serde_json::json!(true));
        }
        let options_json = serde_json::to_string(&options_value)
            .map_err(|e| anyhow!("Failed to serialize registration options: {}", e))?;
        let state_json = serde_json::to_string(&reg_state)
            .map_err(|e| anyhow!("Failed to serialize registration state: {}", e))?;

        let challenge_id = nanoid::nanoid!(32);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = now + 5 * 60;

        self.app
            .db()
            .put_webauthn_state(
                &challenge_id,
                "reg",
                Some(&user_id),
                &state_json,
                expires_at,
            )
            .await?;

        Ok(PasskeyChallenge {
            challenge_id,
            options_json,
        })
    }

    /// Finish passkey registration: verify the browser response and store the credential.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn finish_passkey_registration(
        &self,
        ctx: &Context<'_>,
        challenge_id: String,
        credential_json: String,
        name: String,
    ) -> Result<PasskeyInfo> {
        use webauthn_rs::prelude::*;

        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("Not authenticated")),
        };

        let state_record = self
            .app
            .db()
            .get_webauthn_state(&challenge_id)
            .await?
            .ok_or_else(|| anyhow!("Registration challenge not found or expired"))?;

        if state_record.kind != "reg" {
            return Err(anyhow!("Invalid challenge kind"));
        }
        if state_record.user_id.as_deref() != Some(&user_id) {
            return Err(anyhow!("Challenge belongs to a different user"));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now >= state_record.expires_at {
            let _ = self.app.db().delete_webauthn_state(&challenge_id).await;
            return Err(anyhow!("Registration challenge expired"));
        }

        let reg_state: PasskeyRegistration = serde_json::from_str(&state_record.state_json)
            .map_err(|e| anyhow!("Failed to deserialize registration state: {}", e))?;

        let reg_credential: RegisterPublicKeyCredential = serde_json::from_str(&credential_json)
            .map_err(|e| anyhow!("Failed to parse credential: {}", e))?;

        let webauthn = ctx.data_unchecked::<Arc<Webauthn>>();
        let passkey = webauthn
            .finish_passkey_registration(&reg_credential, &reg_state)
            .map_err(|e| anyhow!("Passkey registration failed: {}", e))?;

        // Re-check cap to guard against races
        let count = self
            .app
            .db()
            .count_webauthn_credentials_by_user(&user_id)
            .await?;
        if count >= 10 {
            let _ = self.app.db().delete_webauthn_state(&challenge_id).await;
            return Err(anyhow!("Maximum of 10 passkeys allowed"));
        }

        let cred_id = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());
        let passkey_json = serde_json::to_string(&passkey)
            .map_err(|e| anyhow!("Failed to serialize passkey: {}", e))?;

        let cred = self
            .app
            .db()
            .create_webauthn_credential(&cred_id, &user_id, &name, &passkey_json)
            .await?;

        let _ = self.app.db().delete_webauthn_state(&challenge_id).await;

        info!(
            "Passkey registered for user_id={} cred_id={}",
            user_id, cred_id
        );

        Ok(PasskeyInfo {
            id: cred.id,
            name: cred.name,
            created_at: cred.created_at as i64,
            last_used_at: None,
        })
    }

    /// Start a discoverable passkey login (no username required).
    /// Returns a JSON challenge to pass to the browser's WebAuthn API.
    async fn begin_passkey_login(&self, ctx: &Context<'_>) -> Result<PasskeyChallenge> {
        use webauthn_rs::prelude::*;

        let webauthn = ctx.data_unchecked::<Arc<Webauthn>>();
        let (rcr, auth_state) = webauthn
            .start_discoverable_authentication()
            .map_err(|e| anyhow!("Failed to start passkey login: {}", e))?;

        let options_json = serde_json::to_string(&rcr.public_key)
            .map_err(|e| anyhow!("Failed to serialize login options: {}", e))?;
        let state_json = serde_json::to_string(&auth_state)
            .map_err(|e| anyhow!("Failed to serialize auth state: {}", e))?;

        let challenge_id = nanoid::nanoid!(32);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = now + 5 * 60;

        self.app
            .db()
            .put_webauthn_state(&challenge_id, "auth", None, &state_json, expires_at)
            .await?;

        Ok(PasskeyChallenge {
            challenge_id,
            options_json,
        })
    }

    /// Finish passkey login: verify the browser response and return an opaque session token.
    async fn finish_passkey_login(
        &self,
        ctx: &Context<'_>,
        challenge_id: String,
        credential_json: String,
    ) -> Result<Option<String>> {
        use webauthn_rs::prelude::*;

        let state_record = self
            .app
            .db()
            .get_webauthn_state(&challenge_id)
            .await?
            .ok_or_else(|| anyhow!("Login challenge not found or expired"))?;

        if state_record.kind != "auth" {
            return Err(anyhow!("Invalid challenge kind"));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if now >= state_record.expires_at {
            let _ = self.app.db().delete_webauthn_state(&challenge_id).await;
            return Ok(None);
        }

        let auth_state: DiscoverableAuthentication = serde_json::from_str(&state_record.state_json)
            .map_err(|e| anyhow!("Failed to deserialize auth state: {}", e))?;

        let auth_credential: PublicKeyCredential = serde_json::from_str(&credential_json)
            .map_err(|_| anyhow!("Failed to parse credential"))?;

        let webauthn = ctx.data_unchecked::<Arc<Webauthn>>();
        let (_user_handle, cred_id_bytes) = webauthn
            .identify_discoverable_authentication(&auth_credential)
            .map_err(|e| anyhow!("Failed to identify credential: {}", e))?;

        let cred_id_str = URL_SAFE_NO_PAD.encode(cred_id_bytes);
        let stored = match self.app.db().get_webauthn_credential(&cred_id_str).await? {
            Some(c) => c,
            None => {
                info!("finish_passkey_login: unknown credential {}", cred_id_str);
                let _ = self.app.db().delete_webauthn_state(&challenge_id).await;
                return Ok(None);
            }
        };

        let mut passkey: Passkey = serde_json::from_str(&stored.passkey_json)
            .map_err(|e| anyhow!("Failed to deserialize stored passkey: {}", e))?;

        let auth_result = webauthn
            .finish_discoverable_authentication(
                &auth_credential,
                auth_state,
                &[DiscoverableKey::from(&passkey)],
            )
            .map_err(|e| anyhow!("Passkey authentication failed: {}", e))?;

        // Always record last_used_at on a successful login. The counter bump is
        // conditional (needs_update() only fires when the signature counter
        // advanced), but most platform/synced passkeys keep the counter at 0 and
        // never report needs_update(), so gating the whole write on it would
        // leave last_used_at perpetually unset.
        if auth_result.needs_update() {
            passkey.update_credential(&auth_result);
        }
        let updated_json = serde_json::to_string(&passkey)
            .map_err(|e| anyhow!("Failed to serialize updated passkey: {}", e))?;
        let _ = self
            .app
            .db()
            .update_webauthn_credential(
                &cred_id_str,
                db::WebauthnCredentialUpdate::TouchLastUsed {
                    passkey_json: updated_json,
                },
            )
            .await;

        let _ = self.app.db().delete_webauthn_state(&challenge_id).await;

        match self
            .app
            .db()
            .get_users(&[&stored.user_id])
            .await?
            .into_iter()
            .next()
            .flatten()
        {
            Some(user) if user.enabled => {}
            _ => {
                info!(
                    "finish_passkey_login: user disabled or missing id={}",
                    stored.user_id
                );
                return Ok(None);
            }
        }

        let token = auth::issue_user_token(&*self.app, &stored.user_id).await?;
        info!(
            "Passkey login for user_id={} cred_id={}",
            stored.user_id, cred_id_str
        );
        Ok(Some(token))
    }

    /// Rename one of the authenticated user's passkeys.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn rename_passkey(
        &self,
        ctx: &Context<'_>,
        id: String,
        name: String,
    ) -> Result<PasskeyInfo> {
        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("Not authenticated")),
        };

        let cred = self
            .app
            .db()
            .get_webauthn_credential(&id)
            .await?
            .ok_or_else(|| anyhow!("Passkey not found"))?;

        if cred.user_id != user_id {
            return Err(anyhow!("Passkey not found"));
        }

        let trimmed = name.trim().to_string();
        if trimmed.is_empty() {
            return Err(anyhow!("Name cannot be empty"));
        }

        self.app
            .db()
            .update_webauthn_credential(&id, db::WebauthnCredentialUpdate::Rename(trimmed.clone()))
            .await?;

        Ok(PasskeyInfo {
            id: cred.id,
            name: trimmed,
            created_at: cred.created_at as i64,
            last_used_at: cred.last_used_at.map(|t| t as i64),
        })
    }

    /// Delete one of the authenticated user's passkeys.
    #[graphql(guard = "AuthGuard::new(AuthRequirement::User)")]
    async fn delete_passkey(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_id = match ctx.data_opt::<AuthInfo>() {
            Some(AuthInfo::User { id, .. }) => id.clone(),
            _ => return Err(anyhow!("Not authenticated")),
        };

        let cred = self
            .app
            .db()
            .get_webauthn_credential(&id)
            .await?
            .ok_or_else(|| anyhow!("Passkey not found"))?;

        if cred.user_id != user_id {
            return Err(anyhow!("Passkey not found"));
        }

        self.app.db().delete_webauthn_credential(&id).await?;
        Ok(true)
    }
}

#[derive(async_graphql::SimpleObject)]
struct PasskeyChallenge {
    challenge_id: String,
    options_json: String,
}

#[cfg(test)]
mod tests {
    // Sanitized fixture captured from a real 0.4.x Passkey serialization. Key bytes are zeroed.
    // This test exists to catch webauthn-rs serde format changes during library upgrades — if
    // deserialization breaks here after a version bump, stored passkeys in DynamoDB are at risk.
    const PASSKEY_JSON_V0_4: &str = r#"{"cred":{"cred_id":"AAAAAAAAAAAAAAAAAAAAAAAAAAAA","cred":{"type_":"ES256","key":{"EC_EC2":{"curve":"SECP256R1","x":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","y":"BAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}}},"counter":0,"transports":null,"user_verified":true,"backup_eligible":true,"backup_state":true,"registration_policy":"preferred","extensions":{"cred_protect":"NotRequested","hmac_create_secret":"NotRequested","appid":"NotRequested","cred_props":"Ignored"},"attestation":{"data":"None","metadata":"None"},"attestation_format":"None"}}"#;

    #[test]
    fn passkey_json_round_trips() {
        use webauthn_rs::prelude::Passkey;
        let passkey: Passkey = serde_json::from_str(PASSKEY_JSON_V0_4).expect(
            "stored passkey JSON must deserialize — format changed after webauthn-rs upgrade?",
        );
        let reserialized =
            serde_json::to_string(&passkey).expect("passkey must reserialize to JSON");
        let reparsed: Passkey =
            serde_json::from_str(&reserialized).expect("reserialized passkey must round-trip");
        let rereserialized =
            serde_json::to_string(&reparsed).expect("reparsed passkey must reserialize");
        assert_eq!(
            reserialized, rereserialized,
            "passkey JSON must be stable across serde round trips"
        );
    }

    mod link_edit_duration {
        use super::super::{LINK_EDIT_MAX_DURATION_S, validate_link_edit_duration};

        #[test]
        fn accepts_a_normal_shift() {
            assert!(validate_link_edit_duration(1_000, 1_000 + 3 * 3600).is_ok());
        }

        #[test]
        fn accepts_exactly_the_limit() {
            assert!(validate_link_edit_duration(1_000, 1_000 + LINK_EDIT_MAX_DURATION_S).is_ok());
        }

        #[test]
        fn rejects_one_second_over_the_limit() {
            assert!(
                validate_link_edit_duration(1_000, 1_000 + LINK_EDIT_MAX_DURATION_S + 1).is_err()
            );
        }

        #[test]
        fn rejects_a_wildly_long_entry() {
            // A member dragging the end date out by a year is the case this exists for.
            assert!(validate_link_edit_duration(0, 365 * 24 * 3600).is_err());
        }
    }

    mod period_reminder_email {
        use super::super::build_period_reminder_email;

        // 2026-07-24 09:53 and 09:55 Sydney time.
        const START: u64 = 1784850780;
        const END: u64 = 1784850959;

        fn body(first_name: &str, category: Option<&str>, end: Option<u64>) -> String {
            build_period_reminder_email(
                first_name,
                "Test Unit",
                category,
                START,
                end,
                "https://new.seslogin.com/period#slp_abc",
            )
        }

        #[test]
        fn includes_the_link_and_the_entry_details() {
            let out = body("Sam", Some("Training"), Some(END));
            assert!(out.contains("Hi Sam,"));
            assert!(out.contains("Test Unit"));
            assert!(out.contains("Activity: Training"));
            assert!(out.contains("https://new.seslogin.com/period#slp_abc"));
            assert!(out.contains("48 hours"));
        }

        #[test]
        fn formats_times_in_sydney_local_time() {
            let out = body("Sam", None, Some(END));
            assert!(out.contains("Fri 24 Jul 2026, 09:53"), "got: {out}");
            assert!(out.contains("Fri 24 Jul 2026, 09:55"), "got: {out}");
        }

        #[test]
        fn falls_back_to_a_neutral_greeting_without_a_name() {
            // Person.first_name is `""` rather than null when missing.
            assert!(body("", None, Some(END)).starts_with("Hello,"));
            assert!(body("   ", None, Some(END)).starts_with("Hello,"));
        }

        #[test]
        fn omits_the_activity_line_when_uncategorised() {
            assert!(!body("Sam", None, Some(END)).contains("Activity:"));
        }

        #[test]
        fn explains_an_entry_that_is_still_open() {
            let out = body("Sam", None, None);
            assert!(out.contains("Finished: still signed in"), "got: {out}");
        }
    }
}
