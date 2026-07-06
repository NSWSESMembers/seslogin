use anyhow::{Context, Result, anyhow};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct SesPerson {
    pub id: Option<i64>,
    #[serde(rename = "registrationNumber")]
    pub registration_number: Option<String>,
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    #[serde(rename = "fullName")]
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub deleted: Option<bool>,
    pub headquarters: Option<SesPersonHeadquarters>,
}

impl SesPerson {
    pub fn headquarters_id(&self) -> Option<i64> {
        self.headquarters.as_ref().and_then(|h| h.id)
    }
}

impl fmt::Display for SesPerson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let first = self
            .first_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("<null>");
        let last = self
            .last_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("<null>");
        let registration_number = self
            .registration_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("<null>");
        let id = self
            .id
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<null>".to_string());

        write!(f, "{} {} ({}, {})", first, last, registration_number, id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesPersonHeadquarters {
    pub id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesHeadquarters {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    #[serde(rename = "type")]
    pub headquarters_type: Option<String>,
    pub status: Option<String>,
    pub zone: Option<Box<SesHeadquarters>>,
}

impl fmt::Display for SesHeadquarters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("<null>");
        let code = self
            .code
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("<null>");
        let id = self
            .id
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<null>".to_string());

        write!(f, "{} ({}, {})", name, code, id)
    }
}

// ── NITC types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SesNonIncident {
    pub id: i64,
    pub completed: Option<bool>,
    #[serde(default)]
    pub participants: Vec<SesParticipant>,
}

/// Body for POST /headquarters/{hq_id}/nitc — creates a new (empty-ish) event
#[derive(Debug, Clone, Serialize)]
pub struct SesNonIncidentCreate {
    pub name: String,
    pub description: String,
    pub nitc_type: String,
    pub location: String,
    pub start_date: String,
    pub end_date: String,
    pub tags: Vec<SesTagRef>,
}

/// Body for PUT /headquarters/{hq_id}/nitc — updates an event and its participants.
#[derive(Debug, Clone, Serialize)]
pub struct SesNonIncidentUpdate {
    pub id: i64,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub nitc_type: String,
    pub location: String,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub participants: Vec<SesParticipantUpsert>,
    pub tags: Vec<SesTagRef>,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SesParticipant {
    pub id: i64,
    pub person: Option<SesPersonRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SesParticipantUpsert {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(rename = "type")]
    pub participant_type: String,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    pub person: SesPersonRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SesPersonRef {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SesTagRef {
    pub id: i32,
    pub name: String,
}

impl SesTagRef {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            name: "Training".to_string(),
        }
    }
}

// ── Reference data types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct NonIncidentTagGroup {
    #[serde(rename = "primaryActivity")]
    primary_activity: NonIncidentTagPrimaryActivity,
    tags: Vec<NonIncidentTagItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct NonIncidentTagPrimaryActivity {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct NonIncidentTagItem {
    id: i32,
    name: String,
}

#[derive(Debug, Clone)]
pub struct SesNonIncidentTag {
    pub id: i32,
    pub name: String,
    pub primary_activity_name: String,
}

// ── Instance-wide 5-minute cache for SES reference data ──────────────────────

struct CacheEntry<T> {
    value: Arc<T>,
    fetched_at: Instant,
}

impl<T> CacheEntry<T> {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < Duration::from_secs(300)
    }
}

struct SesApiCache {
    headquarters: Mutex<Option<CacheEntry<Vec<SesHeadquarters>>>>,
    nonincident_types: Mutex<Option<CacheEntry<Vec<String>>>>,
    nonincident_tags: Mutex<Option<CacheEntry<HashMap<i32, SesNonIncidentTag>>>>,
    participant_types: Mutex<Option<CacheEntry<Vec<String>>>>,
}

static SES_CACHE: LazyLock<SesApiCache> = LazyLock::new(|| SesApiCache {
    headquarters: Mutex::new(None),
    nonincident_types: Mutex::new(None),
    nonincident_tags: Mutex::new(None),
    participant_types: Mutex::new(None),
});

// ─────────────────────────────────────────────────────────────────────────────

pub struct SesClient {
    client: Client,
    base_url: String,
    api_key: String,
    page_limit: usize,
    max_retries: usize,
}

impl SesClient {
    pub fn new(
        base_url: String,
        api_key: String,
        page_limit: usize,
        max_retries: usize,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("Building SES HTTP client")?;

        Ok(Self {
            client,
            base_url,
            api_key,
            page_limit,
            max_retries,
        })
    }

