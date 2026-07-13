use color_eyre::{Result, eyre::eyre};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::config::{api_base_url, api_key};

/// A callsign license record from the callrx-service API.
///
/// This mirrors the subset of the service's `CallsignResponse` that the CLI
/// displays and caches. The service returns a few additional fields (raw
/// first/last/mi/suffix name parts); `serde` ignores anything not declared
/// here since `display_name` already composes them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallsignRecord {
    pub unique_system_identifier: Option<i64>,
    pub call_sign: Option<String>,
    pub display_name: String,

    /// `Individual` or `Club`.
    pub license_type: Option<String>,
    /// FCC status code: `A`, `C`, `E`, `T`, `L`.
    pub license_status: String,
    /// Human-readable status, e.g. `Active`, `Expired`.
    pub license_status_label: String,

    /// FCC class code (`E`/`A`/`G`/`T`/`N`); null for club licenses.
    pub operator_class: Option<String>,
    /// Full operator-class name, e.g. `Amateur Extra` (null for club licenses).
    pub operator_class_label: Option<String>,
    /// FCC callsign group (A=Extra, B=Advanced, C=General, D=Technician/Novice).
    pub group_code: Option<String>,
    /// FCC geographic call district (1–10, or K/N/S for territories).
    pub region_code: Option<String>,

    pub previous_callsign: Option<String>,
    /// FCC class code before upgrade.
    pub previous_operator_class: Option<String>,
    pub previous_operator_class_label: Option<String>,

    pub trustee_callsign: Option<String>,
    pub trustee_name: Option<String>,
    /// `Y` if the callsign was obtained via the FCC vanity program.
    pub vanity_call_sign_change: Option<String>,
    /// `P`=previous holder, `R`=close relative, `S`=surviving spouse.
    pub vanity_relationship: Option<String>,

    #[serde(default)]
    pub address: AddressInfo,
    pub email: Option<String>,
    pub phone: Option<String>,

    pub frn: Option<String>,

    /// ISO 8601 dates (`YYYY-MM-DD`).
    pub grant_date: Option<String>,
    pub expired_date: Option<String>,
    pub cancellation_date: Option<String>,
    pub effective_date: Option<String>,
    pub last_action_date: Option<String>,

    pub uls_url: Option<String>,

    /// Radio service code: `A`=Amateur Radio, `G`=GMRS.
    pub service: Option<String>,
    pub service_label: Option<String>,

    /// Other licenses held by the same FCC registrant (same FRN).
    pub frn_licenses: Option<Vec<CallsignSummary>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressInfo {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip_code: Option<String>,
    pub po_box: Option<String>,
    /// Approximate — the centroid of the mailing ZIP code (US Census ZCTA
    /// Gazetteer), not the exact street address.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl AddressInfo {
    /// Derives a 6-character Maidenhead grid square from `latitude`/`longitude`,
    /// or `None` when either is absent. The service doesn't compute or return
    /// this itself — it's a well-known, purely local formula, so it's derived
    /// client-side rather than duplicated server-side.
    pub fn grid_square(&self) -> Option<String> {
        let (lat, lon) = (self.latitude?, self.longitude?);
        let lon = (lon + 180.0).rem_euclid(360.0);
        let lat = (lat + 90.0).rem_euclid(180.0);

        let field_lon = (lon / 20.0) as u8;
        let field_lat = (lat / 10.0) as u8;
        let square_lon = ((lon % 20.0) / 2.0) as u8;
        let square_lat = (lat % 10.0) as u8;
        let sub_lon = (((lon % 20.0) % 2.0) / (2.0 / 24.0)) as u8;
        let sub_lat = (((lat % 10.0) % 1.0) / (1.0 / 24.0)) as u8;

        Some(format!(
            "{}{}{}{}{}{}",
            (b'A' + field_lon) as char,
            (b'A' + field_lat) as char,
            square_lon,
            square_lat,
            (b'a' + sub_lon) as char,
            (b'a' + sub_lat) as char,
        ))
    }
}

