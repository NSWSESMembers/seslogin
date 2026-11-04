//! Member-facing period emails — the single home for this wording.
//!
//! Two situations share one builder, and the variant is derived from the data
//! (`end_time.is_none()`) rather than passed in, so the callers cannot disagree
//! about which copy a given period should get:
//!
//!   * **complete** — "please check this entry we recorded for you", sent by an
//!     admin pressing *Remind* on a finished entry; and
//!   * **still open** — "you may have forgotten to sign out", sent by an admin
//!     pressing *Remind* on an open entry, and automatically by
//!     [`crate::open_period_notice`].
//!
//! Plain text on purpose: it renders identically everywhere, and a bare URL is
//! easier to trust than a styled button in a message asking someone to click a
//! link. Kept as pure functions so both wordings are unit-testable without SES.

/// Subject for a period that already has an end time.
const COMPLETE_SUBJECT: &str = "Please check your SES activity time entry";

/// Subject for a period that is still open.
const OPEN_SUBJECT: &str = "Forgot to sign out at SES?";

/// Everything needed to render a period email.
pub struct PeriodEmail<'a> {
    pub first_name: &'a str,
    pub location_name: &'a str,
    pub category_name: Option<&'a str>,
    pub start_time: u64,
    /// `None` selects the "forgot to sign out" copy.
    pub end_time: Option<u64>,
    /// The member-facing edit link, from [`crate::period_link::edit_link_url`].
    pub url: &'a str,
}

/// Subject line for a period email, chosen by the same rule as the body.
pub fn subject(end_time: Option<u64>) -> &'static str {
    match end_time {
        Some(_) => COMPLETE_SUBJECT,
        None => OPEN_SUBJECT,
    }
}

/// Render a timestamp in Sydney local time, matching the activity summary email.
pub(crate) fn format_period_datetime(ts: u64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono_tz::Australia::Sydney)
        .format("%a %-d %b %Y, %H:%M")
        .to_string()
}

fn greeting(first_name: &str) -> String {
    if first_name.trim().is_empty() {
        "Hello,".to_string()
    } else {
        format!("Hi {},", first_name.trim())
    }
}

fn activity_line(category_name: Option<&str>) -> String {
    category_name
        .map(|c| format!("  Activity: {c}\n"))
        .unwrap_or_default()
}

/// Build the body of a period email, picking the copy from `end_time`.
pub fn build(email: &PeriodEmail<'_>) -> String {
    match email.end_time {
        Some(end) => build_complete(email, end),
        None => build_open(email),
    }
}

/// "Please check this entry" — the period already has both times.
fn build_complete(email: &PeriodEmail<'_>, end_time: u64) -> String {
    format!(
        "{}\n\n\
         Please check the following activity recorded for you at {}:\n\n\
         {}  Started:  {}\n  Finished: {}\n\n\
         If that isn't right, you can correct the times and the activity here:\n\n\
         {}\n\n\
         This link works for the next 48 hours and only opens this one entry. \
         If the details above are already correct, you don't need to do anything.\n\n\
         Thanks,\n\
         SESLogin administrators\n",
        greeting(email.first_name),
        email.location_name,
        activity_line(email.category_name),
        format_period_datetime(email.start_time),
        format_period_datetime(end_time),
        email.url,
    )
}

/// "Did you forget to sign out" — the period has no end time.
///
/// Separate from [`build_complete`] because the two cases need different framing:
/// a completed entry just needs checking, but an open one implies the member may
/// still be signed in and points them at the link to finish signing out rather
/// than merely "correct" the entry.
fn build_open(email: &PeriodEmail<'_>) -> String {
    format!(
        "{}\n\n\
         We don't have a sign-out time recorded for you at {} — you may have \
         forgotten to sign out:\n\n\
         {}  You signed in at:  {}\n\n\
         Please complete your sign-out by clicking the link below, confirming your \
         sign out time and picking an activity category:\n\n\
         {}\n\n\
         This link works for the next 48 hours and only opens this one entry. \
         If you aren't ready to sign out yet you can come back here and click \
         the link later.\n\n\
         Thanks,\n\
         SESLogin administrators\n",
        greeting(email.first_name),
        email.location_name,
        activity_line(email.category_name),
        format_period_datetime(email.start_time),
        email.url,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-07-24 09:53 and 09:55 Sydney time.
    const START: u64 = 1784850780;
    const END: u64 = 1784850959;

    fn body(first_name: &str, category: Option<&str>, end_time: Option<u64>) -> String {
        build(&PeriodEmail {
            first_name,
            location_name: "Test Unit",
            category_name: category,
            start_time: START,
            end_time,
            url: "https://new.seslogin.com/period#slp_abc",
        })
    }

    #[test]
    fn subject_follows_the_end_time() {
        assert_eq!(subject(Some(END)), COMPLETE_SUBJECT);
        assert_eq!(subject(None), OPEN_SUBJECT);
    }

    mod complete {
        use super::*;

        #[test]
        fn includes_the_link_and_the_entry_details() {
            let out = body("Sam", Some("Training"), Some(END));
            assert!(out.contains("Hi Sam,"));
            assert!(out.contains("Test Unit"));
            assert!(out.contains("Activity: Training"));
            assert!(out.contains("https://new.seslogin.com/period#slp_abc"));
            assert!(out.contains("48 hours"));
            assert!(!out.contains("forgotten to sign out"));
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
    }

    mod open {
        use super::*;

        #[test]
        fn implies_a_forgotten_sign_out_and_includes_the_link() {
            let out = body("Sam", Some("Training"), None);
            assert!(out.contains("Hi Sam,"));
            assert!(out.contains("forgotten to sign out"));
            assert!(out.contains("Test Unit"));
            assert!(out.contains("Activity: Training"));
            assert!(out.contains("You signed in at:"));
            assert!(out.contains("https://new.seslogin.com/period#slp_abc"));
            assert!(out.contains("48 hours"));
            // The "nothing to do" line from the complete copy would be wrong here.
            assert!(!out.contains("you don't need to do anything"));
        }

        #[test]
        fn formats_the_start_time_in_sydney_local_time() {
            let out = body("Sam", None, None);
            assert!(out.contains("Fri 24 Jul 2026, 09:53"), "got: {out}");
        }

        #[test]
        fn falls_back_to_a_neutral_greeting_without_a_name() {
            assert!(body("", None, None).starts_with("Hello,"));
            assert!(body("   ", None, None).starts_with("Hello,"));
        }

        #[test]
        fn omits_the_activity_line_when_uncategorised() {
            assert!(!body("Sam", None, None).contains("Activity:"));
        }
    }
}