    pub async fn fetch_people_for_headquarters(
        &self,
        headquarters_id: &str,
    ) -> Result<Vec<SesPerson>> {
        let mut offset = 0usize;
        let mut all = Vec::new();

        loop {
            let page = self
                .fetch_people_page(headquarters_id, offset, self.page_limit)
                .await
                .with_context(|| {
                    format!(
                        "Fetching SES people page for headquarters_id={} offset={} limit={}",
                        headquarters_id, offset, self.page_limit
                    )
                })?;

            if page.is_empty() {
                break;
            }

            let page_len = page.len();
            all.extend(page);
            offset += page_len;

            if page_len < self.page_limit {
                break;
            }
        }

        Ok(all)
    }

    pub async fn list_headquarters(&self) -> Result<Vec<SesHeadquarters>> {
        let mut offset = 0usize;
        let mut all = Vec::new();

        loop {
            let page = self
                .list_headquarters_page(offset, self.page_limit)
                .await
                .with_context(|| {
                    format!(
                        "Fetching SES headquarters page offset={} limit={}",
                        offset, self.page_limit
                    )
                })?;

            if page.is_empty() {
                break;
            }

            let page_len = page.len();
            all.extend(page);
            offset += page_len;

            if page_len < self.page_limit {
                break;
            }
        }

        Ok(all)
    }

    async fn fetch_people_page(
        &self,
        headquarters_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SesPerson>> {
        let url = format!(
            "{}/headquarters/{}/people",
            self.base_url.trim_end_matches('/'),
            headquarters_id
        );

        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .get(&url)
                .header("x-api-key", &self.api_key)
                .query(&[("offset", offset), ("limit", limit)])
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();

                    if status == StatusCode::NO_CONTENT {
                        return Ok(Vec::new());
                    }

                    if status.is_success() {
                        let people = resp.json::<Vec<SesPerson>>().await.with_context(|| {
                            format!(
                                "Parsing SES people response for headquarters_id={} offset={} limit={}",
                                headquarters_id, offset, limit
                            )
                        })?;
                        return Ok(people);
                    }

                    if status.is_client_error() {
                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response body>".to_string());
                        return Err(anyhow!(
                            "SES API client error {} for headquarters_id={} offset={} limit={}: {}",
                            status,
                            headquarters_id,
                            offset,
                            limit,
                            body
                        ));
                    }

                    if status.is_server_error() && attempt < self.max_retries {
                        let backoff_s = 1u64 << attempt;
                        warn!(
                            "SES API server error {} for headquarters_id={} offset={} limit={}, retrying in {}s (attempt {}/{})",
                            status,
                            headquarters_id,
                            offset,
                            limit,
                            backoff_s,
                            attempt + 1,
                            self.max_retries + 1
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                        continue;
                    }

                    return Err(anyhow!(
                        "SES API unexpected status {} for headquarters_id={} offset={} limit={}",
                        status,
                        headquarters_id,
                        offset,
                        limit
                    ));
                }
                Err(err) => {
                    if attempt < self.max_retries {
                        let backoff_s = 1u64 << attempt;
                        warn!(
                            "SES API request failed for headquarters_id={} offset={} limit={}, retrying in {}s (attempt {}/{}): {}",
                            headquarters_id,
                            offset,
                            limit,
                            backoff_s,
                            attempt + 1,
                            self.max_retries + 1,
                            err
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                        continue;
                    }

                    return Err(anyhow!(
                        "SES API request failed for headquarters_id={} offset={} limit={}: {}",
                        headquarters_id,
                        offset,
                        limit,
                        err
                    ));
                }
            }
        }

        Err(anyhow!(
            "SES API retries exhausted for headquarters_id={} offset={} limit={}",
            headquarters_id,
            offset,
            limit
        ))
    }

