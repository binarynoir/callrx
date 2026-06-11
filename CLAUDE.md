# CLAUDE.md — callrx project context

This file gives Claude full context about the `callrx` project so any AI-assisted
session starts with accurate, complete knowledge of the codebase.

---

## What this project is

`callrx` is a cross-platform CLI tool for looking up amateur radio callsigns from
the FCC Universal Licensing System (ULS) database via the **callook.info** API.

Target users: ham radio operators, contest loggers, DX hunters, club trustees.

---

## Tech stack

| Concern            | Crate                     | Notes                                   |
| ------------------ | ------------------------- | --------------------------------------- |
| CLI parsing        | `clap` v4 (derive API)    | `#[derive(Parser)]`                     |
| HTTP client        | `minreq`                  | sync, https + json-using-serde features |
| JSON serde         | `serde` + `serde_json`    | derive                                  |
| Tables             | hand-rolled in `display`  | OSC8-aware width math (see below)       |
| Colors             | `owo-colors` + `anstream` | anstream strips ANSI when not a TTY     |
| Spinner            | `indicatif`               | spinner during HTTP fetch               |
| Error handling     | `color-eyre`              | installed in main()                     |
| Terminal detection | `std::io::IsTerminal`     | gates colors + OSC 8 links              |
| Hyperlinks         | custom `hyperlink` module | OSC 8, no extra crate                   |

---

## Project structure

```txt
callrx/
├── src/
│   ├── main.rs          — CLI definition (clap), entry point, routing
│   ├── api.rs           — callook.info HTTP client, response types (serde)
│   ├── display.rs       — pretty table output, plain text, error output, spinner
│   └── hyperlink.rs     — OSC 8 terminal hyperlink helper + support detection
├── .vscode/
│   ├── extensions.json  — recommended extensions
│   ├── settings.json    — rust-analyzer, editor, clippy settings
│   ├── launch.json      — debug configs (lldb), includes prompt for callsign
│   └── tasks.json       — build, test, lint, fmt, run shortcuts
├── .github/
│   ├── workflows/ci.yml      — PR/push checks: fmt + clippy + test × 3 OSes
│   └── workflows/release.yml — tag-triggered cross-platform binary release
├── Cargo.toml
├── Cargo.lock           — committed (this is a binary, not a library)
├── rust-toolchain.toml  — pins stable channel + rustfmt + clippy + rust-src
├── rustfmt.toml         — max_width=100, edition=2021
├── .clippy.toml         — pedantic, cognitive complexity threshold
├── .gitignore
├── README.md
└── CLAUDE.md            — this file
```

---

## Key design decisions

### API source: callook.info, not FCC directly

The FCC ULS does not expose a clean JSON REST API — it's a JSP form. callook.info
mirrors the FCC ULS database and provides a simple JSON endpoint:
`https://callook.info/{CALLSIGN}/json`
No auth, no rate limits published. If callook.info is ever unavailable, the
`api.rs` module is the only file that needs changing.

### HTTP: sync minreq instead of async request

The API call is a single GET — there's no benefit to async here. `minreq` has a
much smaller dependency tree (no `tokio`, no `hyper`, no edition2024 transitive deps)
and results in a faster build and smaller binary.

### Hyperlinks: OSC 8 (no extra crate)

