use color_eyre::{Result, eyre::eyre};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::{AddressInfo, CallsignRecord};

const SCHEMA_VERSION: i32 = 2;

/// Cache TTL: 7 days, matching the FCC's weekly ULS bulk-data publication.
pub const TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Maps an empty string to `None` so blank API fields are stored as SQL NULL.
fn nonempty(s: Option<&str>) -> Option<&str> {
    s.filter(|v| !v.is_empty())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn db_path() -> Option<PathBuf> {
    // CALLRX_CACHE_DIR overrides the default system cache directory.
    // Set via .env (loaded in debug builds) to redirect to target/cache/ during dev.
    if let Ok(dir) = std::env::var("CALLRX_CACHE_DIR") {
        return Some(PathBuf::from(dir).join("callrx.db"));
    }
    dirs::cache_dir().map(|d| d.join("callrx").join("callrx.db"))
}

/// Opens (or creates) the cache database and applies the schema if needed.
///
/// Returns an error only when the cache directory cannot be determined or the
/// file cannot be opened. Callers should use `.ok()` and treat `None` as
/// "cache unavailable" rather than a fatal error.
pub fn open() -> Result<Connection> {
    let path = db_path().ok_or_else(|| eyre!("Could not determine cache directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    apply_schema(&conn)?;
    Ok(conn)
}

fn apply_schema(conn: &Connection) -> Result<()> {
    // These pragmas are connection-level and must be set on every open.
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;

    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    // The callsigns table is a disposable cache of API responses. Its columns
    // changed when the backend moved from callook.info to callrx-service, so on
    // any upgrade we drop and recreate it. lookup_history carries no record data
    // and is preserved across the migration.
    conn.execute_batch(
        "DROP TABLE IF EXISTS callsigns;

         CREATE TABLE callsigns (
             id                    INTEGER PRIMARY KEY,
             callsign              TEXT    NOT NULL UNIQUE,
             display_name          TEXT    NOT NULL,
             license_type          TEXT,
             license_status        TEXT    NOT NULL,
             license_status_label  TEXT    NOT NULL,
             operator_class_label  TEXT,
             previous_callsign     TEXT,
             trustee_callsign      TEXT,
             trustee_name          TEXT,
             street                TEXT,
             city                  TEXT,
             state                 TEXT,
             zip_code              TEXT,
             po_box                TEXT,
             frn                   TEXT,
             grant_date            TEXT,
             expired_date          TEXT,
             last_action_date      TEXT,
             uls_url               TEXT,
             cached_at             INTEGER NOT NULL
         );

         CREATE TABLE IF NOT EXISTS lookup_history (
             id           INTEGER PRIMARY KEY,
             callsign     TEXT    NOT NULL,
             looked_up_at INTEGER NOT NULL,
             source       TEXT    NOT NULL CHECK(source IN ('api', 'cache'))
         );

         CREATE INDEX IF NOT EXISTS idx_lookup_history_callsign
             ON lookup_history(callsign);
         CREATE INDEX IF NOT EXISTS idx_lookup_history_looked_up_at
             ON lookup_history(looked_up_at);",
    )?;

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Returns the cached record and its `cached_at` Unix timestamp when a fresh
/// entry (within `TTL_SECS`) exists for `callsign`.
pub fn get(conn: &Connection, callsign: &str) -> Option<(CallsignRecord, u64)> {
    let min_age = now_secs().saturating_sub(TTL_SECS);

    conn.query_row(
        "SELECT display_name, license_type, license_status, license_status_label,
                operator_class_label, previous_callsign,
                trustee_callsign, trustee_name,
                street, city, state, zip_code, po_box,
                frn, grant_date, expired_date, last_action_date, uls_url,
                cached_at
         FROM   callsigns
         WHERE  callsign = ?1 AND cached_at >= ?2",
        params![callsign, min_age as i64],
        |row| {
            let display_name: String = row.get(0)?;
            let license_type: Option<String> = row.get(1)?;
            let license_status: String = row.get(2)?;
            let license_status_label: String = row.get(3)?;
            let operator_class_label: Option<String> = row.get(4)?;
            let previous_callsign: Option<String> = row.get(5)?;
            let trustee_callsign: Option<String> = row.get(6)?;
            let trustee_name: Option<String> = row.get(7)?;
            let street: Option<String> = row.get(8)?;
            let city: Option<String> = row.get(9)?;
            let state: Option<String> = row.get(10)?;
            let zip_code: Option<String> = row.get(11)?;
            let po_box: Option<String> = row.get(12)?;
            let frn: Option<String> = row.get(13)?;
            let grant_date: Option<String> = row.get(14)?;
            let expired_date: Option<String> = row.get(15)?;
            let last_action_date: Option<String> = row.get(16)?;
            let uls_url: Option<String> = row.get(17)?;
            let cached_at: i64 = row.get(18)?;

            Ok((
                CallsignRecord {
                    call_sign: Some(callsign.to_string()),
                    display_name,
                    license_type,
                    license_status,
                    license_status_label,
                    operator_class_label,
                    previous_callsign,
                    trustee_callsign,
                    trustee_name,
                    address: AddressInfo {
                        street,
                        city,
                        state,
                        zip_code,
                        po_box,
                    },
                    frn,
                    grant_date,
                    expired_date,
                    last_action_date,
                    uls_url,
                },
                cached_at as u64,
            ))
        },
    )
    .optional()
    .ok()
    .flatten()
}

/// Upserts a callsign record into the cache, updating `cached_at` to now.
pub fn store(conn: &Connection, record: &CallsignRecord) -> Result<()> {
    let callsign = record.callsign();
    let now = now_secs() as i64;
    let addr = &record.address;

    conn.execute(
        "INSERT INTO callsigns (
             callsign, display_name, license_type, license_status, license_status_label,
             operator_class_label, previous_callsign, trustee_callsign, trustee_name,
             street, city, state, zip_code, po_box,
             frn, grant_date, expired_date, last_action_date, uls_url, cached_at
         ) VALUES (
             ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20
         )
         ON CONFLICT(callsign) DO UPDATE SET
             display_name         = excluded.display_name,
             license_type         = excluded.license_type,
             license_status       = excluded.license_status,
             license_status_label = excluded.license_status_label,
             operator_class_label = excluded.operator_class_label,
             previous_callsign    = excluded.previous_callsign,
             trustee_callsign     = excluded.trustee_callsign,
             trustee_name         = excluded.trustee_name,
             street               = excluded.street,
             city                 = excluded.city,
             state                = excluded.state,
             zip_code             = excluded.zip_code,
             po_box               = excluded.po_box,
             frn                  = excluded.frn,
             grant_date           = excluded.grant_date,
             expired_date         = excluded.expired_date,
             last_action_date     = excluded.last_action_date,
             uls_url              = excluded.uls_url,
             cached_at            = excluded.cached_at",
        params![
            callsign,
            &record.display_name,
            record.license_type.as_deref(),
            &record.license_status,
            &record.license_status_label,
            record.operator_class_label.as_deref(),
            nonempty(record.previous_callsign.as_deref()),
            nonempty(record.trustee_callsign.as_deref()),
            nonempty(record.trustee_name.as_deref()),
            nonempty(addr.street.as_deref()),
            nonempty(addr.city.as_deref()),
            nonempty(addr.state.as_deref()),
            nonempty(addr.zip_code.as_deref()),
            nonempty(addr.po_box.as_deref()),
            nonempty(record.frn.as_deref()),
            nonempty(record.grant_date.as_deref()),
            nonempty(record.expired_date.as_deref()),
            nonempty(record.last_action_date.as_deref()),
            nonempty(record.uls_url.as_deref()),
            now,
        ],
    )?;
    Ok(())
}

/// Appends a lookup event to the history table.
pub fn record_lookup(conn: &Connection, callsign: &str, source: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO lookup_history (callsign, looked_up_at, source)
         VALUES (?1, ?2, ?3)",
        params![callsign, now_secs() as i64, source],
    )?;
    Ok(())
}

/// Returns all lookup events for `callsign`, most recent first.
/// Each entry is `(looked_up_at_unix_secs, source)`.
pub fn get_history(conn: &Connection, callsign: &str) -> Vec<(u64, String)> {
    let mut stmt = match conn.prepare(
        "SELECT looked_up_at, source FROM lookup_history
         WHERE callsign = ?1 ORDER BY looked_up_at DESC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(params![callsign], |row| {
        let ts: i64 = row.get(0)?;
        let source: String = row.get(1)?;
        Ok((ts as u64, source))
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{SCHEMA_VERSION, TTL_SECS, apply_schema, get, now_secs, record_lookup, store};
    use crate::api::{AddressInfo, CallsignRecord};
    use rusqlite::Connection;

    fn make_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        conn
    }

    fn sample_record(callsign: &str) -> CallsignRecord {
        CallsignRecord {
            call_sign: Some(callsign.to_string()),
            display_name: "TEST OPERATOR".to_string(),
            license_type: Some("Individual".to_string()),
            license_status: "A".to_string(),
            license_status_label: "Active".to_string(),
            operator_class_label: Some("Amateur Extra".to_string()),
            previous_callsign: None,
            trustee_callsign: None,
            trustee_name: None,
            address: AddressInfo {
                street: Some("123 MAIN ST".to_string()),
                city: Some("ANYTOWN".to_string()),
                state: Some("ST".to_string()),
                zip_code: Some("00000".to_string()),
                po_box: None,
            },
            frn: Some("1234567890".to_string()),
            grant_date: Some("2020-01-01".to_string()),
            expired_date: Some("2030-01-01".to_string()),
            last_action_date: Some("2020-01-01".to_string()),
            uls_url: Some("http://example.com".to_string()),
        }
    }

    #[test]
    fn schema_sets_user_version() {
        let conn = make_conn();
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn schema_apply_is_idempotent() {
        let conn = make_conn();
        apply_schema(&conn).unwrap(); // second call must not error
        let v: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn get_returns_none_when_empty() {
        let conn = make_conn();
        assert!(get(&conn, "W1AW").is_none());
    }

    #[test]
    fn store_and_get_round_trip() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        let (rec, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(rec.license_status_label, "Active");
        assert_eq!(rec.callsign(), "W1AW");
        assert_eq!(rec.license_class_label(), "Amateur Extra");
        assert_eq!(rec.display_name, "TEST OPERATOR");
        assert_eq!(rec.address.street.as_deref(), Some("123 MAIN ST"));
        assert_eq!(rec.address.city.as_deref(), Some("ANYTOWN"));
        assert_eq!(rec.frn.as_deref(), Some("1234567890"));
    }

    #[test]
    fn get_respects_ttl_and_rejects_stale_entry() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        // Backdate cached_at to just beyond the TTL.
        let stale = (now_secs() - TTL_SECS - 1) as i64;
        conn.execute(
            "UPDATE callsigns SET cached_at = ?1 WHERE callsign = 'W1AW'",
            rusqlite::params![stale],
        )
        .unwrap();

        assert!(get(&conn, "W1AW").is_none());
    }

    #[test]
    fn get_accepts_entry_within_ttl() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        // Backdate to just inside the TTL.
        let fresh = (now_secs() - TTL_SECS + 60) as i64;
        conn.execute(
            "UPDATE callsigns SET cached_at = ?1 WHERE callsign = 'W1AW'",
            rusqlite::params![fresh],
        )
        .unwrap();

        assert!(get(&conn, "W1AW").is_some());
    }

    #[test]
    fn get_returns_correct_cached_at_timestamp() {
        let conn = make_conn();
        let before = now_secs();
        store(&conn, &sample_record("W1AW")).unwrap();
        let after = now_secs();

        let (_, cached_at) = get(&conn, "W1AW").unwrap();
        assert!(cached_at >= before && cached_at <= after);
    }

    #[test]
    fn store_upserts_on_conflict() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        let mut updated = sample_record("W1AW");
        updated.display_name = "UPDATED NAME".to_string();
        store(&conn, &updated).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM callsigns WHERE callsign = 'W1AW'",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let (rec, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(rec.display_name, "UPDATED NAME");
    }

    #[test]
    fn store_handles_all_none_optional_fields() {
        let conn = make_conn();
        let minimal = CallsignRecord {
            call_sign: Some("W1AW".to_string()),
            display_name: "MINIMAL".to_string(),
            license_type: None,
            license_status: "A".to_string(),
            license_status_label: "Active".to_string(),
            operator_class_label: None,
            previous_callsign: None,
            trustee_callsign: None,
            trustee_name: None,
            address: AddressInfo::default(),
            frn: None,
            grant_date: None,
            expired_date: None,
            last_action_date: None,
            uls_url: None,
        }; // GMRS / club / pending records may omit most fields
        store(&conn, &minimal).unwrap();

        let (rec, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(rec.display_name, "MINIMAL");
        assert!(rec.license_type.is_none());
        assert!(rec.operator_class_label.is_none());
        assert!(rec.previous_callsign.is_none());
        assert!(rec.trustee_callsign.is_none());
        assert!(rec.address.street.is_none());
    }

    #[test]
    fn previous_and_trustee_round_trip() {
        let conn = make_conn();
        let mut rec = sample_record("W1AW");
        rec.previous_callsign = Some("KA1ABC".to_string());
        rec.trustee_callsign = Some("K1ZZ".to_string());
        rec.trustee_name = Some("SUMNER, DAVID G".to_string());
        store(&conn, &rec).unwrap();

        let (got, _) = get(&conn, "W1AW").unwrap();
        assert_eq!(got.previous_callsign.as_deref(), Some("KA1ABC"));
        assert_eq!(got.trustee_callsign.as_deref(), Some("K1ZZ"));
        assert_eq!(got.trustee_name.as_deref(), Some("SUMNER, DAVID G"));
    }

    #[test]
    fn empty_string_previous_callsign_is_stored_as_none() {
        let conn = make_conn();
        let mut rec = sample_record("W1AW");
        rec.previous_callsign = Some(String::new());
        store(&conn, &rec).unwrap();

        // Empty strings are filtered before storage; previous comes back as None.
        let (got, _) = get(&conn, "W1AW").unwrap();
        assert!(got.previous_callsign.is_none());
    }

    #[test]
    fn record_lookup_appends_history() {
        let conn = make_conn();
        store(&conn, &sample_record("W1AW")).unwrap();

        record_lookup(&conn, "W1AW", "api").unwrap();
        record_lookup(&conn, "W1AW", "cache").unwrap();
        record_lookup(&conn, "W1AW", "api").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lookup_history WHERE callsign = 'W1AW'",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn record_lookup_is_independent_of_callsigns_table() {
        // lookup_history has no FK to callsigns, so history can be written
        // even when there is no cached record (e.g. before the cache is populated).
        let conn = make_conn();
        record_lookup(&conn, "W1AW", "api").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lookup_history WHERE callsign = 'W1AW'",
                rusqlite::params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn lookup_history_records_correct_source() {
        let conn = make_conn();
        record_lookup(&conn, "W1AW", "api").unwrap();
        record_lookup(&conn, "W1AW", "cache").unwrap();

        let sources: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT source FROM lookup_history WHERE callsign = 'W1AW' ORDER BY id")
                .unwrap();
            stmt.query_map(rusqlite::params![], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        assert_eq!(sources, vec!["api", "cache"]);
    }

    #[test]
    fn get_history_returns_empty_when_no_entries() {
        use super::get_history;
        let conn = make_conn();
        assert!(get_history(&conn, "W1AW").is_empty());
    }

    #[test]
    fn get_history_returns_events_newest_first() {
        use super::get_history;
        let conn = make_conn();

        // Insert with explicit timestamps so order is deterministic.
        conn.execute(
            "INSERT INTO lookup_history (callsign, looked_up_at, source) VALUES ('W1AW', 1000, 'api')",
            rusqlite::params![],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lookup_history (callsign, looked_up_at, source) VALUES ('W1AW', 3000, 'cache')",
            rusqlite::params![],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lookup_history (callsign, looked_up_at, source) VALUES ('W1AW', 2000, 'api')",
            rusqlite::params![],
        )
        .unwrap();

        let events = get_history(&conn, "W1AW");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].0, 3000); // newest first
        assert_eq!(events[1].0, 2000);
        assert_eq!(events[2].0, 1000);
    }

    #[test]
    fn get_history_filters_by_callsign() {
        use super::get_history;
        let conn = make_conn();

        record_lookup(&conn, "W1AW", "api").unwrap();
        record_lookup(&conn, "KD9ABC", "api").unwrap();
        record_lookup(&conn, "W1AW", "cache").unwrap();

        let w1aw = get_history(&conn, "W1AW");
        assert_eq!(w1aw.len(), 2);
        assert!(w1aw.iter().all(|(_, src)| src == "api" || src == "cache"));

        let kd9abc = get_history(&conn, "KD9ABC");
        assert_eq!(kd9abc.len(), 1);
    }

    #[test]
    fn get_history_returns_correct_source_values() {
        use super::get_history;
        let conn = make_conn();

        conn.execute(
            "INSERT INTO lookup_history (callsign, looked_up_at, source) VALUES ('W1AW', 1000, 'api')",
            rusqlite::params![],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lookup_history (callsign, looked_up_at, source) VALUES ('W1AW', 2000, 'cache')",
            rusqlite::params![],
        )
        .unwrap();

        let events = get_history(&conn, "W1AW");
        assert_eq!(events[0].1, "cache"); // newest first
        assert_eq!(events[1].1, "api");
    }
}
