//! Derived images: the only form in which an uploaded photo is ever served.
//!
//! Originals (`originals/{asset_id}` in the asset bucket) are private and never leave
//! it. A member gets a normalised WebP at one of three fixed sizes ([`ImageVariant`]),
//! generated on first request and stored at `derived/{asset_id}/{variant}.webp`.
//!
//! # URLs
//!
//! `/media/{asset_id}/{variant}?e={expires}&s={sig}`, relative to the API origin
//! ([`media_path`]). `expires` is `(floor(now / 86400) + 2) * 86400`, so a URL is
//! identical for a whole UTC day (the CDN and the app can cache it) and valid for
//! between one and two days from when it was minted. `sig` is the first 16 bytes of
//! HMAC-SHA256 over `"{asset_id}/{variant}/{expires}"`, base64url without padding,
//! keyed by [`ServerSecret::media_url_key`]. Nobody who can reach the API can mint one
//! for an asset they were not shown.
//!
//! # Serving
//!
//! [`serve`] is the whole HTTP behaviour, shared by every server: `poem` and
//! `poem-local` mount it in-process (`server::run`) and the media Lambda
//! (`bin/media-lambda.rs`, behind each CloudFront distribution's `/media/*`) calls it
//! directly. **It never answers 403 or 404**: every CloudFront distribution rewrites
//! those to `/index.html` with a 200, which would hand a browser a web page in place of
//! an image.
//!
//! | status | when |
//! |---|---|
//! | 200 | the image; `image/webp`, `public, max-age=31536000, immutable` |
//! | 400 | an unknown variant, or a malformed path or asset id |
//! | 401 | the signature is missing, wrong or expired |
//! | 410 | there is no original (never completed, or since deleted) |
//! | 422 | the original cannot be decoded |
//! | 500 | storage or encoding failed |
//! | 503 | storage is not configured on this server |
//!
//! Every non-200 is `no-store`.
//!
//! # Pipeline
//!
//! [`derive`]: decode (JPEG or WebP, under [`MAX_PIXELS`] and [`MAX_ALLOC`] limits), a
//! cheap pre-shrink when the image is much larger than the target, a Catmull-Rom
//! resize to fit the variant's long edge (never upscaled), the EXIF orientation applied,
//! an embedded ICC profile converted to sRGB, then a lossy WebP at quality
//! [`WEBP_QUALITY`]. The output carries no metadata (no EXIF, so no GPS, no ICC).

use std::io::Cursor;

use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, Limits};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::server_secret::ServerSecret;
use crate::storage;

const DAY_SECS: u64 = 86_400;
/// Signature length in bytes (of the 32 HMAC-SHA256 produces).
const SIG_BYTES: usize = 16;
/// Refuse to decode more than 100 megapixels.
pub const MAX_PIXELS: u64 = 100_000_000;
/// Decoder allocation budget, within the media Lambda's 2048 MB.
pub const MAX_ALLOC: u64 = 768 * 1024 * 1024;
pub const WEBP_QUALITY: f32 = 80.0;
/// Pre-shrink (the cheap `thumbnail`) only when the source is at least this many
/// times the target's long edge; the exact resize then works from ~2x the target.
const PRESHRINK_RATIO: u32 = 3;

pub const CACHE_FOREVER: &str = "public, max-age=31536000, immutable";
pub const NO_STORE: &str = "no-store";

/// The sizes an image is served at. A GraphQL enum `ImageVariant { THUMB DISPLAY FULL }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, async_graphql::Enum)]
pub enum ImageVariant {
    Thumb,
    Display,
    Full,
}

impl ImageVariant {
    pub const ALL: [ImageVariant; 3] = [Self::Thumb, Self::Display, Self::Full];

    /// The name used in URLs and storage keys.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::Display => "display",
            Self::Full => "full",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// The longest edge, in pixels, of an image at this size.
    pub fn long_edge(self) -> u32 {
        match self {
            Self::Thumb => 400,
            Self::Display => 1440,
            Self::Full => 2560,
        }
    }
}

// ── URL signing ──────────────────────────────────────────────────────────────

fn expiry_for(now: u64) -> u64 {
    (now / DAY_SECS + 2) * DAY_SECS
}

