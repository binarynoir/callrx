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
├── man/
│   └── callrx.1         — man page (troff); @@VERSION@@ substituted by release.yml
├── .vscode/
│   ├── extensions.json  — recommended extensions
│   ├── settings.json    — rust-analyzer, editor, clippy settings
│   ├── launch.json      — debug configs (lldb), includes prompt for callsign
│   └── tasks.json       — build, test, lint, fmt, run shortcuts
├── .github/
│   ├── workflows/ci.yml              — PR/push checks: fmt + clippy + test × 3 OSes
│   ├── workflows/release.yml         — tag-triggered cross-platform binary release
│   ├── workflows/release-please.yml  — release PR + tag; invokes release & tap update
│   └── workflows/update-homebrew.yml — regenerates the Homebrew tap formula on release
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

Versioning is automated with **release-please** (`release-please.yml`) driven by
[Conventional Commits](https://www.conventionalcommits.org/). You do **not** edit
the version in `Cargo.toml` by hand.

1. Land work on `main` using conventional commit messages:
   - `fix: …` → patch bump (0.1.0 → 0.1.1)
   - `feat: …` → minor bump (0.1.0 → 0.2.0)
   - `feat!: …` / `fix!: …` / a `BREAKING CHANGE:` footer → major bump
   - `chore:`, `docs:`, `refactor:`, `test:`, `ci:` → no release on their own
2. release-please opens/maintains a **Release PR** that bumps `Cargo.toml` +
   `Cargo.lock` and updates `CHANGELOG.md`. Review and merge it when ready.
3. On merge, release-please creates the `vX.Y.Z` tag and a GitHub Release with
   notes from the changelog, then calls `release.yml` (via `workflow_call`) to
   build binaries for all platforms and attach them to that release.
4. After the binaries are attached, `update-homebrew.yml` regenerates the
   formula in the Homebrew tap (see below) so `brew upgrade callrx` works.

State lives in `release-please-config.json` and `.release-please-manifest.json`.

**Manual / re-release:** `release.yml` can also be run from the Actions tab
("Run workflow" → enter an existing tag), or triggered by pushing a `v*` tag
directly. The `workflow_call` path skips note generation so it never clobbers
release-please's notes; the manual paths generate notes as before.

**Why workflow_call instead of a tag trigger:** a tag pushed by release-please's
`GITHUB_TOKEN` will not trigger the `push: tags` workflow (GitHub's recursion
guard). Invoking `release.yml` directly via `workflow_call` avoids needing a PAT.

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
  commits it to the tap. `release-please.yml` calls it once binaries are
  attached; it can also be run manually from the Actions tab for an existing tag.
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

- [ ] `callrx search --name "John Smith"` — search by licensee name
- [ ] `callrx batch <file>` — look up a list of callsigns from a file
- [ ] `callrx history <callsign>` — show previous callsigns
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
