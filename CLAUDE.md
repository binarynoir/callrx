# CLAUDE.md — callrx project context

This file gives Claude full context about the `callrx` project so any AI-assisted
session starts with accurate, complete knowledge of the codebase.

---

## Working conventions (read first)

- **Keep `README.md` up to date.** When a change affects anything user-facing —
  CLI flags, commands, usage examples, installation, build/release steps,
  supported platforms, badges, or the demo output — update `README.md` in the
  same change. Treat the README as part of the definition of "done"; never let it
  drift from the actual behaviour of the code or workflows.
- **Keep `man/callrx.1` up to date.** When a change affects CLI flags, subcommands,
  output formats, exit codes, or any documented behaviour, update the man page in
  the same change. The `.TH` header date line should reflect the month and year of
  the change (e.g. `"June 2026"`). The version placeholder `@@VERSION@@` in the
  source is substituted automatically by the release workflow — do not replace it
  by hand.
- **Never use emojis.** Do not add emojis to code, comments, doc strings, CLI
  output, commit messages, `README.md`, `CLAUDE.md`, or any other file. Plain
  text only. (Functional Unicode glyphs already used in terminal output, such as
  the `✓` / `✗` / `⚠` status markers, are not decorative emoji and may stay.)
- **Commit as the repository owner, never as Claude.** Author and commit every
  change as the user's own git identity. Do not add Claude or Anthropic as an
  author, committer, or `Co-Authored-By` trailer, and do not mention Claude or
  any AI assistant in commit messages or PR descriptions.

---

## What this project is

`callrx` is a cross-platform CLI tool for looking up amateur radio callsigns from
the FCC Universal Licensing System (ULS) database via the **callrx-service** REST
API (the backend counterpart in `binarynoir/callrx-service`).

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
│   ├── api.rs           — callrx-service HTTP client, response types (serde)
│   ├── cache.rs         — local SQLite cache + lookup history (rusqlite)
│   ├── config.rs        — resolves the callrx-service base URL (CALLRX_API_URL)
│   ├── display.rs       — pretty table output, plain text, error output, spinner
│   └── hyperlink.rs     — OSC 8 terminal hyperlink helper + support detection
├── man/
│   └── callrx.1         — man page (troff); @@VERSION@@ substituted by release.yml
├── .vscode/
│   ├── extensions.json  — recommended extensions
│   ├── settings.json    — rust-analyzer, editor, clippy settings
│   ├── launch.json      — debug configs (lldb), includes prompt for callsign
│   └── tasks.json       — build, test, lint, fmt, run shortcuts
├── .github/
│   ├── workflows/ci.yml              — PR/push checks: fmt + clippy + test × 3 OSes
│   ├── workflows/release.yml         — manual version bump + tag + release + binary build/attach
│   └── workflows/update-homebrew.yml — regenerates the Homebrew tap formula on release
├── Cargo.toml
├── Cargo.lock           — committed (this is a binary, not a library)
├── Cross.toml           — passes CALLRX_API_URL into the ARM cross build container
├── env-sample           — template for .env (CALLRX_API_URL, CALLRX_CACHE_DIR)
├── rust-toolchain.toml  — pins stable channel + rustfmt + clippy + rust-src
├── rustfmt.toml         — max_width=100, edition=2021
├── .clippy.toml         — pedantic, cognitive complexity threshold
├── .gitignore
├── README.md
└── CLAUDE.md            — this file
```

---

## Key design decisions

### API source: callrx-service

The CLI fetches license data from **callrx-service**, our own FastAPI REST API
over the FCC ULS bulk database. The only endpoint the CLI consumes is
`GET {base}/callsign/{CALLSIGN}`, which returns a JSON `CallsignResponse` (404 if
not found, 429 if rate limited). No auth is required. `api.rs` owns the request
and the response types; it is the only file that needs changing if the backend
contract changes.

### Endpoint configuration (CALLRX_API_URL)

`config.rs::api_base_url()` resolves the backend base URL in this order:

1. `CALLRX_API_URL` env var at runtime (loaded from `.env` in debug builds by
   `dotenvy`, so `cargo run` targets the dev service);
2. `CALLRX_API_URL` baked in at compile time via `option_env!` — the release
   workflow supplies this from a GitHub secret so the production URL is not
   committed to the public source tree (and reaches the ARM `cross` containers
   via `Cross.toml` passthrough);
3. `http://localhost:8073` fallback (the service's documented dev port).

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