fn signature(asset_id: &str, variant: ImageVariant, expires: u64, secret: &ServerSecret) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(&secret.media_url_key())
        .expect("HMAC accepts a key of any length");
    mac.update(format!("{asset_id}/{}/{expires}", variant.as_str()).as_bytes());
    BASE64_URL_SAFE_NO_PAD.encode(&mac.finalize().into_bytes()[..SIG_BYTES])
}

/// The relative URL for an asset's image at `variant`, valid from `now`: see the module
/// docs for the format and expiry window.
pub fn media_path(
    asset_id: &str,
    variant: ImageVariant,
    now: u64,
    secret: &ServerSecret,
) -> String {
    let expires = expiry_for(now);
    format!(
        "/media/{asset_id}/{}?e={expires}&s={}",
        variant.as_str(),
        signature(asset_id, variant, expires, secret)
    )
}

/// Whether `sig` is the signature [`media_path`] would produce for `expires`, and
/// `expires` is still in the future. The comparison is constant-time.
pub fn verify(
    asset_id: &str,
    variant: ImageVariant,
    expires: u64,
    sig: &str,
    now: u64,
    secret: &ServerSecret,
) -> bool {
    let expected = signature(asset_id, variant, expires, secret);
    let matches: bool = expected.as_bytes().ct_eq(sig.as_bytes()).into();
    matches && now < expires
}

// ── Derivation ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DeriveError {
    /// The bytes are not a JPEG or WebP we can decode, or are too large to.
    #[error("cannot decode image: {0}")]
    Undecodable(String),
    #[error("cannot encode image: {0}")]
    Encode(String),
}

fn undecodable(e: impl std::fmt::Display) -> DeriveError {
    DeriveError::Undecodable(e.to_string())
}

/// Dimensions that fit `(w, h)` within `long_edge` on the longer side, keeping the
/// aspect ratio; unchanged when it already fits (never upscaled).
fn fit(w: u32, h: u32, long_edge: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= long_edge {
        return (w, h);
    }
    let scale = |side: u32| {
        ((u64::from(side) * u64::from(long_edge) + u64::from(long) / 2) / u64::from(long)).max(1)
            as u32
    };
    (scale(w), scale(h))
}

/// Normalise `original` into the WebP served at `variant`. CPU-bound and memory-hungry:
/// call it from `spawn_blocking`.
pub fn derive(original: &[u8], variant: ImageVariant) -> Result<Vec<u8>, DeriveError> {
    let reader = image::ImageReader::new(Cursor::new(original))
        .with_guessed_format()
        .map_err(undecodable)?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg) | Some(ImageFormat::WebP)
    ) {
        return Err(undecodable("not a JPEG or WebP image"));
    }
    let mut decoder = reader.into_decoder().map_err(undecodable)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(undecodable(format!("{width}x{height} is too large")));
    }
    // A malformed profile or orientation tag is not worth refusing the photo for.
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let icc = decoder.icc_profile().ok().flatten();
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    decoder.set_limits(limits).map_err(undecodable)?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(undecodable)?;

    let (target_w, target_h) = fit(img.width(), img.height(), variant.long_edge());
    if (target_w, target_h) != (img.width(), img.height()) {
        if img.width().max(img.height()) / variant.long_edge() >= PRESHRINK_RATIO {
            let edge = variant.long_edge() * 2;
            img = img.thumbnail(edge, edge);
        }
        img = img.resize_exact(target_w, target_h, FilterType::CatmullRom);
    }
    img.apply_orientation(orientation);

    let has_alpha = img.color().has_alpha();
    let (w, h) = (img.width(), img.height());
    let mut pixels = if has_alpha {
        img.into_rgba8().into_raw()
    } else {
        img.into_rgb8().into_raw()
    };
    if let Some(icc) = icc {
        to_srgb(&icc, has_alpha, &mut pixels);
    }
    let encoder = if has_alpha {
        webp::Encoder::from_rgba(&pixels, w, h)
    } else {
        webp::Encoder::from_rgb(&pixels, w, h)
    };
    Ok(encoder.encode(WEBP_QUALITY).to_vec())
}

