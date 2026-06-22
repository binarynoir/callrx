use crate::api::{AddressInfo, CallsignRecord};
use crate::cache::TTL_SECS;
use crate::hyperlink;
use anstream::{eprintln, println};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Spinner ───────────────────────────────────────────────────────────────────

pub fn make_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

// ── Pretty output ─────────────────────────────────────────────────────────────

/// Rendered width of a string, ignoring ANSI SGR color codes and OSC 8
/// hyperlink escape sequences. We lay the table out by hand because no table
/// crate measures embedded OSC 8 hyperlinks correctly — they count the URL
/// bytes as visible columns and blow the layout out.
fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\x1b' {
            width += 1;
            continue;
        }
        match chars.peek() {
            // CSI (e.g. colors): ESC [ … <final letter>
            Some('[') => {
                chars.next();
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            // OSC (e.g. hyperlinks): ESC ] … terminated by BEL or ST (ESC \)
            Some(']') => {
                chars.next();
                while let Some(c2) = chars.next() {
                    if c2 == '\x07' {
                        break;
                    }
                    if c2 == '\x1b' {
                        chars.next(); // consume the '\' of the ST terminator
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    width
}

/// Converts a Unix timestamp to a YYYY-MM-DD string (UTC).
///
/// Uses Howard Hinnant's civil_from_days algorithm — no date library needed.
pub(crate) fn unix_to_date(ts: u64) -> String {
    let days = ts / 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Converts a Unix timestamp to an HH:MM string (UTC).
pub(crate) fn unix_to_time(ts: u64) -> String {
    let secs = ts % 86_400;
    format!("{:02}:{:02}", secs / 3_600, (secs % 3_600) / 60)
}

fn age_words(age_secs: u64) -> String {
    if age_secs < 120 {
        "just now".to_string()
    } else if age_secs < 3_600 {
        format!("{} min ago", age_secs / 60)
    } else if age_secs < 172_800 {
        format!("{} hr ago", age_secs / 3_600)
    } else {
        format!("{} days ago", age_secs / 86_400)
    }
}

fn ttl_words(remaining_secs: u64) -> String {
    if remaining_secs < 3_600 {
        format!("{} min", remaining_secs / 60)
    } else if remaining_secs < 172_800 {
        format!("{} hr", remaining_secs / 3_600)
    } else {
        format!("{} days", remaining_secs / 86_400)
    }
}

/// Returns a single human-readable string describing the cache entry, e.g.:
/// "2026-06-09 · fetched 3 days ago · refreshes in 4 days"
pub(crate) fn cache_info_label(cached_at: u64, ttl_secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age = now.saturating_sub(cached_at);
    let remaining = ttl_secs.saturating_sub(age);
    let date = unix_to_date(cached_at);
    format!(
        "{date} · {} · refreshes in {}",
        age_words(age),
        ttl_words(remaining)
    )
}

pub fn print_pretty(record: &CallsignRecord, links_enabled: bool, cached_at: Option<u64>) {
    // Detect OSC 8 support — respect the caller's override flag
    let use_links = links_enabled && hyperlink::osc8_supported();

    let callsign = record.callsign();
    let name = record.display_name.as_str();
    let rows = build_rows(record, use_links, cached_at);

    // ── Render a content-sized box, measuring visible width to stay aligned ─────
    let label_w = rows
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    let value_w = rows
        .iter()
        .map(|(_, v)| visible_width(v))
        .max()
        .unwrap_or(0);
    // " " + label + "   " + value + " " between the vertical borders
    let inner_w = 1 + label_w + 3 + value_w + 1;

    println!();
    println!(
        "{} {}",
        callsign.bold().bright_cyan(),
        format!("· {name}").dimmed()
    );
    println!("┌{}┐", "─".repeat(inner_w));
    for (label, value) in &rows {
        let label_padded = format!("{label:>label_w$}");
        let label_cell = label_padded.cyan().bold().to_string();
        let pad = " ".repeat(value_w - visible_width(value));
        println!("│ {label_cell}   {value}{pad} │");
    }
    println!("└{}┘", "─".repeat(inner_w));
    if cached_at.is_some() {
        println!(
            "  {}",
            "Cached data · use --no-cache to fetch fresh".dimmed()
        );
    }
    println!();
}

/// Composes the "CITY, ST ZIP" address line from the structured address,
/// returning None when there is nothing to show.
fn format_city_line(addr: &AddressInfo) -> Option<String> {
    let city = addr.city.as_deref().unwrap_or("").trim();
    let state = addr.state.as_deref().unwrap_or("").trim();
    let zip = addr.zip_code.as_deref().unwrap_or("").trim();

    let mut line = String::new();
    if !city.is_empty() {
        line.push_str(city);
    }
    if !state.is_empty() {
        if !line.is_empty() {
            line.push_str(", ");
        }
        line.push_str(state);
    }
    if !zip.is_empty() {
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(zip);
    }
    if line.is_empty() { None } else { Some(line) }
}

/// Builds the `(label, styled value)` rows shown in the pretty table.
fn build_rows(
    record: &CallsignRecord,
    use_links: bool,
    cached_at: Option<u64>,
) -> Vec<(&'static str, String)> {
    let expired = record.is_expired();
    let mut rows: Vec<(&'static str, String)> = Vec::new();

    // Status — marker color keyed on the FCC status code, label from the API.
    let label = record.license_status_label.as_str();
    let status_val = match record.license_status.as_str() {
        "A" => format!("✓ {label}").bright_green().bold().to_string(),
        "E" => format!("✗ {label}").bright_red().bold().to_string(),
        _ => format!("⚠ {label}").bright_yellow().bold().to_string(),
    };
    rows.push(("Status", status_val));

    // License type
    rows.push((
        "Type",
        record
            .license_type
            .as_deref()
            .unwrap_or("Individual")
            .to_string(),
    ));

    // License class (skipped when unknown/absent)
    let class = record.license_class_label();
    if class != "—" {
        let class_val = match class {
            "Amateur Extra" => class.bright_magenta().bold().to_string(),
            "Advanced" => class.bright_blue().bold().to_string(),
            "General" => class.bright_cyan().to_string(),
            "Technician" => class.bright_green().to_string(),
            "Novice" => class.yellow().to_string(),
            _ => class.to_string(),
        };
        rows.push(("Class", class_val));
    }

    // Trustee (club licenses)
    if let Some(tcall) = record.trustee_callsign.as_deref().filter(|s| !s.is_empty()) {
        let tname = record.trustee_name.as_deref().unwrap_or("");
        rows.push((
            "Trustee",
            format!("{} — {tname}", tcall.bright_cyan().bold()),
        ));
    }

    // Previous callsign
    if let Some(pc) = record
        .previous_callsign
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        rows.push(("Previous", pc.dimmed().to_string()));
    }

    // Address
    let addr = &record.address;
    if let Some(line1) = addr
        .street
        .as_deref()
        .or(addr.po_box.as_deref())
        .filter(|s| !s.is_empty())
    {
        rows.push(("Address", line1.to_string()));
    }
    if let Some(line2) = format_city_line(addr) {
        rows.push(("", line2));
    }

    // Dates and identifiers
    if let Some(g) = record.grant_date.as_deref().filter(|s| !s.is_empty()) {
        rows.push(("Granted", g.to_string()));
    }
    if let Some(e) = record.expired_date.as_deref().filter(|s| !s.is_empty()) {
        let expiry_val = if expired {
            e.bright_red().to_string()
        } else {
            e.bright_green().to_string()
        };
        rows.push(("Expires", expiry_val));
    }
    if let Some(d) = record.last_action_date.as_deref().filter(|s| !s.is_empty()) {
        rows.push(("Last Action", d.dimmed().to_string()));
    }
    if let Some(frn) = record.frn.as_deref().filter(|s| !s.is_empty()) {
        rows.push(("FRN", frn.dimmed().to_string()));
    }
    // ULS link — clickable if supported
    if let Some(uls_url) = record.uls_url.as_deref().filter(|s| !s.is_empty()) {
        rows.push(("ULS Record", hyperlink::link(uls_url, uls_url, use_links)));
    }

    if let Some(ca) = cached_at {
        rows.push((
            "Cached",
            cache_info_label(ca, TTL_SECS).dimmed().to_string(),
        ));
    }

    rows
}

// ── Plain text output (--raw) ─────────────────────────────────────────────────

pub fn print_plain(record: &CallsignRecord, cached_at: Option<u64>) {
    println!("Callsign:    {}", record.callsign());
    println!("Status:      {}", record.license_status_label);

    if let Some(t) = &record.license_type {
        println!("Type:        {t}");
    }
    println!("Class:       {}", record.license_class_label());

    println!("Name:        {}", record.display_name);

    if let Some(tc) = record.trustee_callsign.as_deref().filter(|s| !s.is_empty()) {
        let tn = record.trustee_name.as_deref().unwrap_or("");
        println!("Trustee:     {tn} ({tc})");
    }

    let addr = &record.address;
    if let Some(l1) = addr
        .street
        .as_deref()
        .or(addr.po_box.as_deref())
        .filter(|s| !s.is_empty())
    {
        println!("Address:     {l1}");
    }
    if let Some(l2) = format_city_line(addr) {
        println!("             {l2}");
    }

    if let Some(g) = record.grant_date.as_deref().filter(|s| !s.is_empty()) {
        println!("Granted:     {g}");
    }
    if let Some(e) = record.expired_date.as_deref().filter(|s| !s.is_empty()) {
        println!("Expires:     {e}");
    }
    if let Some(uls) = record.uls_url.as_deref().filter(|s| !s.is_empty()) {
        println!("ULS URL:     {uls}");
    }

    if let Some(ca) = cached_at {
        println!("Cached:      {}", cache_info_label(ca, TTL_SECS));
    }
}

// ── History output ────────────────────────────────────────────────────────────

pub fn print_history(callsign: &str, events: &[(u64, String)]) {
    let count = events.len();
    let plural = if count == 1 { "lookup" } else { "lookups" };

    println!();
    println!(
        "{} {}",
        callsign.bold().bright_cyan(),
        format!("· {count} {plural}").dimmed()
    );

    if events.is_empty() {
        println!("  {}", "No lookup history found.".dimmed());
        println!();
        return;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    println!();
    for (ts, source) in events {
        let date = unix_to_date(*ts);
        let time = unix_to_time(*ts);
        let age = now.saturating_sub(*ts);
        let source_label = if source == "api" {
            "live  ".bright_green().to_string()
        } else {
            "cached".dimmed().to_string()
        };
        println!(
            "  {}   {}   {}",
            format!("{date} {time}").dimmed(),
            source_label,
            age_words(age).dimmed()
        );
    }
    println!();
}

pub fn print_history_plain(callsign: &str, events: &[(u64, String)]) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    println!("Callsign: {callsign}");
    println!("Lookups:  {}", events.len());
    for (ts, source) in events {
        let date = unix_to_date(*ts);
        let time = unix_to_time(*ts);
        let age = now.saturating_sub(*ts);
        println!("{date} {time}  {source}  {}", age_words(age));
    }
}

// ── Error output ──────────────────────────────────────────────────────────────

pub fn print_error(callsign: &str, message: &str) {
    eprintln!(
        "\n  {} {}\n  {}\n",
        "✗".bright_red().bold(),
        format!("Could not look up {callsign}").bold(),
        message.dimmed()
    );
}

#[cfg(test)]
mod tests {
    use super::{
        age_words, cache_info_label, ttl_words, unix_to_date, unix_to_time, visible_width,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    // ── visible_width ─────────────────────────────────────────────────────────

    #[test]
    fn plain_text_width_is_char_count() {
        assert_eq!(visible_width("W1AW"), 4);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn ansi_color_codes_are_not_counted() {
        let styled = "\x1b[1m\x1b[96mW1AW\x1b[39m\x1b[0m";
        assert_eq!(visible_width(styled), 4);
    }

    #[test]
    fn osc8_hyperlink_counts_only_visible_label() {
        let link = "\x1b]8;;https://example.com/very/long/path\x1b\\click\x1b]8;;\x1b\\";
        assert_eq!(visible_width(link), "click".len());
    }

    // ── unix_to_date ──────────────────────────────────────────────────────────

    #[test]
    fn unix_epoch_is_1970_01_01() {
        assert_eq!(unix_to_date(0), "1970-01-01");
    }

    #[test]
    fn known_timestamp_round_trips() {
        // 2001-09-09 01:46:40 UTC (the "Unix billennium")
        assert_eq!(unix_to_date(1_000_000_000), "2001-09-09");
    }

    #[test]
    fn leap_year_feb_29_is_correct() {
        // 2000-02-29 00:00:00 UTC = 951782400
        assert_eq!(unix_to_date(951_782_400), "2000-02-29");
    }

    #[test]
    fn non_leap_year_mar_1_is_correct() {
        // 2001-03-01 00:00:00 UTC = 983404800
        assert_eq!(unix_to_date(983_404_800), "2001-03-01");
    }

    #[test]
    fn year_boundary_new_year() {
        // 2026-01-01 00:00:00 UTC = 1767225600
        assert_eq!(unix_to_date(1_767_225_600), "2026-01-01");
    }

    // ── age_words / ttl_words ─────────────────────────────────────────────────

    #[test]
    fn age_words_just_now() {
        assert_eq!(age_words(30), "just now");
        assert_eq!(age_words(119), "just now");
    }

    #[test]
    fn age_words_minutes() {
        assert_eq!(age_words(300), "5 min ago");
        assert_eq!(age_words(3599), "59 min ago");
    }

    #[test]
    fn age_words_hours() {
        assert_eq!(age_words(3_600), "1 hr ago");
        assert_eq!(age_words(7_200), "2 hr ago");
    }

    #[test]
    fn age_words_days() {
        assert_eq!(age_words(172_800), "2 days ago");
        assert_eq!(age_words(7 * 86_400), "7 days ago");
    }

    #[test]
    fn ttl_words_minutes() {
        assert_eq!(ttl_words(1_800), "30 min");
    }

    #[test]
    fn ttl_words_hours() {
        assert_eq!(ttl_words(7_200), "2 hr");
    }

    #[test]
    fn ttl_words_days() {
        assert_eq!(ttl_words(4 * 86_400), "4 days");
    }

    // ── cache_info_label ──────────────────────────────────────────────────────

    #[test]
    fn cache_info_label_contains_date_and_age_and_expiry() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cached_3_days_ago = now - 3 * 86_400;
        let ttl = 7 * 24 * 3_600; // 7 days

        let label = cache_info_label(cached_3_days_ago, ttl);
        assert!(label.contains("3 days ago"), "label: {label}");
        assert!(label.contains("4 days"), "label: {label}"); // refreshes in 4 days
        assert!(label.contains('-'), "label should contain date: {label}");
    }

    // ── unix_to_time ──────────────────────────────────────────────────────────

    #[test]
    fn unix_to_time_midnight() {
        assert_eq!(unix_to_time(0), "00:00");
        assert_eq!(unix_to_time(86_400), "00:00"); // next day's midnight
    }

    #[test]
    fn unix_to_time_noon() {
        assert_eq!(unix_to_time(43_200), "12:00");
    }

    #[test]
    fn unix_to_time_end_of_day() {
        assert_eq!(unix_to_time(86_340), "23:59"); // one minute before midnight
    }

    #[test]
    fn unix_to_time_known_timestamp() {
        // 2001-09-09 01:46:40 UTC = 1_000_000_000
        assert_eq!(unix_to_time(1_000_000_000), "01:46");
    }

    #[test]
    fn cache_info_label_shows_correct_date() {
        // Use the Unix billennium (2001-09-09) as a fixed cached_at.
        let label = cache_info_label(1_000_000_000, 7 * 86_400);
        assert!(
            label.starts_with("2001-09-09"),
            "label should start with date: {label}"
        );
    }
}
