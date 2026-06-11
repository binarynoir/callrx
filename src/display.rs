use crate::api::CallsignRecord;
use crate::hyperlink;
use anstream::{eprintln, println};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::time::Duration;

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

pub fn print_pretty(record: &CallsignRecord, links_enabled: bool) {
    // Detect OSC 8 support — respect the caller's override flag
    let use_links = links_enabled && hyperlink::osc8_supported();

    let callsign = record.callsign();
    let name = record.name.as_deref().unwrap_or("—");
    let rows = build_rows(record, use_links);

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
    println!();
}

/// Builds the `(label, styled value)` rows shown in the pretty table.
fn build_rows(record: &CallsignRecord, use_links: bool) -> Vec<(&'static str, String)> {
    let expired = record.is_expired();
    let mut rows: Vec<(&'static str, String)> = Vec::new();

    // Status
    let status_str = record.status.as_str();
    let status_val = if status_str == "VALID" && !expired {
        "✓ VALID".bright_green().bold().to_string()
    } else if expired {
        "✗ EXPIRED".bright_red().bold().to_string()
    } else {
        format!("⚠ {status_str}").bright_yellow().bold().to_string()
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
    if let Some(trustee) = &record.trustee
        && let Some(tcall) = trustee.callsign.as_deref().filter(|s| !s.is_empty())
    {
        let tname = trustee.name.as_deref().unwrap_or("");
        rows.push((
            "Trustee",
            format!("{} — {tname}", tcall.bright_cyan().bold()),
        ));
    }

    // Previous callsign
    if let Some(prev) = &record.previous
        && let Some(pc) = prev.callsign.as_deref().filter(|s| !s.is_empty())
    {
        rows.push(("Previous", pc.dimmed().to_string()));
    }

    // Address
    if let Some(addr) = &record.address {
        if let Some(line1) = addr.line1.as_deref().filter(|s| !s.is_empty()) {
            rows.push(("Address", line1.to_string()));
        }
        if let Some(line2) = addr.line2.as_deref().filter(|s| !s.is_empty()) {
            rows.push(("", line2.to_string()));
        }
    }

    // Grid square + coordinates
    if let Some(loc) = &record.location
        && let Some(grid) = loc.gridsquare.as_deref().filter(|s| !s.is_empty())
    {
        let lat = loc.latitude.as_deref().unwrap_or("");
        let lon = loc.longitude.as_deref().unwrap_or("");
        let grid_display = if !lat.is_empty() && !lon.is_empty() {
            let maps_url = format!("https://www.google.com/maps/search/?api=1&query={lat},{lon}");
            hyperlink::link(&maps_url, &format!("{grid} ({lat}, {lon})"), use_links)
        } else {
            grid.to_string()
        };
        rows.push(("Grid", grid_display));
    }

    // Dates and identifiers
    if let Some(info) = &record.other_info {
        if let Some(g) = info.grant_date.as_deref().filter(|s| !s.is_empty()) {
            rows.push(("Granted", g.to_string()));
        }
        if let Some(e) = info.expiry_date.as_deref().filter(|s| !s.is_empty()) {
            let expiry_val = if expired {
                e.bright_red().to_string()
            } else {
                e.bright_green().to_string()
            };
            rows.push(("Expires", expiry_val));
        }
        if let Some(d) = info.last_action_date.as_deref().filter(|s| !s.is_empty()) {
            rows.push(("Last Action", d.dimmed().to_string()));
        }
        if let Some(frn) = info.frn.as_deref().filter(|s| !s.is_empty()) {
            rows.push(("FRN", frn.dimmed().to_string()));
        }
        // ULS link — clickable if supported
        if let Some(uls_url) = info.uls_url.as_deref().filter(|s| !s.is_empty()) {
            rows.push(("ULS Record", hyperlink::link(uls_url, uls_url, use_links)));
        }
    }

    // Callook.info source link
    let callook_url = format!("https://callook.info/{}", record.callsign());
    rows.push((
        "callook.info",
        hyperlink::link(&callook_url, &callook_url, use_links),
    ));

    rows
}

// ── Plain text output (--raw) ─────────────────────────────────────────────────

pub fn print_plain(record: &CallsignRecord) {
    println!("Callsign:    {}", record.callsign());
    println!("Status:      {}", record.status);

    if let Some(t) = &record.license_type {
        println!("Type:        {t}");
    }
    println!("Class:       {}", record.license_class_label());

    if let Some(name) = &record.name {
        println!("Name:        {name}");
    }

    if let Some(trustee) = &record.trustee
        && let (Some(tc), Some(tn)) = (&trustee.callsign, &trustee.name)
        && !tc.is_empty()
    {
        println!("Trustee:     {tn} ({tc})");
    }

    if let Some(addr) = &record.address {
        if let Some(l1) = &addr.line1 {
            println!("Address:     {l1}");
        }
        if let Some(l2) = &addr.line2 {
            println!("             {l2}");
        }
    }

    if let Some(loc) = &record.location {
        if let Some(g) = &loc.gridsquare {
            println!("Grid:        {g}");
        }
        if let (Some(lat), Some(lon)) = (&loc.latitude, &loc.longitude) {
            println!("Coordinates: {lat}, {lon}");
        }
    }

    if let Some(info) = &record.other_info {
        if let Some(g) = &info.grant_date {
            println!("Granted:     {g}");
        }
        if let Some(e) = &info.expiry_date {
            println!("Expires:     {e}");
        }
        if let Some(uls) = &info.uls_url {
            println!("ULS URL:     {uls}");
        }
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
    use super::visible_width;

    #[test]
    fn plain_text_width_is_char_count() {
        assert_eq!(visible_width("W1AW"), 4);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn ansi_color_codes_are_not_counted() {
        // "\x1b[1m\x1b[96mW1AW\x1b[39m\x1b[0m" renders as 4 columns.
        let styled = "\x1b[1m\x1b[96mW1AW\x1b[39m\x1b[0m";
        assert_eq!(visible_width(styled), 4);
    }

    #[test]
    fn osc8_hyperlink_counts_only_visible_label() {
        // The URL inside the OSC 8 escape must not inflate the width.
        let link = "\x1b]8;;https://example.com/very/long/path\x1b\\click\x1b]8;;\x1b\\";
        assert_eq!(visible_width(link), "click".len());
    }
}
