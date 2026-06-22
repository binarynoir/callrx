# callrx

> Beautiful amateur radio callsign lookup for the terminal

Look up any US amateur radio callsign from the [FCC Universal Licensing System](https://wireless2.fcc.gov/UlsApp/UlsSearch/searchLicense.jsp) directly in your terminal — with color output, clickable links, and a clean table layout.

[![CI](https://github.com/binarynoir/callrx/actions/workflows/ci.yml/badge.svg)](https://github.com/binarynoir/callrx/actions/workflows/ci.yml)
[![Release Please](https://github.com/binarynoir/callrx/actions/workflows/release-please.yml/badge.svg)](https://github.com/binarynoir/callrx/actions/workflows/release-please.yml)
[![Latest release](https://img.shields.io/github/v/release/binarynoir/callrx)](https://github.com/binarynoir/callrx/releases/latest)

[![Support me on Buy Me a Coffee](https://img.shields.io/badge/Support%20me-Buy%20Me%20a%20Coffee-orange?style=for-the-badge&logo=buy-me-a-coffee)](https://buymeacoffee.com/binarynoir)
[![Support me on Ko-fi](https://img.shields.io/badge/Support%20me-Ko--fi-blue?style=for-the-badge&logo=ko-fi)](https://ko-fi.com/binarynoir)

---

## Demo

![callrx demo](callrx-demo.gif)

Links are **clickable** in [iTerm2](https://iterm2.com), [WezTerm](https://wezfurlong.org/wezterm/),
[Windows Terminal](https://aka.ms/terminal), [Kitty](https://sw.kovidgoyal.net/kitty/), and other
OSC 8-capable terminals.

---

## Installation

### Homebrew (macOS / Linux)

```bash
brew install binarynoir/callrx/callrx
```

Or tap first, then install:

```bash
brew tap binarynoir/callrx
brew install callrx
```

Upgrade later with `brew upgrade callrx`. The formula installs the prebuilt
release binary for your platform — no Rust toolchain required.

### Download a binary

Grab the latest binary for your platform from the [Releases page](https://github.com/binarynoir/callrx/releases):

| Platform              | File                                             |
| --------------------- | ------------------------------------------------ |
| macOS (Apple Silicon) | `callrx-vX.Y.Z-aarch64-apple-darwin.tar.gz`      |
| macOS (Intel)         | `callrx-vX.Y.Z-x86_64-apple-darwin.tar.gz`       |
| Linux x86_64          | `callrx-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux ARM64           | `callrx-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` |
| Windows               | `callrx-vX.Y.Z-x86_64-pc-windows-msvc.zip`       |

**macOS / Linux:**

```bash
tar -xzf callrx-*.tar.gz
sudo mv callrx /usr/local/bin/
callrx --version
```

**Windows:** extract the `.zip` and add the folder to your `PATH`.

### Build from source

```bash
git clone https://github.com/binarynoir/callrx
cd callrx
cargo build --release
./target/release/callrx --version
```

Requires [Rust](https://rustup.rs) (stable toolchain, 1.75+).

---

## Usage

```txt
callrx [CALLSIGN]
callrx lookup <CALLSIGN> [OPTIONS]
callrx history <CALLSIGN> [--raw]
callrx completions <SHELL>

OPTIONS:
    --json       Output the raw JSON response from the callrx-service API
    --raw        Plain text output (no color, no formatting)
    --no-links   Disable clickable hyperlinks
    --no-cache   Bypass the local cache; always fetch fresh data
    --help       Print help
    --version    Print version

SHELLS: bash, zsh, fish, elvish, powershell
```

**Examples:**

```bash
callrx W1AW                    # Quick lookup (served from cache if fresh)
callrx lookup KD9ABC           # Via subcommand
callrx lookup W1AW --json      # Raw JSON (pipe to jq)
callrx lookup W1AW --raw       # Plain text (pipe to grep)
callrx lookup W1AW --no-cache  # Force a fresh API fetch
callrx lookup W1AW | grep Expires # Colors stripped when piped
callrx history W1AW            # Show all past lookups of W1AW
callrx history W1AW --raw      # History as plain text (for scripts)
```

### Local cache

`callrx` caches every successful lookup in a local SQLite database for 7 days
(matching the FCC's weekly ULS publication schedule). Subsequent lookups for the
same callsign are served instantly from the cache and show a "Cached X ago"
note in the output.

The cache database lives at:

| Platform | Path                                |
| -------- | ----------------------------------- |
| macOS    | `~/Library/Caches/callrx/callrx.db` |
| Linux    | `~/.cache/callrx/callrx.db`         |
| Windows  | `%LOCALAPPDATA%\callrx\callrx.db`   |

Use `--no-cache` to force a fresh fetch. The fresh result is still written
back to the cache so the next lookup benefits from it.

### Lookup history

Every successful lookup — whether served from the cache or fetched live — is
recorded in the local SQLite database. Use `callrx history <CALLSIGN>` to see
when a callsign was last looked up:

```bash
callrx history W1AW
```

```txt
W1AW · 3 lookups

  2026-06-11 10:30   live     just now
  2026-06-10 09:15   cached   1 day ago
  2026-06-09 14:22   live     2 days ago
```

`live` means the data was fetched fresh from the callrx-service API; `cached`
means it was served from the local cache. Use `--raw` for plain text output
suitable for scripting.

### Shell completions

Generate and install a completion script for your shell:

**zsh:**

```zsh
callrx completions zsh > ~/.zsh/completions/_callrx
# Ensure ~/.zsh/completions is in your $fpath (add to ~/.zshrc if needed):
# fpath=(~/.zsh/completions $fpath)
# autoload -Uz compinit && compinit
```

**bash:**

```bash
# Add to ~/.bashrc to load on every session:
eval "$(callrx completions bash)"
```

**fish:**

```fish
callrx completions fish > ~/.config/fish/completions/callrx.fish
```

**PowerShell:**

```powershell
callrx completions powershell >> $PROFILE
```

---

## Data source

Data is served by **callrx-service**, a REST API over the official [FCC Universal
Licensing System (ULS)](https://wireless2.fcc.gov/UlsApp/UlsSearch/searchLicense.jsp)
amateur radio database. The service is refreshed weekly from the FCC bulk download.

For the authoritative FCC record, click the **ULS Record** link in the output.

---

## Supported terminals (clickable links)

OSC 8 hyperlinks work in:

- [iTerm2](https://iterm2.com) (macOS)
- [WezTerm](https://wezfurlong.org/wezterm/) (macOS, Windows, Linux)
- [Windows Terminal](https://aka.ms/terminal) v1.4+
- [Kitty](https://sw.kovidgoyal.net/kitty/)
- [GNOME Terminal](https://help.gnome.org/users/gnome-terminal/) 3.26+
- Most VTE-based terminals

Links degrade gracefully to plain text in unsupported terminals or when output is piped.

---

## Development

Requires the stable [Rust](https://rustup.rs) toolchain (pinned in `rust-toolchain.toml`).

### Backend configuration

`callrx` talks to a [callrx-service](https://github.com/binarynoir/callrx-service)
backend. The endpoint is resolved at runtime from the `CALLRX_API_URL` environment
variable, falling back to the URL baked in at build time, and finally to
`http://localhost:8073`.

For local development, copy `env-sample` to `.env` and point `CALLRX_API_URL` at
your running service — `.env` is loaded automatically in debug builds (`cargo run`).
Release binaries get their default endpoint from the `CALLRX_API_URL` GitHub Actions
secret, baked in at compile time, so installed binaries work without any setup.

```bash
# Build
cargo build                       # debug build
cargo build --release             # optimized binary at target/release/callrx

# Run
cargo run -- W1AW                 # run against a callsign
cargo run -- W1AW --json          # the shorthand accepts the same flags as `lookup`

# Test
cargo test

# Lint & format (matches CI)
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

CI (`.github/workflows/ci.yml`) runs `fmt`, `clippy -D warnings`, `build`, and
`test` on Linux, macOS, and Windows for every push and pull request. It can also
be run on demand from the **Actions** tab.

---

## Releasing

Versioning is automated with [release-please](https://github.com/googleapis/release-please)
and driven by [Conventional Commits](https://www.conventionalcommits.org/) — you
**don't** edit the version in `Cargo.toml` by hand.

1. Merge work into `main` using conventional commit messages:

   | Commit prefix                      | Effect                     |
   | ---------------------------------- | -------------------------- |
   | `fix: …`                           | patch bump (0.1.0 → 0.1.1) |
   | `feat: …`                          | minor bump (0.1.0 → 0.2.0) |
   | `feat!: …` / `BREAKING CHANGE:`    | major bump (0.1.0 → 1.0.0) |
   | `chore:` `docs:` `refactor:` `ci:` | no release on their own    |

2. release-please opens and maintains a **Release PR** that bumps `Cargo.toml` +
   `Cargo.lock` and updates `CHANGELOG.md`. Review and merge it when you're ready
   to ship.
3. On merge it creates the `vX.Y.Z` tag and a GitHub Release (notes from the
   changelog), builds binaries for all platforms and attaches them, then
   regenerates the [Homebrew tap](https://github.com/binarynoir/homebrew-callrx)
   formula so `brew upgrade callrx` picks up the new version.

**Build targets:** macOS (Apple Silicon + Intel), Linux (x86_64, ARM64, ARMv7),
and Windows x86_64.

**Manual / re-release:** the **Release** workflow can also be run from the
**Actions** tab ("Run workflow" → enter an existing tag), or triggered by pushing
a `v*` tag directly.

> **One-time setup:**
>
> - In **Settings → Actions → General**, enable _"Allow GitHub Actions to create
>   and approve pull requests"_ so release-please can open the Release PR.
> - Add a `HOMEBREW_TAP_TOKEN` repo secret: a fine-grained PAT (or classic token
>   with `repo` scope) that can push to `binarynoir/homebrew-callrx`. The
>   `update-homebrew.yml` workflow uses it to commit the regenerated formula to
>   the tap. The default `GITHUB_TOKEN` cannot write to another repository.
> - Add a `CALLRX_API_URL` repo secret: the production callrx-service endpoint
>   (e.g. `https://api.example.com`). The **Release** workflow bakes it into the
>   binaries at compile time so installed copies point at the live service.

---

## License

MIT
