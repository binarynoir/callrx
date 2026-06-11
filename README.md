# 📡 callrx

> Beautiful amateur radio callsign lookup for the terminal

Look up any US amateur radio callsign from the [FCC Universal Licensing System](https://wireless2.fcc.gov/UlsApp/UlsSearch/searchLicense.jsp) directly in your terminal — with color output, clickable links, and a clean table layout.

[![CI](https://github.com/your-username/callrx/actions/workflows/ci.yml/badge.svg)](https://github.com/your-username/callrx/actions/workflows/ci.yml)
[![Release](https://github.com/your-username/callrx/actions/workflows/release.yml/badge.svg)](https://github.com/your-username/callrx/releases)

---

## Demo

```txt
  W1AW · ARRL HQ OPERATORS CLUB
  ──────────────────────────────────────────────────
┌─────────────┬──────────────────────────────────────────────────┐
│      Status │ ✓ VALID                                          │
│        Type │ CLUB                                             │
│     Trustee │ K1ZZ — SUMNER, DAVID G                           │
│     Address │ 225 MAIN ST                                      │
│             │ NEWINGTON, CT 06111                              │
│        Grid │ FN31pr (41.714776, -72.726744)                   │
│     Granted │ 12/02/2010                                       │
│     Expires │ 02/26/2031                                       │
│  ULS Record │ https://wireless2.fcc.gov/UlsApp/...             │
│ callook.info│ https://callook.info/W1AW                        │
└─────────────┴──────────────────────────────────────────────────┘
```

Links are **clickable** in [iTerm2](https://iterm2.com), [WezTerm](https://wezfurlong.org/wezterm/),
[Windows Terminal](https://aka.ms/terminal), [Kitty](https://sw.kovidgoyal.net/kitty/), and other
OSC 8-capable terminals.

---

## Installation

### Download a binary (recommended)

Grab the latest binary for your platform from the [Releases page](https://github.com/your-username/callrx/releases):

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
git clone https://github.com/your-username/callrx
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

OPTIONS:
    --json       Output raw JSON from callook.info
    --raw        Plain text output (no color, no formatting)
    --no-links   Disable clickable hyperlinks
    --help       Print help
    --version    Print version
```

**Examples:**

```bash
callrx W1AW                    # Quick lookup
callrx lookup KD9ABC           # Via subcommand
callrx lookup W1AW --json      # Raw JSON (pipe to jq)
callrx lookup W1AW --raw       # Plain text (pipe to grep)
callrx lookup W1AW | grep Grid # Colors stripped when piped
```

---

## Data source

Data comes from [callook.info](https://callook.info), which mirrors the FCC Universal
Licensing System (ULS) and updates weekly. callook.info is not affiliated with the ARRL
or the FCC — it is an independent service maintained by the ham radio community.

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

## License

MIT
