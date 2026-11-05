//! The rules behind uploaded assets (photos now, files later), kept free of I/O. See
//! `graphql::assets` for the API, `storage` for where the bytes live and SCHEMA.md's
//! `asset` section for the stored shape.
//!
//! The phone sends what it read from the original (size, dimensions, time taken, camera)
//! with `createAssetUpload`. None of it is trusted: the type and size are enforced by the
//! presigned POST and re-checked at completion ([`sniff`]); the rest is clamped here so a
//! bad client cannot store nonsense.

use chrono::TimeZone;
use chrono_tz::Australia::Sydney;

/// The only types the server accepts. Tally converts HEIC to JPEG before uploading.
pub const ALLOWED_CONTENT_TYPES: [&str; 2] = ["image/jpeg", "image/webp"];

/// 30 MiB.
pub const MAX_UPLOAD_BYTES: u64 = 31_457_280;

pub const MAX_FILENAME_CHARS: usize = 255;
pub const MAX_CAMERA_CHARS: usize = 100;
pub const MAX_DIMENSION: i64 = 20_000;

/// How many uploads one member may start in a Sydney day.
pub const UPLOADS_PER_MEMBER_PER_DAY: u64 = 200;

/// How long a presigned upload form works.
pub const UPLOAD_URL_TTL_SECS: u64 = 15 * 60;

const DAY_SECS: u64 = 24 * 60 * 60;

/// A daily counter outlives its Sydney day by a margin, so the TTL never races a late write.
pub const COUNTER_TTL_SECS: u64 = 3 * DAY_SECS;

/// A client clock may be a day fast; a photo older than five years is not a recent one.
const TAKEN_AT_FUTURE_SLACK_SECS: u64 = DAY_SECS;
const TAKEN_AT_MAX_AGE_SECS: u64 = 5 * 365 * DAY_SECS;

/// The error is a message fit to show the member.
pub fn validate_content_type(content_type: &str) -> Result<&'static str, String> {
    ALLOWED_CONTENT_TYPES
        .into_iter()
        .find(|t| *t == content_type)
        .ok_or_else(|| "Only JPEG and WebP images can be uploaded".to_string())
}

pub fn validate_byte_size(byte_size: i64) -> Result<u64, String> {
    match u64::try_from(byte_size) {
        Ok(0) | Err(_) => Err("The file is empty".to_string()),
        Ok(n) if n > MAX_UPLOAD_BYTES => Err(format!(
            "The file is too large (the most allowed is {} MB)",
            MAX_UPLOAD_BYTES / (1024 * 1024)
        )),
        Ok(n) => Ok(n),
    }
}

/// The content type a file's first bytes announce, if it is one we accept: JPEG
/// (`FF D8 FF`) or WebP (`RIFF????WEBP`).
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// The name as picked, without any directory, control characters or surrounding space,
/// and at most [`MAX_FILENAME_CHARS`] characters. `None` if nothing is left.
pub fn sanitize_filename(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let cleaned: String = cleaned.trim().chars().take(MAX_FILENAME_CHARS).collect();
    let cleaned = cleaned.trim_end().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// A width or height hint: 1..=20000, else dropped.
pub fn clean_dimension(value: i64) -> Option<u32> {
    (1..=MAX_DIMENSION)
        .contains(&value)
        .then(|| u32::try_from(value).expect("range checked"))
}

/// The time the photo was taken, if it is plausible relative to `now` (all Unix seconds).
pub fn clamp_taken_at(taken_at: i64, now: u64) -> Option<u64> {
    let taken_at = u64::try_from(taken_at).ok()?;
    let newest = now.saturating_add(TAKEN_AT_FUTURE_SLACK_SECS);
    let oldest = now.saturating_sub(TAKEN_AT_MAX_AGE_SECS);
    (oldest..=newest).contains(&taken_at).then_some(taken_at)
}

/// A camera make or model, trimmed and at most [`MAX_CAMERA_CHARS`] characters; `None` if blank.
pub fn clean_camera_text(text: &str) -> Option<String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_CAMERA_CHARS)
        .collect();
    let cleaned = cleaned.trim_end().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

// ── Daily counters ───────────────────────────────────────────────────────────

/// `YYYY-MM-DD` of the Sydney day `now` (Unix seconds) falls in.
pub fn sydney_date(now: u64) -> String {
    Sydney
        .timestamp_opt(i64::try_from(now).unwrap_or(i64::MAX), 0)
        .single()
        .unwrap_or_else(|| Sydney.timestamp_opt(0, 0).unwrap())
        .format("%Y-%m-%d")
        .to_string()
}

/// The id of the daily counter `name` for `key` (a person or location ID) on the Sydney
/// day of `now`: `counter#<name>#<key>#<YYYY-MM-DD>`. See SCHEMA.md.
pub fn daily_counter_id(name: &str, key: &str, now: u64) -> String {
    format!("counter#{name}#{key}#{}", sydney_date(now))
}

/// When a counter written at `now` may be removed by DynamoDB's TTL.
pub fn counter_expires_at(now: u64) -> u64 {
    now + COUNTER_TTL_SECS
}