/// Convert 8-bit `pixels` from the embedded profile to sRGB in place. A profile we
/// cannot parse or build a transform for leaves the pixels as they are.
fn to_srgb(icc: &[u8], has_alpha: bool, pixels: &mut [u8]) {
    let Some(input) = qcms::Profile::new_from_slice(icc, false) else {
        return;
    };
    let ty = if has_alpha {
        qcms::DataType::RGBA8
    } else {
        qcms::DataType::RGB8
    };
    let output = qcms::Profile::new_sRGB();
    if let Some(transform) = qcms::Transform::new(&input, &output, ty, qcms::Intent::default()) {
        transform.apply(pixels);
    }
}

// ── Serving ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MediaResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub cache_control: &'static str,
    pub body: Vec<u8>,
}

impl MediaResponse {
    fn error(status: u16, message: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            cache_control: NO_STORE,
            body: message.as_bytes().to_vec(),
        }
    }

    fn image(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "image/webp",
            cache_control: CACHE_FOREVER,
            body,
        }
    }
}

/// Split `/media/{asset_id}/{variant}` into its two parts.
pub fn parse_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/media/")?;
    let (asset_id, variant) = rest.split_once('/')?;
    (!variant.contains('/')).then_some((asset_id, variant))
}

/// Asset IDs are `db::new_id()` values; anything else is refused before it can reach a
/// storage key.
fn valid_asset_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn query_param(query: &str, name: &str) -> Option<String> {
    form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
}

fn storage_failure(e: storage::Error) -> MediaResponse {
    match e {
        storage::Error::Unavailable => MediaResponse::error(503, "Media is not available"),
        e => {
            tracing::error!("media: storage failed: {e}");
            MediaResponse::error(500, "Media failed")
        }
    }
}

