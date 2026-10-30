//! The rules behind location sharing ("Locate"), kept free of I/O. See SCHEMA.md's
//! `location_share` section for storage and `graphql::location_share` for the API.
//!
//! A user shares their phone's position with one location's members for a fixed time
//! (5, 15, 30 or 60 minutes), saying how often the phone will report in (every 1 to 5
//! minutes). Only the latest position is kept. A share shows for
//! [`LINGER_SECS`] after its latest position arrived, even once it has ended, so a
//! phone that loses signal fades from the map instead of vanishing at once; stopping a
//! share removes it straight away.

use crate::db::SharedPosition;

/// How long a share may run, in minutes.
pub const DURATIONS_MINUTES: [u64; 4] = [5, 15, 30, 60];
/// The shortest and longest the phone may wait between reports, in seconds.
pub const MIN_INTERVAL_SECS: u64 = 60;
pub const MAX_INTERVAL_SECS: u64 = 300;
/// How long a share keeps showing after its latest position: 7 minutes, so a phone on
/// the slowest interval can miss a report without dropping off.
pub const LINGER_SECS: u64 = 7 * 60;
/// Accuracies above this (100 km) are not a position worth showing.
pub const MAX_ACCURACY_METRES: f64 = 100_000.0;

/// A share's length in seconds, from minutes.
pub fn validate_duration(minutes: i32) -> Result<u64, String> {
    u64::try_from(minutes)
        .ok()
        .filter(|m| DURATIONS_MINUTES.contains(m))
        .map(|m| m * 60)
        .ok_or_else(|| "A location can be shared for 5, 15, 30 or 60 minutes".to_string())
}

pub fn validate_interval(seconds: i32) -> Result<u64, String> {
    u64::try_from(seconds)
        .ok()
        .filter(|s| (MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(s))
        .ok_or_else(|| {
            format!(
                "A location can be updated every {} to {} minutes",
                MIN_INTERVAL_SECS / 60,
                MAX_INTERVAL_SECS / 60
            )
        })
}

/// A position as a phone sent it, before it is checked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PositionInput {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: Option<f64>,
    pub altitude: Option<f64>,
    pub altitude_accuracy: Option<f64>,
    pub heading: Option<f64>,
    pub speed: Option<f64>,
    /// Unix seconds when the fix was taken; `now` when absent.
    pub recorded_at: Option<i64>,
}

/// Check a reported position. The latitude and longitude must be finite and in range,
/// and accuracies non-negative. Phones report "unknown" heading and speed as negative
/// numbers, so those become absent; a heading of 360 or more wraps round. A fix time
/// in the future (a phone's clock running fast) is taken as `now`.
pub fn validate_position(input: PositionInput, now: u64) -> Result<SharedPosition, String> {
    let PositionInput {
        latitude,
        longitude,
        accuracy,
        altitude,
        altitude_accuracy,
        heading,
        speed,
        recorded_at,
    } = input;
    if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
        return Err("Latitude must be between -90 and 90".to_string());
    }
    if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
        return Err("Longitude must be between -180 and 180".to_string());
    }
    let non_negative = |v: Option<f64>, what: &str| match v {
        Some(v) if !v.is_finite() || v < 0.0 => Err(format!("{what} must be zero or more")),
        v => Ok(v),
    };
    let accuracy = non_negative(accuracy, "Accuracy")?;
    if accuracy.is_some_and(|a| a > MAX_ACCURACY_METRES) {
        return Err("That position is too inaccurate to share".to_string());
    }
    let altitude_accuracy = non_negative(altitude_accuracy, "Altitude accuracy")?;
    if altitude.is_some_and(|a| !a.is_finite()) {
        return Err("Altitude must be a number".to_string());
    }
    let heading = heading
        .filter(|h| h.is_finite() && *h >= 0.0)
        .map(|h| h % 360.0);
    let speed = speed.filter(|s| s.is_finite() && *s >= 0.0);
    let recorded_at = match recorded_at {
        None => now,
        Some(t) if t < 0 => return Err("The position's time must be after 1970".to_string()),
        Some(t) => (t as u64).min(now),
    };
    Ok(SharedPosition {
        latitude,
        longitude,
        accuracy,
        altitude,
        altitude_accuracy,
        heading,
        speed,
        recorded_at,
    })
}

/// When a share whose latest position arrived at `updated_at` stops showing.
pub fn expires_at(updated_at: u64) -> u64 {
    updated_at + LINGER_SECS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(latitude: f64, longitude: f64) -> PositionInput {
        PositionInput {
            latitude,
            longitude,
            ..Default::default()
        }
    }

    #[test]
    fn durations_are_the_four_offered() {
        assert_eq!(validate_duration(5).unwrap(), 300);
        assert_eq!(validate_duration(60).unwrap(), 3600);
        for bad in [0, -5, 10, 90, 120] {
            assert!(validate_duration(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn intervals_are_one_to_five_minutes() {
        assert_eq!(validate_interval(60).unwrap(), 60);
        assert_eq!(validate_interval(150).unwrap(), 150);
        assert_eq!(validate_interval(300).unwrap(), 300);
        for bad in [-60, 0, 59, 301, 3600] {
            assert!(validate_interval(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn positions_must_be_on_the_globe() {
        assert!(validate_position(at(-33.7, 151.1), 100).is_ok());
        assert!(validate_position(at(90.0, 180.0), 100).is_ok());
        for (lat, lng) in [
            (91.0, 0.0),
            (0.0, -180.5),
            (f64::NAN, 0.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(validate_position(at(lat, lng), 100).is_err(), "{lat} {lng}");
        }
    }

    #[test]
    fn accuracies_must_be_sensible() {
        let with = |accuracy| PositionInput {
            accuracy: Some(accuracy),
            ..at(0.0, 0.0)
        };
        assert_eq!(
            validate_position(with(12.5), 100).unwrap().accuracy,
            Some(12.5)
        );
        assert!(validate_position(with(-1.0), 100).is_err());
        assert!(validate_position(with(f64::NAN), 100).is_err());
        assert!(validate_position(with(MAX_ACCURACY_METRES + 1.0), 100).is_err());
        let altitude = PositionInput {
            altitude_accuracy: Some(-3.0),
            ..at(0.0, 0.0)
        };
        assert!(validate_position(altitude, 100).is_err());
    }

    #[test]
    fn unknown_heading_and_speed_are_dropped() {
        let p = validate_position(
            PositionInput {
                heading: Some(-1.0),
                speed: Some(-1.0),
                ..at(0.0, 0.0)
            },
            100,
        )
        .unwrap();
        assert_eq!((p.heading, p.speed), (None, None));
        let p = validate_position(
            PositionInput {
                heading: Some(370.0),
                speed: Some(4.2),
                ..at(0.0, 0.0)
            },
            100,
        )
        .unwrap();
        assert_eq!((p.heading, p.speed), (Some(10.0), Some(4.2)));
    }

    #[test]
    fn fix_times_are_never_in_the_future() {
        let when = |recorded_at| PositionInput {
            recorded_at,
            ..at(0.0, 0.0)
        };
        assert_eq!(validate_position(when(None), 100).unwrap().recorded_at, 100);
        assert_eq!(
            validate_position(when(Some(40)), 100).unwrap().recorded_at,
            40
        );
        assert_eq!(
            validate_position(when(Some(500)), 100).unwrap().recorded_at,
            100
        );
        assert!(validate_position(when(Some(-1)), 100).is_err());
    }

    #[test]
    fn shares_linger_seven_minutes() {
        assert_eq!(expires_at(1000), 1420);
    }
}