/// Compact record used for FRN siblings and neighbors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallsignSummary {
    pub unique_system_identifier: Option<i64>,
    pub call_sign: Option<String>,
    pub display_name: String,
    pub license_status_label: String,
    pub license_type: Option<String>,
    pub operator_class_label: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub service: Option<String>,
    pub service_label: Option<String>,
}

/// A [`CallsignSummary`] plus the street address, used in neighbors results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborSummary {
    pub unique_system_identifier: Option<i64>,
    pub call_sign: Option<String>,
    pub display_name: String,
    pub license_status_label: String,
    pub license_type: Option<String>,
    pub operator_class_label: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub service: Option<String>,
    pub service_label: Option<String>,
    pub street_address: Option<String>,
}

/// Response from `GET /callsign/{callsign}/neighbors` — other active licensees
/// near the queried callsign's mailing address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborsResponse {
    pub call_sign: String,
    pub address_count: i64,
    pub address_results: Vec<NeighborSummary>,
    pub street_count: i64,
    pub street_results: Vec<NeighborSummary>,
}

/// Weather fields relevant to amateur radio operation, mirroring what the
/// web front-end renders (a subset of the service's fuller `WeatherInfo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeatherInfo {
    pub temperature_c: Option<f64>,
    pub temperature_f: Option<f64>,
    pub humidity_pct: Option<i64>,
    pub wind_speed_mph: Option<f64>,
    pub wind_direction_label: Option<String>,
    pub wind_gusts_mph: Option<f64>,
    pub pressure_hpa: Option<f64>,
    pub precipitation_mm: Option<f64>,
    pub cloud_cover_pct: Option<i64>,
    pub conditions: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocationTimeInfo {
    pub local_time: Option<String>,
    pub utc_time: Option<String>,
    pub timezone: Option<String>,
    pub utc_offset_seconds: Option<i64>,
}

/// A single active NWS weather alert covering the licensee's location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherAlert {
    pub event: Option<String>,
    /// NWS severity: `Extreme`, `Severe`, `Moderate`, `Minor`, or `Unknown`.
    pub severity: Option<String>,
    pub urgency: Option<String>,
    pub headline: Option<String>,
    pub effective: Option<String>,
    pub expires: Option<String>,
    pub area_desc: Option<String>,
}

/// Response from `GET /callsign/{callsign}/location-info` — local time,
/// weather, and active alerts at the licensee's mailing address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocationInfoResponse {
    pub call_sign: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub time: Option<LocationTimeInfo>,
    pub weather: Option<WeatherInfo>,
    /// Active NWS alerts (empty when none are active; `None` only if the
    /// alerts fetch itself failed server-side).
    pub alerts: Option<Vec<WeatherAlert>>,
}

/// A single amateur radio frequency segment from 47 CFR § 97.301.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandSegment {
    pub band: String,
    pub freq_low_khz: f64,
    /// `None` for the open-ended top band (above 275 GHz).
    pub freq_high_khz: Option<f64>,
    /// Minimum FCC operator class code authorized (N/T/G/A/E).
    pub min_operator_class: String,
    pub min_operator_class_label: String,
    pub cfr_paragraph: String,
}

/// A single GMRS channel from 47 CFR § 95.1763.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmrsChannel {
    pub frequency_mhz: f64,
    /// `462_main`, `462_interstitial`, `467_main`, or `467_interstitial`.
    pub channel_type: String,
    pub max_power_watts: f64,
    /// Offset in kHz to the paired repeater-input channel; `None` for
    /// interstitial channels, which have no repeater pairing.
    pub repeater_offset_khz: Option<i64>,
    pub notes: String,
}

/// Response from `GET /bandplan` — static amateur/GMRS frequency allocation
/// reference data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BandPlanResponse {
    /// `None` when `service=G` was requested.
    pub amateur_bands: Option<Vec<BandSegment>>,
    /// `None` when `service=A` was requested.
    pub gmrs_channels: Option<Vec<GmrsChannel>>,
    #[serde(default)]
    pub source: String,
}