/// Answer `GET /media/{asset_id}/{variant}?{query}`. See the module docs for the
/// status codes.
pub async fn serve(
    storage: &impl storage::Handler,
    asset_id: &str,
    variant: &str,
    query: &str,
    now: u64,
    secret: &ServerSecret,
) -> MediaResponse {
    let (Some(variant), true) = (ImageVariant::parse(variant), valid_asset_id(asset_id)) else {
        return MediaResponse::error(400, "Bad request");
    };
    let expires = query_param(query, "e").and_then(|e| e.parse::<u64>().ok());
    let sig = query_param(query, "s");
    let (Some(expires), Some(sig)) = (expires, sig) else {
        return MediaResponse::error(401, "Invalid or expired link");
    };
    if !verify(asset_id, variant, expires, &sig, now, secret) {
        return MediaResponse::error(401, "Invalid or expired link");
    }

    let derived_key = storage::derived_key(asset_id, variant.as_str());
    match storage.get(&derived_key).await {
        Ok(Some(bytes)) => return MediaResponse::image(bytes),
        Ok(None) => {}
        Err(e) => return storage_failure(e),
    }
    let original = match storage.get(&storage::original_key(asset_id)).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return MediaResponse::error(410, "This image is gone"),
        Err(e) => return storage_failure(e),
    };
    let derived = match tokio::task::spawn_blocking(move || derive(&original, variant)).await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(DeriveError::Undecodable(e))) => {
            tracing::warn!("media: asset {asset_id} cannot be decoded: {e}");
            return MediaResponse::error(422, "This image cannot be processed");
        }
        Ok(Err(e)) => {
            tracing::error!("media: asset {asset_id}: {e}");
            return MediaResponse::error(500, "Media failed");
        }
        Err(e) => {
            tracing::error!("media: asset {asset_id}: derivation task failed: {e}");
            return MediaResponse::error(500, "Media failed");
        }
    };
    if let Err(e) = storage
        .put(&derived_key, derived.clone(), "image/webp")
        .await
    {
        // The image is still good; the next request will derive it again.
        tracing::error!("media: could not store {derived_key}: {e}");
    }
    MediaResponse::image(derived)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mockstorage;
    use crate::storage::Handler as _;

    fn secret() -> ServerSecret {
        ServerSecret::new("test-secret")
    }

    const NOON: u64 = 1_700_000_000;

    fn parts(path: &str) -> (String, String, String) {
        let (path, query) = path.split_once('?').unwrap();
        let (id, variant) = parse_path(path).unwrap();
        (id.to_string(), variant.to_string(), query.to_string())
    }

    #[test]
    fn variants_parse_and_round_trip() {
        for v in ImageVariant::ALL {
            assert_eq!(ImageVariant::parse(v.as_str()), Some(v));
        }
        assert_eq!(ImageVariant::Thumb.long_edge(), 400);
        assert_eq!(ImageVariant::Display.long_edge(), 1440);
        assert_eq!(ImageVariant::Full.long_edge(), 2560);
        for bad in ["", "THUMB", "Thumb", "huge", "thumb.webp", "../full"] {
            assert_eq!(ImageVariant::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn path_parsing() {
        assert_eq!(parse_path("/media/abc/thumb"), Some(("abc", "thumb")));
        for bad in [
            "/media/abc",
            "/media/abc/thumb/x",
            "/media",
            "/other/abc/thumb",
        ] {
            assert_eq!(parse_path(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn url_format_and_expiry_window() {
        let p = media_path("abc", ImageVariant::Thumb, NOON, &secret());
        let (id, variant, query) = parts(&p);
        assert_eq!((id.as_str(), variant.as_str()), ("abc", "thumb"));
        let expires: u64 = query_param(&query, "e").unwrap().parse().unwrap();
        assert_eq!(expires % DAY_SECS, 0);
        assert_eq!(expires, (NOON / DAY_SECS + 2) * DAY_SECS);
        assert!(expires - NOON > DAY_SECS && expires - NOON <= 2 * DAY_SECS);
        // 16 bytes in base64url without padding.
        assert_eq!(query_param(&query, "s").unwrap().len(), 22);
    }

    #[test]
    fn url_is_stable_within_a_utc_day_and_changes_after() {
        let day = NOON / DAY_SECS * DAY_SECS;
        let at = |t| media_path("abc", ImageVariant::Display, t, &secret());
        assert_eq!(at(day), at(day + DAY_SECS - 1));
        assert_ne!(at(day), at(day + DAY_SECS));
    }

    #[test]
    fn signature_verifies_until_it_expires() {
        let p = media_path("abc", ImageVariant::Full, NOON, &secret());
        let (_, _, q) = parts(&p);
        let expires = query_param(&q, "e").unwrap().parse().unwrap();
        let sig = query_param(&q, "s").unwrap();
        let ok = |t| verify("abc", ImageVariant::Full, expires, &sig, t, &secret());
        assert!(ok(NOON));
        assert!(ok(expires - 1));
        assert!(!ok(expires));
        assert!(!ok(expires + 1));
    }

    #[test]
    fn tampering_is_refused() {
        let p = media_path("abc", ImageVariant::Thumb, NOON, &secret());
        let (_, _, q) = parts(&p);
        let expires: u64 = query_param(&q, "e").unwrap().parse().unwrap();
        let sig = query_param(&q, "s").unwrap();
        let check =
            |id: &str, v, e, s: &str, secret: &ServerSecret| verify(id, v, e, s, NOON, secret);
        assert!(check("abc", ImageVariant::Thumb, expires, &sig, &secret()));
        // Another variant, asset, expiry, secret or signature.
        assert!(!check("abc", ImageVariant::Full, expires, &sig, &secret()));
        assert!(!check("abd", ImageVariant::Thumb, expires, &sig, &secret()));
        assert!(!check(
            "abc",
            ImageVariant::Thumb,
            expires + DAY_SECS,
            &sig,
            &secret()
        ));
        assert!(!check(
            "abc",
            ImageVariant::Thumb,
            expires,
            &sig,
            &ServerSecret::new("other")
        ));
        assert!(!check("abc", ImageVariant::Thumb, expires, "", &secret()));
        assert!(!check(
            "abc",
            ImageVariant::Thumb,
            expires,
            &sig[..21],
            &secret()
        ));
        let mut flipped = sig.clone().into_bytes();
        flipped[0] = if flipped[0] == b'A' { b'B' } else { b'A' };
        assert!(!check(
            "abc",
            ImageVariant::Thumb,
            expires,
            std::str::from_utf8(&flipped).unwrap(),
            &secret()
        ));
    }

    /// Pins the signature: a change would invalidate every URL a client has cached.
    #[test]
    fn signature_is_stable() {
        let expires = expiry_for(NOON);
        assert_eq!(
            signature("abc", ImageVariant::Thumb, expires, &secret()),
            "VjrvsyexdcX8ipihmT_umg"
        );
    }

    // ── derive ──

    fn gradient(w: u32, h: u32) -> image::RgbImage {
        image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x * 255 / w) as u8, (y * 255 / h) as u8, 90])
        })
    }

    fn jpeg(img: &image::RgbImage) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
            .unwrap();
        out
    }

    /// Insert an EXIF APP1 segment holding only the orientation tag after the SOI marker.
    fn with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
        let mut tiff = b"II*\0\x08\0\0\0".to_vec();
        tiff.extend_from_slice(&1u16.to_le_bytes()); // one IFD entry
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
        tiff.extend_from_slice(&3u16.to_le_bytes()); // SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // count
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]); // value padding
        tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
        let mut segment = b"Exif\0\0".to_vec();
        segment.extend_from_slice(&tiff);
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&((segment.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&segment);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn decode(webp: &[u8]) -> DynamicImage {
        image::load_from_memory_with_format(webp, ImageFormat::WebP).unwrap()
    }

    /// The four-character codes of a WebP file's chunks.
    fn chunks(webp: &[u8]) -> Vec<String> {
        assert_eq!(&webp[..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
        let mut at = 12;
        let mut found = vec![];
        while at + 8 <= webp.len() {
            found.push(String::from_utf8_lossy(&webp[at..at + 4]).into_owned());
            let len = u32::from_le_bytes(webp[at + 4..at + 8].try_into().unwrap()) as usize;
            at += 8 + len + (len & 1);
        }
        found
    }

    #[test]
    fn large_photo_is_shrunk_to_the_variant() {
        let original = jpeg(&gradient(4000, 3000));
        for (variant, dims) in [
            (ImageVariant::Thumb, (400, 300)),
            (ImageVariant::Display, (1440, 1080)),
            (ImageVariant::Full, (2560, 1920)),
        ] {
            let out = derive(&original, variant).unwrap();
            let img = decode(&out);
            assert_eq!((img.width(), img.height()), dims, "{variant:?}");
        }
    }

    #[test]
    fn portrait_photo_is_fitted_by_its_long_edge() {
        let out = derive(&jpeg(&gradient(1500, 3000)), ImageVariant::Thumb).unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (200, 400));
    }

    #[test]
    fn small_photo_is_not_upscaled() {
        let out = derive(&jpeg(&gradient(300, 200)), ImageVariant::Full).unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (300, 200));
    }

    #[test]
    fn exif_orientation_is_applied() {
        // Orientation 6: rotate 90 degrees clockwise to display upright.
        let landscape = with_orientation(&jpeg(&gradient(60, 40)), 6);
        let out = derive(&landscape, ImageVariant::Thumb).unwrap();
        let img = decode(&out);
        assert_eq!((img.width(), img.height()), (40, 60));
        // Without the tag it is left alone.
        let out = derive(&jpeg(&gradient(60, 40)), ImageVariant::Thumb).unwrap();
        assert_eq!((decode(&out).width(), decode(&out).height()), (60, 40));
    }

    #[test]
    fn orientation_applies_to_a_resized_photo_too() {
        let tall_after = with_orientation(&jpeg(&gradient(4000, 3000)), 6);
        let img = decode(&derive(&tall_after, ImageVariant::Thumb).unwrap());
        assert_eq!((img.width(), img.height()), (300, 400));
    }

    #[test]
    fn output_is_webp_without_metadata() {
        let original = with_orientation(&jpeg(&gradient(600, 400)), 3);
        assert!(original.windows(4).any(|w| w == b"Exif"));
        let out = derive(&original, ImageVariant::Thumb).unwrap();
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[8..12], b"WEBP");
        let found = chunks(&out);
        for banned in ["EXIF", "ICCP", "XMP "] {
            assert!(!found.iter().any(|c| c == banned), "{banned} in {found:?}");
        }
    }

    #[test]
    fn webp_input_is_accepted() {
        let rgb = gradient(500, 500);
        let webp = webp::Encoder::from_rgb(rgb.as_raw(), 500, 500)
            .encode(90.0)
            .to_vec();
        let img = decode(&derive(&webp, ImageVariant::Thumb).unwrap());
        assert_eq!((img.width(), img.height()), (400, 400));
    }

    #[test]
    fn undecodable_input_is_an_error() {
        let good = jpeg(&gradient(200, 200));
        for (what, bytes) in [
            ("empty", vec![]),
            ("garbage", b"definitely not an image".to_vec()),
            (
                "jpeg magic then garbage",
                [&good[..3], &[0x55; 64][..]].concat(),
            ),
            ("png", {
                let mut out = Vec::new();
                // PNG support is not compiled in, so build the signature by hand.
                out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
                out.extend_from_slice(&[0; 32]);
                out
            }),
        ] {
            assert!(
                matches!(
                    derive(&bytes, ImageVariant::Thumb),
                    Err(DeriveError::Undecodable(_))
                ),
                "{what}"
            );
        }
    }

    #[test]
    fn fit_never_upscales_and_keeps_the_ratio() {
        assert_eq!(fit(4000, 3000, 400), (400, 300));
        assert_eq!(fit(3000, 4000, 400), (300, 400));
        assert_eq!(fit(400, 100, 400), (400, 100));
        assert_eq!(fit(10, 10, 400), (10, 10));
        assert_eq!(fit(10_000, 1, 400), (400, 1));
    }

    // ── serve ──

    const ID: &str = "asset1";

    fn url(variant: ImageVariant) -> String {
        let (_, _, q) = parts(&media_path(ID, variant, NOON, &secret()));
        q
    }

    async fn serve_at(storage: &mockstorage::Handler, variant: &str, query: &str) -> MediaResponse {
        serve(storage, ID, variant, query, NOON, &secret()).await
    }

    #[tokio::test]
    async fn serve_derives_stores_and_then_reuses() {
        let storage = mockstorage::Handler::in_memory();
        let original = jpeg(&gradient(1000, 500));
        storage
            .put(&storage::original_key(ID), original, "image/jpeg")
            .await
            .unwrap();
        let q = url(ImageVariant::Thumb);

        let first = serve_at(&storage, "thumb", &q).await;
        assert_eq!(first.status, 200);
        assert_eq!(first.content_type, "image/webp");
        assert_eq!(first.cache_control, "public, max-age=31536000, immutable");
        let img = decode(&first.body);
        assert_eq!((img.width(), img.height()), (400, 200));

        let key = storage::derived_key(ID, "thumb");
        assert_eq!(storage.get(&key).await.unwrap().unwrap(), first.body);
        assert_eq!(storage.content_type(&key).as_deref(), Some("image/webp"));

        // Served from the stored copy even once the original is gone.
        storage.delete(&storage::original_key(ID)).await.unwrap();
        let second = serve_at(&storage, "thumb", &q).await;
        assert_eq!((second.status, second.body), (200, first.body));
    }

    #[tokio::test]
    async fn serve_returns_a_cached_derived_image_as_is() {
        let storage = mockstorage::Handler::in_memory();
        storage
            .put(
                &storage::derived_key(ID, "full"),
                b"cached".to_vec(),
                "image/webp",
            )
            .await
            .unwrap();
        let r = serve_at(&storage, "full", &url(ImageVariant::Full)).await;
        assert_eq!((r.status, r.body.as_slice()), (200, &b"cached"[..]));
    }

    #[tokio::test]
    async fn serve_status_codes() {
        let storage = mockstorage::Handler::in_memory();
        let q = url(ImageVariant::Thumb);

        let r = serve_at(&storage, "thumb", &q).await;
        assert_eq!((r.status, r.cache_control), (410, "no-store"));

        let r = serve_at(&storage, "huge", &q).await;
        assert_eq!((r.status, r.cache_control), (400, "no-store"));
        let r = serve(&storage, "a/b", "thumb", &q, NOON, &secret()).await;
        assert_eq!(r.status, 400);
        let r = serve(&storage, "", "thumb", &q, NOON, &secret()).await;
        assert_eq!(r.status, 400);

        // The variant is checked before the signature, so a bad one is always 400.
        assert_eq!(serve_at(&storage, "huge", "").await.status, 400);

        for query in ["", "e=1&s=x", &q.replace("s=", "s=X"), "s=abc"] {
            let r = serve_at(&storage, "thumb", query).await;
            assert_eq!((r.status, r.cache_control), (401, "no-store"), "{query:?}");
        }
        // A signature for another variant.
        let r = serve_at(&storage, "full", &q).await;
        assert_eq!(r.status, 401);
        // Expired.
        let r = serve(&storage, ID, "thumb", &q, expiry_for(NOON), &secret()).await;
        assert_eq!(r.status, 401);

        storage
            .put(
                &storage::original_key(ID),
                b"not an image".to_vec(),
                "image/jpeg",
            )
            .await
            .unwrap();
        let r = serve_at(&storage, "thumb", &q).await;
        assert_eq!((r.status, r.cache_control), (422, "no-store"));
        assert_eq!(
            storage
                .get(&storage::derived_key(ID, "thumb"))
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn serve_without_storage_is_unavailable() {
        let storage = mockstorage::Handler::disabled();
        let r = serve_at(&storage, "thumb", &url(ImageVariant::Thumb)).await;
        assert_eq!((r.status, r.cache_control), (503, "no-store"));
    }

    #[tokio::test]
    async fn serve_never_answers_403_or_404() {
        let storage = mockstorage::Handler::in_memory();
        let q = url(ImageVariant::Thumb);
        for (id, variant, query) in [
            (ID, "thumb", q.as_str()),
            (ID, "nope", q.as_str()),
            (ID, "thumb", ""),
            ("../x", "thumb", q.as_str()),
        ] {
            let r = serve(&storage, id, variant, query, NOON, &secret()).await;
            assert!(![403, 404].contains(&r.status), "{id} {variant} {query}");
        }
    }

    /// A store whose writes fail, to check the image is still returned.
    struct FailingPuts(mockstorage::Handler);

    impl storage::Handler for FailingPuts {
        fn is_enabled(&self) -> bool {
            true
        }
        async fn presign_post(
            &self,
            key: &str,
            ct: &str,
            size: u64,
            expires_in: std::time::Duration,
        ) -> storage::Result<storage::PresignedPost> {
            self.0.presign_post(key, ct, size, expires_in).await
        }
        async fn head(&self, key: &str) -> storage::Result<Option<storage::ObjectInfo>> {
            self.0.head(key).await
        }
        async fn get_range(&self, key: &str, start: u64, len: u64) -> storage::Result<Vec<u8>> {
            self.0.get_range(key, start, len).await
        }
        async fn get(&self, key: &str) -> storage::Result<Option<Vec<u8>>> {
            self.0.get(key).await
        }
        async fn put(&self, _: &str, _: Vec<u8>, _: &str) -> storage::Result<()> {
            Err(storage::Error::Infrastructure("disk full".into()))
        }
        async fn copy(&self, from: &str, to: &str) -> storage::Result<()> {
            self.0.copy(from, to).await
        }
        async fn delete(&self, key: &str) -> storage::Result<()> {
            self.0.delete(key).await
        }
    }

    #[tokio::test]
    async fn serve_returns_the_image_even_if_storing_it_fails() {
        let inner = mockstorage::Handler::in_memory();
        inner
            .put(
                &storage::original_key(ID),
                jpeg(&gradient(800, 600)),
                "image/jpeg",
            )
            .await
            .unwrap();
        let r = serve_at_failing(&FailingPuts(inner)).await;
        assert_eq!(r.status, 200);
        assert_eq!(decode(&r.body).width(), 400);
    }

    async fn serve_at_failing(storage: &FailingPuts) -> MediaResponse {
        serve(
            storage,
            ID,
            "thumb",
            &url(ImageVariant::Thumb),
            NOON,
            &secret(),
        )
        .await
    }
}