    /// this creates an NITC event but with most bits stripped out due to an SES API bug
    pub async fn create_nitc_event(&self, hq_id: i64, body: &SesNonIncidentCreate) -> Result<i64> {
        // SES API bugs:
        // * it appears to ignore completed=true on create so we'd have to update it immediately after anyway
        // * we've run into unknown HTTP 500s with both the create (POST) and update (PUT) endpoints so it seems
        //   safest to create with minimal data to reduce the risk of a 500 then update it later. Reducing the risk
        //   of a 500 on create is important because if the NITC event is inserted into the DB but a HTTP 500 is
        //   returned we'll end up with heaps of of duplicate NITC events.
        if body.name.chars().count() > 50 {
            anyhow::bail!(
                "NITC event name too long ({} chars) for headquarters_id={}: {}",
                body.name.chars().count(),
                hq_id,
                body.name
            );
        }

        let json = serde_json::json!({
            "name": body.name,
            "description": body.description,
            "type": body.nitc_type,
            "location": body.location,
            "startDate": body.start_date,
            "endDate": body.end_date,
            "participants": [], // this must be present
            "tags": body.tags,
            "completed": true, // this is ignored by SES on create but we'll update it later anyway
        });
        info!(
            "Creating NITC event for headquarters_id={}: {}",
            hq_id,
            serde_json::to_string(&json).unwrap_or_default()
        );
        let res: SesNonIncident = self
            .post_json_retry(
                &format!(
                    "{}/headquarters/{}/nitc",
                    self.base_url.trim_end_matches('/'),
                    hq_id
                ),
                &json,
                &format!("create NITC event hq={}", hq_id),
            )
            .await?;
        Ok(res.id)
    }

    pub async fn update_nitc_event(
        &self,
        hq_id: i64,
        body: &SesNonIncidentUpdate,
    ) -> Result<SesNonIncident> {
        let url = format!(
            "{}/headquarters/{}/nitc",
            self.base_url.trim_end_matches('/'),
            hq_id
        );
        if body.name.chars().count() > 50 {
            anyhow::bail!(
                "NITC event name too long ({} chars) for headquarters_id={}: {}",
                body.name.chars().count(),
                hq_id,
                body.name
            );
        }
        info!(
            "Updating NITC event for headquarters_id={}: {:?}",
            hq_id, body,
        );
        self.put_json_retry(&url, body, &format!("update NITC event hq={}", hq_id))
            .await
    }