callrx-service returns the operator class as a ready-made full label
(`Amateur Extra`, `Advanced`, `General`, `Technician`, `Novice`), so
`api.rs::CallsignRecord::license_class_label()` returns it verbatim (or `—` for
club licenses, which carry a trustee instead of an operator class). `display.rs`
color-matches on those exact strings.

### Expiry detection

`api.rs::CallsignRecord::is_expired()` trusts the FCC license status code
(`license_status == "E"`) rather than parsing dates — the service already
exposes the authoritative status. Used only as a display hint (red expiry date).

### --raw flag

`--raw` outputs plain text with no ANSI escapes — suitable for piping to grep,
awk, scripts, etc.

### --json flag

`--json` pretty-prints the raw API struct via serde. Useful for debugging or
piping to `jq`.

---

## Data model: callrx-service response

`api.rs::CallsignRecord` deserializes the **subset** of the service's
`CallsignResponse` that the CLI displays and caches. The service returns more
fields (email, phone, GMRS `service`, `frn_licenses` siblings, …); `serde`
ignores anything not declared in the struct. Dates are ISO 8601 (`YYYY-MM-DD`).
The service does **not** provide grid square or lat/lon — that data is absent
from the FCC bulk source, so the CLI no longer shows a Grid row.

```json
{
    "call_sign": "W1AW",
    "display_name": "ARRL HQ OPERATORS CLUB",
    "license_type": "Club", // "Individual" | "Club"
    "license_status": "A", // A | C | E | T | L
    "license_status_label": "Active",
    "operator_class_label": null, // e.g. "Amateur Extra" for individuals
    "previous_callsign": null,
    "trustee_callsign": "K1ZZ",
    "trustee_name": "SUMNER, DAVID G",
    "address": {
        "street": "225 MAIN ST",
        "city": "NEWINGTON",
        "state": "CT",
        "zip_code": "06111",
        "po_box": null
    },
    "frn": "0004511143",
    "grant_date": "2010-12-02",
    "expired_date": "2031-02-26",
    "last_action_date": "2010-12-02",
    "uls_url": "https://wireless2.fcc.gov/UlsApp/UlsSearch/license.jsp?licKey=1000001"
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

Versioning is manual and driven entirely by `release.yml` — the same
workflow_dispatch-with-a-bump-choice pattern as `callrx-service`,
`callrx-frontend`, and `callrx-cli-admin`'s own release workflows. You do
**not** edit the version in `Cargo.toml` by hand.

1. Merge all changes to `main` and confirm CI passes.
2. Go to **Actions → Release → Run workflow** (`main` branch only).
3. Choose a bump type (`patch` / `minor` / `major`) and, optionally, release
   notes — leave notes blank to auto-generate them from commit messages.
4. `release.yml`'s `version` job bumps `Cargo.toml`/`Cargo.lock`, commits the
   bump to `main` with `[skip ci]`, creates and pushes the `vX.Y.Z` tag, and
   publishes the GitHub release via `gh release create` (using the default
   `GITHUB_TOKEN` — nothing else in this repo listens for the `release`
   event, so no PAT is needed to fan out to another workflow).
5. The `build` job (needs `version`) then builds binaries for all platforms
   at that tag; `attach` (needs `build`) uploads them to the release via
   `gh release upload --clobber`; `update-homebrew` (needs `attach`) calls
   `update-homebrew.yml` to regenerate the formula in the Homebrew tap so
   `brew upgrade callrx` picks up the new version.

There is no `CHANGELOG.md` — release notes live only on the GitHub Release
itself (auto-generated from commits, or the notes typed into the dispatch
form).

**Build targets:**

- `aarch64-apple-darwin` — macOS Apple Silicon
- `x86_64-apple-darwin` — macOS Intel
- `x86_64-unknown-linux-gnu` — Linux x86_64
- `aarch64-unknown-linux-gnu` — Linux ARM64 (cross)
- `armv7-unknown-linux-gnueabihf` — Linux ARMv7 (cross)
- `x86_64-pc-windows-msvc` — Windows x86_64

---

## Homebrew distribution

`callrx` is installable via Homebrew from the
[`binarynoir/homebrew-callrx`](https://github.com/binarynoir/homebrew-callrx)
tap: `brew install binarynoir/callrx/callrx`. The full maintainer guide (setup,
automation, homebrew/core, troubleshooting) lives in the **private**
`binarynoir/programming-cookbook` repo at `homebrew/callrx/HOMEBREW.md` — it is
kept out of this public repo on purpose.

- **Formula type:** the formula installs the **prebuilt release binary** for the
  user's platform (no Rust toolchain needed). It uses `on_macos` / `on_linux`
  with nested `on_arm` / `on_intel` blocks, one `url` + `sha256` per arch,
  pointing at the GitHub release `.tar.gz` assets. Supported arches: macOS
  (Apple Silicon, Intel) and Linux (x86_64, ARM64). Windows is not distributed
  via Homebrew.
- **Automation:** `update-homebrew.yml` regenerates `Formula/callrx.rb` after
  each release. It reads the version from the tag, pulls each platform's SHA256
  from the published `.tar.gz.sha256` sidecar assets, writes the formula, and
  commits it to the tap. `release.yml`'s `update-homebrew` job calls it once
  binaries are attached; it can also be run manually from the Actions tab for
  an existing tag.
- **Required secret:** `HOMEBREW_TAP_TOKEN` — a fine-grained PAT (or classic
  token with `repo` scope) that can push to `binarynoir/homebrew-callrx`. The
  default `GITHUB_TOKEN` cannot write to another repo, so this is mandatory for
  the automation to push the updated formula.
- **Do not hand-edit** `Formula/callrx.rb`; it is overwritten on every release.
  If the release asset naming or target list changes, update the heredoc
  template inside `update-homebrew.yml` to match.

### homebrew/core (the bare `brew install callrx`) — separate path

The tap above is **not** homebrew/core. homebrew/core is **source-only** (it
rejects prebuilt-binary URLs) and gated on Homebrew's notability requirements, so
its formula must be a different, source-build recipe — kept as a versioned draft
in the private `binarynoir/programming-cookbook` repo at `homebrew/callrx/callrx.rb`
(`depends_on "rust" => :build`, `cargo install` via `std_cargo_args`,
source-tarball `url` + `sha256`). Do not try to put the prebuilt formula in
homebrew/core, and do not merge the two formulae — they have the same class name
and would collide. See `homebrew/callrx/HOMEBREW.md` (Part B) in that private repo
for the submission process.

---

## Future ideas / roadmap

- [ ] `callrx batch <file>` — look up a list of callsigns from a file
- [ ] DXCC country detection from callsign prefix
- [ ] QRZ.com XML API as a fallback source (requires account)

---

## Known limitations

- The FCC ULS only covers **US callsigns**. It does not cover:
  - Canadian callsigns (Industry Canada)
  - UK callsigns (Ofcom)
  - DX (non-US) callsigns
- callrx-service updates weekly from FCC bulk data — not real-time
- Grid square / lat-lon are not available (absent from the FCC bulk source)
- Successful lookups are cached locally for 7 days (`cache.rs`); use `--no-cache`
  to bypass the read

---

## Development environment

- **macOS** (Apple Silicon or Intel) with VS Code
- Rust stable toolchain (pinned via `rust-toolchain.toml`)
- VS Code extensions: rust-analyzer, CodeLLDB, crates, Even Better TOML
- Debugger: LLDB via CodeLLDB extension (launch configs in `.vscode/launch.json`)