/// The counter limiting how many uploads `person_id` may start today.
pub fn upload_counter_id(person_id: &str, now: u64) -> String {
    daily_counter_id("asset-upload", person_id, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn only_jpeg_and_webp_are_accepted() {
        assert_eq!(validate_content_type("image/jpeg"), Ok("image/jpeg"));
        assert_eq!(validate_content_type("image/webp"), Ok("image/webp"));
        for bad in [
            "image/png",
            "image/heic",
            "IMAGE/JPEG",
            "",
            "application/pdf",
        ] {
            assert!(validate_content_type(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn size_must_be_positive_and_at_most_30_mib() {
        assert_eq!(MAX_UPLOAD_BYTES, 30 * 1024 * 1024);
        assert_eq!(validate_byte_size(1), Ok(1));
        assert_eq!(
            validate_byte_size(MAX_UPLOAD_BYTES as i64),
            Ok(MAX_UPLOAD_BYTES)
        );
        assert!(validate_byte_size(MAX_UPLOAD_BYTES as i64 + 1).is_err());
        assert!(validate_byte_size(0).is_err());
        assert!(validate_byte_size(-5).is_err());
    }

    #[test]
    fn sniffs_jpeg_and_webp_by_magic_bytes() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]), Some("image/jpeg"));
        assert_eq!(sniff(b"RIFF\x10\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n"), None);
        assert_eq!(sniff(b"GIF89a"), None);
        assert_eq!(sniff(b"RIFF\x10\0\0\0WAVEfmt "), None);
        assert_eq!(sniff(b"RIFF\x10\0\0\0WEB"), None);
        assert_eq!(sniff(&[0xFF, 0xD8]), None);
        assert_eq!(sniff(&[]), None);
    }

    #[test]
    fn filenames_lose_paths_controls_and_excess() {
        assert_eq!(
            sanitize_filename("IMG_0001.jpg").as_deref(),
            Some("IMG_0001.jpg")
        );
        assert_eq!(
            sanitize_filename("  ../../etc/pic.jpg ").as_deref(),
            Some("pic.jpg")
        );
        assert_eq!(
            sanitize_filename(r"C:\Users\me\a b.jpg").as_deref(),
            Some("a b.jpg")
        );
        assert_eq!(
            sanitize_filename("a\u{0}b\n.jpg").as_deref(),
            Some("ab.jpg")
        );
        assert_eq!(sanitize_filename(""), None);
        assert_eq!(sanitize_filename("   "), None);
        assert_eq!(sanitize_filename("dir/"), None);
        assert_eq!(sanitize_filename("\u{7}\u{8}"), None);
        let long = "é".repeat(400);
        assert_eq!(
            sanitize_filename(&long).unwrap().chars().count(),
            MAX_FILENAME_CHARS
        );
    }

    #[test]
    fn dimensions_are_one_to_twenty_thousand() {
        assert_eq!(clean_dimension(1), Some(1));
        assert_eq!(clean_dimension(20_000), Some(20_000));
        assert_eq!(clean_dimension(0), None);
        assert_eq!(clean_dimension(-1), None);
        assert_eq!(clean_dimension(20_001), None);
    }

    #[test]
    fn taken_at_must_be_plausible() {
        assert_eq!(clamp_taken_at(NOW as i64, NOW), Some(NOW));
        assert_eq!(clamp_taken_at(NOW as i64 + 86_400, NOW), Some(NOW + 86_400));
        assert_eq!(clamp_taken_at(NOW as i64 + 86_401, NOW), None);
        let oldest = NOW - 5 * 365 * 86_400;
        assert_eq!(clamp_taken_at(oldest as i64, NOW), Some(oldest));
        assert_eq!(clamp_taken_at(oldest as i64 - 1, NOW), None);
        assert_eq!(clamp_taken_at(-1, NOW), None);
        assert_eq!(clamp_taken_at(0, NOW), None);
    }

    #[test]
    fn camera_text_is_trimmed_and_capped() {
        assert_eq!(clean_camera_text("  Apple ").as_deref(), Some("Apple"));
        assert_eq!(clean_camera_text("   "), None);
        assert_eq!(
            clean_camera_text(&"x".repeat(150)).unwrap().chars().count(),
            MAX_CAMERA_CHARS
        );
    }

    #[test]
    fn counter_ids_use_the_sydney_date() {
        // 2027-01-15 13:30 UTC is already 00:30 on the 16th in Sydney (UTC+11).
        let now = 1_800_019_800;
        assert_eq!(sydney_date(now), "2027-01-16");
        assert_eq!(sydney_date(now - 3_600), "2027-01-15");
        assert_eq!(
            upload_counter_id("p1", now),
            "counter#asset-upload#p1#2027-01-16"
        );
        assert_eq!(
            daily_counter_id("photo", "loc", now),
            "counter#photo#loc#2027-01-16"
        );
        assert_eq!(counter_expires_at(now), now + 3 * 86_400);
        assert_eq!(UPLOADS_PER_MEMBER_PER_DAY, 200);
    }
}