fn get_json<T: DeserializeOwned>(path: &str, not_found: &str) -> Result<T> {
    let url = format!("{}{path}", api_base_url());

    let mut request = minreq::get(&url)
        .with_header("User-Agent", concat!("callrx/", env!("CARGO_PKG_VERSION")))
        .with_timeout(10);

    if let Some(key) = api_key() {
        request = request.with_header("X-API-Key", key);
    }

    let response = request.send().map_err(|e| eyre!("Network error: {e}"))?;

    match response.status_code {
        200 => {}
        401 => {
            return Err(eyre!(
                "Unauthorized — run `callrx auth login` to sign in, or set CALLRX_API_KEY to a \
                 valid callrx-service API key."
            ));
        }
        403 => {
            return Err(eyre!(
                "Forbidden — this API key has been suspended or revoked."
            ));
        }
        404 => return Err(eyre!("{not_found}")),
        429 => {
            return Err(eyre!(
                "Rate limit exceeded — please wait a moment and try again."
            ));
        }
        code => {
            return Err(eyre!(
                "HTTP {code} from the callrx-service API — the service may be temporarily unavailable."
            ));
        }
    }

    response
        .json()
        .map_err(|e| eyre!("Failed to parse response: {e}"))
}

/// Fetch a callsign record from the callrx-service backend.
pub fn lookup_callsign(callsign: &str) -> Result<CallsignRecord> {
    get_json(
        &format!("/callsign/{callsign}"),
        &format!("Callsign '{callsign}' was not found in the FCC ULS database."),
    )
}

/// Fetch other active licensees near the callsign's mailing address.
pub fn lookup_neighbors(callsign: &str) -> Result<NeighborsResponse> {
    get_json(
        &format!("/callsign/{callsign}/neighbors"),
        &format!("Callsign '{callsign}' was not found in the FCC ULS database."),
    )
}

/// Fetch local time and weather at the callsign's mailing address.
pub fn lookup_location_info(callsign: &str) -> Result<LocationInfoResponse> {
    get_json(
        &format!("/callsign/{callsign}/location-info"),
        &format!("Callsign '{callsign}' was not found in the FCC ULS database."),
    )
}

/// Fetch the static amateur/GMRS band plan reference data. `service` restricts
/// the result to `"A"` (amateur only) or `"G"` (GMRS only); `None` returns both.
pub fn lookup_bandplan(service: Option<&str>) -> Result<BandPlanResponse> {
    let path = match service {
        Some(s) => format!("/bandplan?service={s}"),
        None => "/bandplan".to_string(),
    };
    get_json(&path, "Band plan data was not found.")
}

// ── Device-authorization login (`callrx auth login`) ────────────────────────

/// Response from `POST /auth/device/code` — the start of a device login.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Returned by `POST /auth/device/token` once the login has been approved —
/// the CLI's one-time look at its new key, same as a manual `POST /keys`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceTokenResponse {
    pub api_key: String,
    pub key_prefix: String,
    pub tier: String,
}

/// The signed-in key's current quota status — `GET /keys/usage`.
#[derive(Debug, Clone, Deserialize)]
pub struct KeyUsageResponse {
    pub tier: String,
    pub requests_per_day: i64,
    pub used_today: i64,
    pub remaining_today: i64,
    pub reset_at: String,
    pub requests_per_minute: i64,
    pub used_this_minute: i64,
    pub remaining_this_minute: i64,
    pub in_grace_period: bool,
}

/// Outcome of one `POST /auth/device/token` poll.
pub enum DevicePollOutcome {
    Pending,
    SlowDown { new_interval_secs: u64 },
    Denied(String),
    Expired,
    Success(DeviceTokenResponse),
}