Clickable terminal links use the OSC 8 escape sequence:
`ESC ] 8 ;; <url> ESC \ <text> ESC ] 8 ;; ESC \`
Implemented in `hyperlink::link()`. Detection uses `std::io::IsTerminal` plus
known `$TERM` / `$TERM_PROGRAM` / `$WT_SESSION` env vars. Degrades gracefully
to plain text when piped or in an unsupported terminal.

### Table rendering: hand-rolled, not comfy-table

The pretty table is rendered manually in `display.rs`. Table crates measure
cell width by byte/char count, which is wrong for cells containing OSC 8
hyperlinks (they count the URL inside the escape sequence as visible columns and
blow the layout out). `display::visible_width()` skips ANSI SGR and OSC 8
sequences so the box stays content-sized and aligned whether or not links are on.

### License class decoding

callook.info reports operator class as a full word (`EXTRA`, `ADVANCED`,
`GENERAL`, `TECHNICIAN`, `NOVICE`). The single-letter FCC codes (E/A/G/T/N) are
also accepted for robustness. Decoded in
`api.rs::CallsignRecord::license_class_label()`.

### Expiry detection

`api.rs::CallsignRecord::is_expired()` does a rough date comparison without
importing `chrono` — keeps the binary small and avoids a heavy dep for a simple
display hint. Not used for anything safety-critical.

### --raw flag

`--raw` outputs plain text with no ANSI escapes — suitable for piping to grep,
awk, scripts, etc.

### --json flag

`--json` pretty-prints the raw API struct via serde. Useful for debugging or
piping to `jq`.

---

## Data model: callook.info response

```json
{
	"status": "VALID", // "VALID" | "INVALID"
	"type": "CLUB", // "CLUB" | "INDIVIDUAL" | "MILITARY" | etc.
	"current": {
		"callsign": "W1AW",
		"operClass": "EXTRA" // EXTRA | ADVANCED | GENERAL | TECHNICIAN | NOVICE
	},
	"previous": { "callsign": "", "operClass": "" },
	"trustee": { "callsign": "K1ZZ", "name": "SUMNER, DAVID G" },
	"name": "ARRL HQ OPERATORS CLUB",
	"address": {
		"line1": "225 MAIN ST",
		"line2": "NEWINGTON, CT 06111",
		"attn": ""
	},
	"location": {
		"latitude": "41.714776",
		"longitude": "-72.726744",
		"gridsquare": "FN31pr"
	},
	"otherInfo": {
		"grantDate": "12/02/2010",
		"expiryDate": "02/26/2021",
		"lastActionDate": "12/02/2010",
		"frn": "0004511143",
		"ulsUrl": "http://wireless2.fcc.gov/UlsApp/UlsSearch/license.jsp?licKey=780866"
	}
}
```

---

## Build & run

```bash
# Development (the bare-callsign shorthand accepts the same flags as `lookup`)
cargo run -- W1AW
cargo run -- W1AW --json
cargo run -- W1AW --raw
cargo run -- lookup W1AW --json
cargo run -- --help

# Release binary
cargo build --release
./target/release/callrx W1AW

# Run tests
cargo test

# Lint
cargo clippy -- -W clippy::all -W clippy::pedantic

# Format
cargo fmt
```

---

## Release process

1. Bump version in `Cargo.toml`
2. Commit: `git commit -am "chore: bump to v0.2.0"`
3. Tag: `git tag v0.2.0`
4. Push: `git push && git push --tags`
5. GitHub Actions (`release.yml`) builds binaries for all platforms and creates a
   GitHub Release automatically with generated release notes.

**Build targets:**

- `aarch64-apple-darwin` — macOS Apple Silicon
- `x86_64-apple-darwin` — macOS Intel
- `x86_64-unknown-linux-gnu` — Linux x86_64
- `aarch64-unknown-linux-gnu` — Linux ARM64 (cross)
- `armv7-unknown-linux-gnueabihf` — Linux ARMv7 (cross)
- `x86_64-pc-windows-msvc` — Windows x86_64

---

## Future ideas / roadmap

- [ ] `callrx search --name "John Smith"` — search by licensee name
- [ ] `callrx batch <file>` — look up a list of callsigns from a file
- [ ] `callrx history <callsign>` — show previous callsigns
- [ ] JSON output to file: `callrx W1AW --json > w1aw.json`
- [ ] Shell completions: `callrx --completions zsh`
- [ ] Offline mode: cache recent lookups in `~/.cache/callrx/`
- [ ] DXCC country detection from callsign prefix
- [ ] QRZ.com XML API as a fallback source (requires account)

---

## Known limitations

- callook.info only covers **US callsigns** (FCC ULS). It does not cover:
  - Canadian callsigns (Industry Canada)
  - UK callsigns (Ofcom)
  - DX (non-US) callsigns
- callook.info updates weekly from FCC bulk data — not real-time
- The expiry check in `api.rs` is approximate (ignores leap years precisely)
- No caching — every invocation makes an HTTP request

---

## Development environment

- **macOS** (Apple Silicon or Intel) with VS Code
- Rust stable toolchain (pinned via `rust-toolchain.toml`)
- VS Code extensions: rust-analyzer, CodeLLDB, crates, Even Better TOML
- Debugger: LLDB via CodeLLDB extension (launch configs in `.vscode/launch.json`)