    async fn post_json_retry<B: Serialize, R: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
        body: &B,
        ctx: &str,
    ) -> Result<R> {
        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .post(url)
                .header("x-api-key", &self.api_key)
                .json(body)
                .send()
                .await;

            match self.handle_response(response, url, ctx, attempt).await? {
                Some(text) => {
                    return serde_json::from_str(&text)
                        .with_context(|| format!("Parsing response from {}: {}", ctx, text));
                }
                None => continue,
            }
        }
        Err(anyhow!("Retries exhausted for {}", ctx))
    }

    async fn put_json_retry<B: Serialize, R: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
        body: &B,
        ctx: &str,
    ) -> Result<R> {
        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .put(url)
                .header("x-api-key", &self.api_key)
                .json(body)
                .send()
                .await;

            match self.handle_response(response, url, ctx, attempt).await? {
                Some(text) => {
                    return serde_json::from_str(&text)
                        .with_context(|| format!("Parsing response from {}: {}", ctx, text));
                }
                None => continue,
            }
        }
        Err(anyhow!("Retries exhausted for {}", ctx))
    }

    /// Returns Ok(Some(body)) on success, Ok(None) to retry, Err on fatal error.
    async fn handle_response(
        &self,
        response: reqwest::Result<reqwest::Response>,
        url: &str,
        ctx: &str,
        attempt: usize,
    ) -> Result<Option<String>> {
        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    return Ok(Some(text));
                }
                if status.is_client_error() {
                    let body = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "<failed to read body>".to_string());
                    return Err(anyhow!(
                        "SES API client error {} for {}: {}",
                        status,
                        ctx,
                        body
                    ));
                }
                if status.is_server_error() && attempt < self.max_retries {
                    let backoff_s = 1u64 << attempt;
                    let body = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "<failed to read body>".to_string());
                    warn!(
                        "SES API server error {} for {}, retrying in {}s (attempt {}/{}): {}",
                        status,
                        ctx,
                        backoff_s,
                        attempt + 1,
                        self.max_retries + 1,
                        body
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                    return Ok(None);
                }
                Err(anyhow!(
                    "SES API unexpected status {} for {}: {}",
                    status,
                    ctx,
                    url
                ))
            }
            Err(err) => {
                if attempt < self.max_retries {
                    let backoff_s = 1u64 << attempt;
                    warn!(
                        "SES API request failed for {}, retrying in {}s (attempt {}/{}): {}",
                        ctx,
                        backoff_s,
                        attempt + 1,
                        self.max_retries + 1,
                        err
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                    return Ok(None);
                }
                Err(anyhow!("SES API request failed for {}: {}", ctx, err))
            }
        }
    }

    async fn list_headquarters_page(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SesHeadquarters>> {
        let url = format!("{}/headquarters", self.base_url.trim_end_matches('/'));

        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .get(&url)
                .header("x-api-key", &self.api_key)
                .query(&[("offset", offset), ("limit", limit)])
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();

                    if status == StatusCode::NO_CONTENT {
                        return Ok(Vec::new());
                    }

                    if status.is_success() {
                        let headquarters =
                            resp.json::<Vec<SesHeadquarters>>().await.with_context(|| {
                                format!(
                                    "Parsing SES headquarters response offset={} limit={}",
                                    offset, limit
                                )
                            })?;
                        return Ok(headquarters);
                    }

                    if status.is_client_error() {
                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response body>".to_string());
                        return Err(anyhow!(
                            "SES API client error {} for headquarters offset={} limit={}: {}",
                            status,
                            offset,
                            limit,
                            body
                        ));
                    }

                    if status.is_server_error() && attempt < self.max_retries {
                        let backoff_s = 1u64 << attempt;
                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response body>".to_string());
                        warn!(
                            "SES API server error {} for headquarters offset={} limit={}, retrying in {}s (attempt {}/{}): {}",
                            status,
                            offset,
                            limit,
                            backoff_s,
                            attempt + 1,
                            self.max_retries + 1,
                            body
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                        continue;
                    }

                    return Err(anyhow!(
                        "SES API unexpected status {} for headquarters offset={} limit={}",
                        status,
                        offset,
                        limit
                    ));
                }
                Err(err) => {
                    if attempt < self.max_retries {
                        let backoff_s = 1u64 << attempt;
                        warn!(
                            "SES API request failed for headquarters offset={} limit={}, retrying in {}s (attempt {}/{}): {}",
                            offset,
                            limit,
                            backoff_s,
                            attempt + 1,
                            self.max_retries + 1,
                            err
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                        continue;
                    }

                    return Err(anyhow!(
                        "SES API request failed for headquarters offset={} limit={}: {}",
                        offset,
                        limit,
                        err
                    ));
                }
            }
        }

        Err(anyhow!(
            "SES API retries exhausted for headquarters offset={} limit={}",
            offset,
            limit
        ))
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );

        for attempt in 0..=self.max_retries {
            let response = self
                .client
                .get(&url)
                .header("x-api-key", &self.api_key)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();

                    if status.is_success() {
                        return resp
                            .json::<T>()
                            .await
                            .with_context(|| format!("Parsing SES response from {}", path));
                    }

                    if status.is_client_error() {
                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response body>".to_string());
                        return Err(anyhow!(
                            "SES API client error {} for {}: {}",
                            status,
                            path,
                            body
                        ));
                    }

                    if status.is_server_error() && attempt < self.max_retries {
                        let backoff_s = 1u64 << attempt;
                        warn!(
                            "SES API server error {} for {}, retrying in {}s (attempt {}/{})",
                            status,
                            path,
                            backoff_s,
                            attempt + 1,
                            self.max_retries + 1
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                        continue;
                    }

                    return Err(anyhow!("SES API unexpected status {} for {}", status, path));
                }
                Err(err) => {
                    if attempt < self.max_retries {
                        let backoff_s = 1u64 << attempt;
                        warn!(
                            "SES API request failed for {}, retrying in {}s (attempt {}/{}): {}",
                            path,
                            backoff_s,
                            attempt + 1,
                            self.max_retries + 1,
                            err
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_s)).await;
                        continue;
                    }

                    return Err(anyhow!("SES API request failed for {}: {}", path, err));
                }
            }
        }

        Err(anyhow!("SES API retries exhausted for {}", path))
    }

    pub async fn fetch_nonincident_types(&self) -> Result<Vec<String>> {
        self.get_json("/nonincidenttypes").await
    }

    pub async fn fetch_nonincident_tags(&self) -> Result<serde_json::Value> {
        self.get_json("/nonincidenttags").await
    }

    pub async fn fetch_participant_types(&self) -> Result<Vec<String>> {
        self.get_json("/participanttypes").await
    }

    pub async fn list_headquarters_cached(&self) -> Result<Arc<Vec<SesHeadquarters>>> {
        let mut guard = SES_CACHE.headquarters.lock().await;
        if let Some(ref entry) = *guard
            && entry.is_fresh()
        {
            return Ok(Arc::clone(&entry.value));
        }
        let data = self.list_headquarters().await?;
        let value = Arc::new(data);
        *guard = Some(CacheEntry {
            value: Arc::clone(&value),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub async fn fetch_nonincident_types_cached(&self) -> Result<Arc<Vec<String>>> {
        let mut guard = SES_CACHE.nonincident_types.lock().await;
        if let Some(ref entry) = *guard
            && entry.is_fresh()
        {
            return Ok(Arc::clone(&entry.value));
        }
        let data = self.fetch_nonincident_types().await?;
        let value = Arc::new(data);
        *guard = Some(CacheEntry {
            value: Arc::clone(&value),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub async fn fetch_nonincident_tags_cached(
        &self,
    ) -> Result<Arc<HashMap<i32, SesNonIncidentTag>>> {
        let mut guard = SES_CACHE.nonincident_tags.lock().await;
        if let Some(ref entry) = *guard
            && entry.is_fresh()
        {
            return Ok(Arc::clone(&entry.value));
        }
        let groups: Vec<NonIncidentTagGroup> = self.get_json("/nonincidenttags").await?;
        let mut map = HashMap::new();
        for group in groups {
            let primary_activity_name = group.primary_activity.name;
            for tag in group.tags {
                map.insert(
                    tag.id,
                    SesNonIncidentTag {
                        id: tag.id,
                        name: tag.name,
                        primary_activity_name: primary_activity_name.clone(),
                    },
                );
            }
        }
        let value = Arc::new(map);
        *guard = Some(CacheEntry {
            value: Arc::clone(&value),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub async fn fetch_participant_types_cached(&self) -> Result<Arc<Vec<String>>> {
        let mut guard = SES_CACHE.participant_types.lock().await;
        if let Some(ref entry) = *guard
            && entry.is_fresh()
        {
            return Ok(Arc::clone(&entry.value));
        }
        let data = self.fetch_participant_types().await?;
        let value = Arc::new(data);
        *guard = Some(CacheEntry {
            value: Arc::clone(&value),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }
}