/// Start a device login: `POST /auth/device/code`. Unauthenticated — no
/// `X-API-Key` is attached, since signing in is how one is obtained.
pub fn start_device_login() -> Result<DeviceCodeResponse> {
    let url = format!("{}/auth/device/code", api_base_url());
    let response = minreq::post(&url)
        .with_header("User-Agent", concat!("callrx/", env!("CARGO_PKG_VERSION")))
        .with_timeout(10)
        .send()
        .map_err(|e| eyre!("Network error: {e}"))?;

    if response.status_code != 200 {
        return Err(eyre!(
            "HTTP {} from the callrx-service API while starting login.",
            response.status_code
        ));
    }
    response
        .json()
        .map_err(|e| eyre!("Failed to parse response: {e}"))
}

/// Maps a `POST /auth/device/token` error body (RFC 8628 vocabulary in
/// `detail.error`) to an outcome. Pure and infallible — an unparseable or
/// unrecognized body degrades to `Expired` rather than panicking, since the
/// caller can't distinguish "give up" from "keep polling" any other safe way.
fn parse_device_error(body: &str) -> DevicePollOutcome {
    #[derive(Deserialize)]
    struct ErrorBody {
        detail: ErrorDetail,
    }
    #[derive(Deserialize)]
    struct ErrorDetail {
        error: String,
        interval: Option<u64>,
        message: Option<String>,
    }

    let Ok(parsed) = serde_json::from_str::<ErrorBody>(body) else {
        return DevicePollOutcome::Expired;
    };

    match parsed.detail.error.as_str() {
        "authorization_pending" => DevicePollOutcome::Pending,
        "slow_down" => DevicePollOutcome::SlowDown {
            new_interval_secs: parsed.detail.interval.unwrap_or(10),
        },
        "expired_token" => DevicePollOutcome::Expired,
        "access_denied" => DevicePollOutcome::Denied(
            parsed
                .detail
                .message
                .unwrap_or_else(|| "Login was denied.".to_string()),
        ),
        _ => DevicePollOutcome::Expired,
    }
}

/// Poll `POST /auth/device/token` once. Unauthenticated — the device_code
/// itself is the credential.
pub fn poll_device_token(device_code: &str) -> Result<DevicePollOutcome> {
    let url = format!("{}/auth/device/token", api_base_url());
    let body = serde_json::json!({ "device_code": device_code });
    let response = minreq::post(&url)
        .with_header("User-Agent", concat!("callrx/", env!("CARGO_PKG_VERSION")))
        .with_timeout(10)
        .with_json(&body)
        .map_err(|e| eyre!("Failed to build request: {e}"))?
        .send()
        .map_err(|e| eyre!("Network error: {e}"))?;

    if response.status_code == 200 {
        return response
            .json::<DeviceTokenResponse>()
            .map(DevicePollOutcome::Success)
            .map_err(|e| eyre!("Failed to parse response: {e}"));
    }

    Ok(parse_device_error(response.as_str().unwrap_or_default()))
}

/// Fetch the signed-in key's quota status — `GET /keys/usage`. Reuses
/// `get_json`, which already attaches `X-API-Key` from `config::api_key()`.
pub fn lookup_key_usage() -> Result<KeyUsageResponse> {
    get_json(
        "/keys/usage",
        "No active API key — run `callrx auth login`.",
    )
}

// ── Helpers ──────────────────────────────────────────────────────────────────

impl CallsignRecord {
    /// Returns the canonical callsign from the record.
    pub fn callsign(&self) -> &str {
        self.call_sign.as_deref().unwrap_or("—")
    }

    /// Returns the human-readable license class.
    ///
    /// The service already emits full operator-class names (`Amateur Extra`,
    /// `General`, …); this returns it verbatim, or `—` when absent (e.g. club
    /// licenses, which have a trustee instead of an operator class).
    pub fn license_class_label(&self) -> &str {
        self.operator_class_label.as_deref().unwrap_or("—")
    }

