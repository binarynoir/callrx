use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize};

/// Top-level response from callook.info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallsignRecord {
    pub status: String,

    #[serde(rename = "type")]
    pub license_type: Option<String>,

    pub current: Option<CurrentInfo>,
    pub previous: Option<PreviousInfo>,
    pub trustee: Option<TrusteeInfo>,
    pub name: Option<String>,
    pub address: Option<AddressInfo>,
    pub location: Option<LocationInfo>,

    #[serde(rename = "otherInfo")]
    pub other_info: Option<OtherInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentInfo {
    pub callsign: Option<String>,

    #[serde(rename = "operClass")]
    pub oper_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviousInfo {
    pub callsign: Option<String>,

    #[serde(rename = "operClass")]
    pub oper_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrusteeInfo {
    pub callsign: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInfo {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub attn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub gridsquare: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherInfo {
    #[serde(rename = "grantDate")]
    pub grant_date: Option<String>,

    #[serde(rename = "expiryDate")]
    pub expiry_date: Option<String>,

    #[serde(rename = "lastActionDate")]
    pub last_action_date: Option<String>,

    pub frn: Option<String>,

    #[serde(rename = "ulsUrl")]
    pub uls_url: Option<String>,
}

/// Fetch a callsign record from callook.info
pub fn lookup_callsign(callsign: &str) -> Result<CallsignRecord> {
    let url = format!("https://callook.info/{callsign}/json");

    let response = minreq::get(&url)
        .with_header("User-Agent", concat!("callrx/", env!("CARGO_PKG_VERSION")))
        .with_timeout(10)
        .send()
        .map_err(|e| eyre!("Network error: {e}"))?;

    if response.status_code != 200 {
        return Err(eyre!(
            "HTTP {} from callook.info — the service may be temporarily unavailable.",
            response.status_code
        ));
    }

    let record: CallsignRecord = response
        .json()
        .map_err(|e| eyre!("Failed to parse response: {e}"))?;

    if record.status == "INVALID" {
        return Err(eyre!(
            "Callsign '{callsign}' was not found in the FCC ULS database."
        ));
    }

    Ok(record)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

impl CallsignRecord {
    /// Returns the canonical callsign from the record
    pub fn callsign(&self) -> &str {
        self.current
            .as_ref()
            .and_then(|c| c.callsign.as_deref())
            .unwrap_or("—")
    }

    /// Returns the human-readable license class.
    ///
    /// callook.info reports `operClass` as a full word (e.g. `"EXTRA"`); the
    /// single-letter FCC codes are also accepted for robustness.
    pub fn license_class_label(&self) -> &'static str {
        let code = self
            .current
            .as_ref()
            .and_then(|c| c.oper_class.as_deref())
            .unwrap_or("");
        match code.to_ascii_uppercase().as_str() {
            "E" | "EXTRA" => "Amateur Extra",
            "A" | "ADVANCED" => "Advanced",
            "G" | "GENERAL" => "General",
            "T" | "TECHNICIAN" | "TECHNICIAN PLUS" => "Technician",
            "N" | "NOVICE" => "Novice",
            "" => "—",
            _ => "Unknown",
        }
    }

    /// Returns true if the license appears to be expired.
    ///
    /// This is a rough day-count comparison (leap years approximated) so we can
    /// avoid a heavyweight date dependency. It is a display hint only.
    pub fn is_expired(&self) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};

        let expiry = self
            .other_info
            .as_ref()
            .and_then(|o| o.expiry_date.as_deref())
            .unwrap_or("");

        // Parse MM/DD/YYYY; bail out silently on anything unexpected.
        let [month, day, year] = match expiry.split('/').collect::<Vec<_>>().as_slice() {
            [m, d, y] => [*m, *d, *y],
            _ => return false,
        };
        let (Ok(month), Ok(day), Ok(year)) = (
            month.parse::<u64>(),
            day.parse::<u64>(),
            year.parse::<i64>(),
        ) else {
            return false;
        };
        let Ok(years_since_epoch) = u64::try_from(year - 1970) else {
            return false; // year before the epoch — treat as not-expired
        };

        let now_days = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / 86_400;

        let month_index = usize::try_from(month.saturating_sub(1)).unwrap_or(0);
        let cumulative_month_days = [0u64, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
            .get(month_index)
            .copied()
            .unwrap_or(0);
        let expiry_days =
            years_since_epoch * 365 + years_since_epoch / 4 + cumulative_month_days + day;

        now_days > expiry_days
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(oper_class: Option<&str>, expiry: Option<&str>) -> CallsignRecord {
        CallsignRecord {
            status: "VALID".to_string(),
            license_type: None,
            current: Some(CurrentInfo {
                callsign: Some("W1AW".to_string()),
                oper_class: oper_class.map(str::to_string),
            }),
            previous: None,
            trustee: None,
            name: None,
            address: None,
            location: None,
            other_info: expiry.map(|e| OtherInfo {
                grant_date: None,
                expiry_date: Some(e.to_string()),
                last_action_date: None,
                frn: None,
                uls_url: None,
            }),
        }
    }

    #[test]
    fn class_label_decodes_full_words_and_letters() {
        assert_eq!(
            record_with(Some("EXTRA"), None).license_class_label(),
            "Amateur Extra"
        );
        assert_eq!(
            record_with(Some("E"), None).license_class_label(),
            "Amateur Extra"
        );
        assert_eq!(
            record_with(Some("general"), None).license_class_label(),
            "General"
        );
        assert_eq!(
            record_with(Some("TECHNICIAN"), None).license_class_label(),
            "Technician"
        );
    }

    #[test]
    fn class_label_handles_missing_and_unknown() {
        assert_eq!(record_with(None, None).license_class_label(), "—");
        assert_eq!(record_with(Some(""), None).license_class_label(), "—");
        assert_eq!(
            record_with(Some("BOGUS"), None).license_class_label(),
            "Unknown"
        );
    }

    #[test]
    fn expiry_in_the_past_is_expired() {
        assert!(record_with(None, Some("01/01/2000")).is_expired());
    }

    #[test]
    fn expiry_far_in_the_future_is_not_expired() {
        assert!(!record_with(None, Some("01/01/2099")).is_expired());
    }

    #[test]
    fn malformed_or_missing_expiry_is_not_expired() {
        assert!(!record_with(None, None).is_expired());
        assert!(!record_with(None, Some("")).is_expired());
        assert!(!record_with(None, Some("not-a-date")).is_expired());
        assert!(!record_with(None, Some("13/40/abcd")).is_expired());
        assert!(!record_with(None, Some("01/01/1969")).is_expired());
    }

    #[test]
    fn callsign_falls_back_when_absent() {
        let mut rec = record_with(None, None);
        assert_eq!(rec.callsign(), "W1AW");
        rec.current = None;
        assert_eq!(rec.callsign(), "—");
    }
}
