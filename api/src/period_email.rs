//! Member-facing period emails — the single home for this wording.
//!
//! Two situations share one builder, and the variant is derived from the data
//! (`end_time.is_none()`) rather than passed in, so the callers cannot disagree
//! about which copy a given period should get:
//!
//!   * **complete** — "please check this entry we recorded for you", sent by an
//!     admin pressing *Remind*; and
//!   * **incomplete** — "you're still signed in", sent automatically by
//!     [`crate::open_period_notice`] when someone forgets to sign out.
//!
//! Plain text on purpose: it renders identically everywhere, and a bare URL is
//! easier to trust than a styled button in a message asking someone to click a
//! link. Kept as pure functions so both wordings are unit-testable without SES.

/// Subject for a period that already has an end time.
const COMPLETE_SUBJECT: &str = "Please check your SES activity time entry";

/// Subject for a period that is still open. Deliberately states the problem
/// rather than asking for a review — the member has to *do* something here.
const INCOMPLETE_SUBJECT: &str = "You didn't sign out of your SES activity";

/// Everything needed to render a period email.
pub struct PeriodEmail<'a> {
    pub first_name: &'a str,
    pub location_name: &'a str,
    pub category_name: Option<&'a str>,
    pub start_time: u64,
    /// `None` selects the "still signed in" copy set.
    pub end_time: Option<u64>,
    /// The member-facing edit link, from [`crate::period_link::edit_link_url`].
    pub url: &'a str,
}

/// Subject line for a period email, chosen by the same rule as the body.
pub fn subject(end_time: Option<u64>) -> &'static str {
    match end_time {
        Some(_) => COMPLETE_SUBJECT,
        None => INCOMPLETE_SUBJECT,
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

/// Describe how long ago something was, for the incomplete copy. Rounded down to
/// whole hours: the point is "this has been open a long time", not precision.
fn describe_elapsed(start_time: u64, now: u64) -> String {
    let hours = now.saturating_sub(start_time) / 3600;
    match hours {
        0 => "less than an hour ago".to_string(),
        1 => "about an hour ago".to_string(),
        h if h < 48 => format!("about {h} hours ago"),
        h => format!("about {} days ago", h / 24),
    }
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

/// Build the body of a period email, picking the copy set from `end_time`.
///
/// `now` is passed in rather than read from the clock so the "about N hours ago"
/// phrasing is testable; callers pass [`crate::clock::now_sec`].
pub fn build(email: &PeriodEmail<'_>, now: u64) -> String {
    match email.end_time {
        Some(end) => build_complete(email, end),
        None => build_incomplete(email, now),
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
         SES Activity administrators\n",
        greeting(email.first_name),
        email.location_name,
        activity_line(email.category_name),
        format_period_datetime(email.start_time),
        format_period_datetime(end_time),
        email.url,
    )
}

/// "You're still signed in" — the period has no end time.
///
/// Leads with the problem and asks for one specific thing (the finish time),
/// because unlike the complete case there is no "do nothing" outcome: an entry
/// left open never counts towards the member's recorded hours.
fn build_incomplete(email: &PeriodEmail<'_>, now: u64) -> String {
    format!(
        "{}\n\n\
         You're still signed in at {}, and it looks like you forgot to sign out.\n\n\
         {}  Started:  {} ({})\n  Finished: not recorded\n\n\
         Please enter the time you finished here:\n\n\
         {}\n\n\
         This link works for the next 48 hours and only opens this one entry. \
         Until a finish time is recorded, this activity won't count towards your hours.\n\n\
         Thanks,\n\
         SES Activity administrators\n",
        greeting(email.first_name),
        email.location_name,
        activity_line(email.category_name),
        format_period_datetime(email.start_time),
        describe_elapsed(email.start_time, now),
        email.url,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2024-06-01 09:00 Sydney = 2024-05-31 23:00 UTC.
    const START: u64 = 1_717_196_400;

    fn email(end_time: Option<u64>) -> PeriodEmail<'static> {
        PeriodEmail {
            first_name: "Alex",
            location_name: "Springwood",
            category_name: Some("Training"),
            start_time: START,
            end_time,
            url: "https://new.seslogin.com/period#slp_abc",
        }
    }

    #[test]
    fn subject_follows_the_end_time() {
        assert_eq!(subject(Some(START + 3600)), COMPLETE_SUBJECT);
        assert_eq!(subject(None), INCOMPLETE_SUBJECT);
        // The incomplete subject must not read as a routine "please check" —
        // it is the difference between an FYI and an action.
        assert_ne!(subject(None), subject(Some(0)));
    }

    #[test]
    fn complete_email_reads_as_a_review_request() {
        let body = build(&email(Some(START + 4 * 3600)), START + 5 * 3600);
        assert!(body.starts_with("Hi Alex,"), "{body}");
        assert!(body.contains("Please check the following activity"));
        assert!(body.contains("  Activity: Training\n"));
        assert!(body.contains("  Started:  Sat 1 Jun 2024, 09:00\n"));
        assert!(body.contains("  Finished: Sat 1 Jun 2024, 13:00\n"));
        assert!(body.contains("https://new.seslogin.com/period#slp_abc"));
        assert!(body.contains("you don't need to do anything"));
        // Must not leak the incomplete framing.
        assert!(!body.contains("still signed in"));
    }

    #[test]
    fn incomplete_email_asks_for_a_finish_time() {
        let body = build(&email(None), START + 13 * 3600);
        assert!(body.starts_with("Hi Alex,"), "{body}");
        assert!(body.contains("You're still signed in at Springwood"));
        assert!(body.contains("forgot to sign out"));
        assert!(body.contains("  Started:  Sat 1 Jun 2024, 09:00 (about 13 hours ago)\n"));
        assert!(body.contains("  Finished: not recorded\n"));
        assert!(body.contains("Please enter the time you finished here:"));
        assert!(body.contains("https://new.seslogin.com/period#slp_abc"));
        assert!(body.contains("won't count towards your hours"));
        // The "nothing to do" line from the complete copy would be actively
        // wrong here — an open entry always needs action.
        assert!(!body.contains("you don't need to do anything"));
    }

    #[test]
    fn missing_first_name_falls_back_to_a_bare_greeting() {
        let mut e = email(None);
        e.first_name = "   ";
        assert!(build(&e, START).starts_with("Hello,\n"));
    }

    #[test]
    fn missing_category_omits_the_activity_line() {
        let mut e = email(None);
        e.category_name = None;
        let body = build(&e, START + 13 * 3600);
        assert!(!body.contains("Activity:"));
        assert!(body.contains("  Started:  "));
    }

    #[test]
    fn elapsed_is_described_in_sensible_units() {
        assert_eq!(describe_elapsed(1000, 1000), "less than an hour ago");
        assert_eq!(describe_elapsed(0, 3599), "less than an hour ago");
        assert_eq!(describe_elapsed(0, 3600), "about an hour ago");
        assert_eq!(describe_elapsed(0, 12 * 3600), "about 12 hours ago");
        assert_eq!(describe_elapsed(0, 47 * 3600), "about 47 hours ago");
        assert_eq!(describe_elapsed(0, 48 * 3600), "about 2 days ago");
        assert_eq!(describe_elapsed(0, 7 * 24 * 3600), "about 7 days ago");
        // A clock skewed backwards must not underflow.
        assert_eq!(describe_elapsed(2000, 1000), "less than an hour ago");
    }
}