    /// Returns true if the license is expired, per the FCC status code.
    pub fn is_expired(&self) -> bool {
        self.license_status == "E"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(class_label: Option<&str>, status: &str) -> CallsignRecord {
        CallsignRecord {
            call_sign: Some("W1AW".to_string()),
            display_name: "ARRL HQ OPERATORS CLUB".to_string(),
            license_type: Some("Club".to_string()),
            license_status: status.to_string(),
            license_status_label: "Active".to_string(),
            operator_class_label: class_label.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn class_label_returns_service_label_verbatim() {
        assert_eq!(
            record_with(Some("Amateur Extra"), "A").license_class_label(),
            "Amateur Extra"
        );
        assert_eq!(
            record_with(Some("General"), "A").license_class_label(),
            "General"
        );
    }

    #[test]
    fn class_label_handles_missing() {
        assert_eq!(record_with(None, "A").license_class_label(), "—");
    }

    #[test]
    fn is_expired_follows_status_code() {
        assert!(record_with(None, "E").is_expired());
        assert!(!record_with(None, "A").is_expired());
        assert!(!record_with(None, "C").is_expired());
    }

    #[test]
    fn callsign_falls_back_when_absent() {
        let mut rec = record_with(None, "A");
        assert_eq!(rec.callsign(), "W1AW");
        rec.call_sign = None;
        assert_eq!(rec.callsign(), "—");
    }

    // ── AddressInfo::grid_square ─────────────────────────────────────────────

    #[test]
    fn grid_square_none_when_coords_missing() {
        let addr = AddressInfo::default();
        assert_eq!(addr.grid_square(), None);
    }

    #[test]
    fn grid_square_newington_ct() {
        // W1AW's mailing address — well-known reference point, FN31.
        let addr = AddressInfo {
            latitude: Some(41.686764),
            longitude: Some(-72.730593),
            ..Default::default()
        };
        let grid = addr.grid_square().unwrap();
        assert!(grid.starts_with("FN31"), "grid: {grid}");
        assert_eq!(grid.len(), 6);
    }

    #[test]
    fn grid_square_southern_hemisphere() {
        // Sydney, Australia — negative lat/lon exercise the rem_euclid wrap.
        let addr = AddressInfo {
            latitude: Some(-33.8688),
            longitude: Some(151.2093),
            ..Default::default()
        };
        let grid = addr.grid_square().unwrap();
        assert!(grid.starts_with("QF56"), "grid: {grid}");
    }

    // ── parse_device_error ───────────────────────────────────────────────────

    #[test]
    fn parse_device_error_authorization_pending() {
        let body = r#"{"detail": {"error": "authorization_pending"}}"#;
        assert!(matches!(
            parse_device_error(body),
            DevicePollOutcome::Pending
        ));
    }

    #[test]
    fn parse_device_error_slow_down_carries_new_interval() {
        let body = r#"{"detail": {"error": "slow_down", "interval": 15}}"#;
        match parse_device_error(body) {
            DevicePollOutcome::SlowDown { new_interval_secs } => {
                assert_eq!(new_interval_secs, 15);
            }
            _ => panic!("expected SlowDown"),
        }
    }

    #[test]
    fn parse_device_error_expired_token() {
        let body = r#"{"detail": {"error": "expired_token"}}"#;
        assert!(matches!(
            parse_device_error(body),
            DevicePollOutcome::Expired
        ));
    }

    #[test]
    fn parse_device_error_access_denied_carries_message() {
        let body = r#"{"detail": {"error": "access_denied", "message": "Login was denied in the browser"}}"#;
        match parse_device_error(body) {
            DevicePollOutcome::Denied(msg) => assert_eq!(msg, "Login was denied in the browser"),
            _ => panic!("expected Denied"),
        }
    }

    #[test]
    fn parse_device_error_unparseable_body_degrades_to_expired() {
        assert!(matches!(
            parse_device_error("not json"),
            DevicePollOutcome::Expired
        ));
    }

    #[test]
    fn parse_device_error_unrecognized_error_degrades_to_expired() {
        let body = r#"{"detail": {"error": "something_new"}}"#;
        assert!(matches!(
            parse_device_error(body),
            DevicePollOutcome::Expired
        ));
    }
}
