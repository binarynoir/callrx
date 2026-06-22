mod api;
mod cache;
mod config;
mod display;
mod hyperlink;

use anstream::{eprintln, println};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use color_eyre::Result;

/// callrx — Amateur radio callsign lookup (FCC ULS via callrx-service)
#[derive(Parser, Debug)]
#[command(
    name = "callrx",
    version,
    about = "Beautiful amateur radio callsign lookup for the terminal",
    long_about = "Look up FCC ULS amateur radio licenses by callsign.\n\nData is served by callrx-service, a REST API over the official FCC\nUniversal Licensing System (ULS) database.\n\nSupports clickable links in iTerm2, WezTerm, Windows Terminal, and other\nOSC 8-capable terminals.",
    after_help = "EXAMPLES:\n  callrx W1AW\n  callrx W1AW --json\n  callrx lookup W1AW --raw"
)]
struct Cli {
    /// Callsign to look up (shorthand — same as `callrx lookup <CALLSIGN>`)
    callsign: Option<String>,

    #[command(flatten)]
    opts: OutputOpts,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Look up an amateur radio callsign
    Lookup {
        /// The callsign to look up (e.g. W1AW, KD9ABC)
        callsign: String,

        #[command(flatten)]
        opts: OutputOpts,
    },
    /// Show lookup history for a callsign
    History {
        /// The callsign to show history for (e.g. W1AW, KD9ABC)
        callsign: String,
        /// Output plain text without color or formatting
        #[arg(long)]
        raw: bool,
    },
    /// Print a shell completion script to stdout
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

/// Output options shared by the bare-callsign shorthand and the `lookup` subcommand.
#[derive(Args, Debug, Default)]
struct OutputOpts {
    /// Output the raw JSON response from the callrx-service API
    #[arg(long)]
    json: bool,

    /// Output plain text without color or formatting
    #[arg(long)]
    raw: bool,

    /// Disable clickable hyperlinks (useful when piping output)
    #[arg(long)]
    no_links: bool,

    /// Bypass the local cache and fetch fresh data from the callrx-service API.
    /// The result is still written to the cache for future lookups.
    #[arg(long)]
    no_cache: bool,
}

fn main() -> Result<()> {
    // Load .env when compiled in debug mode so `cargo run` uses CALLRX_CACHE_DIR
    // from .env instead of the system cache directory. No-op in release builds.
    #[cfg(debug_assertions)]
    let _ = dotenvy::dotenv();

    color_eyre::install()?;

    let cli = Cli::parse();

    // Support `callrx W1AW` shorthand (no subcommand)
    match (cli.callsign, cli.command) {
        (Some(callsign), None) => run_lookup(&callsign, &cli.opts)?,
        (None, Some(Commands::Lookup { callsign, opts })) => run_lookup(&callsign, &opts)?,
        (None, Some(Commands::History { callsign, raw })) => run_history(&callsign, raw)?,
        (None, Some(Commands::Completions { shell })) => {
            clap_complete::generate(shell, &mut Cli::command(), "callrx", &mut std::io::stdout());
        }
        (Some(_), Some(_)) => {
            eprintln!("Error: provide either a callsign or a subcommand, not both.");
            std::process::exit(1);
        }
        (None, None) => {
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}

fn run_history(callsign: &str, raw: bool) -> Result<()> {
    let callsign = callsign.to_uppercase();
    let events = cache::open()
        .ok()
        .map(|conn| cache::get_history(&conn, &callsign))
        .unwrap_or_default();

    if raw {
        display::print_history_plain(&callsign, &events);
    } else {
        display::print_history(&callsign, &events);
    }
    Ok(())
}

fn run_lookup(callsign: &str, opts: &OutputOpts) -> Result<()> {
    let callsign = callsign.to_uppercase();

    // Cache is always opened for writing; --no-cache only bypasses the read.
    let cache_conn = cache::open().ok();

    let cached = if opts.no_cache {
        None
    } else {
        cache_conn
            .as_ref()
            .and_then(|conn| cache::get(conn, &callsign))
    };

    let (record, source, cached_at) = match cached {
        Some((record, cached_at)) => (record, "cache", Some(cached_at)),
        None => {
            let spinner = display::make_spinner(&format!("Looking up {callsign}…"));
            let result = api::lookup_callsign(&callsign);
            spinner.finish_and_clear();

            let record = match result {
                Ok(r) => r,
                Err(e) => {
                    display::print_error(&callsign, &e.to_string());
                    std::process::exit(1);
                }
            };

            if let Some(ref conn) = cache_conn {
                let _ = cache::store(conn, &record);
            }

            (record, "api", None)
        }
    };

    if let Some(ref conn) = cache_conn {
        let _ = cache::record_lookup(conn, &callsign, source);
    }

    if opts.json {
        println!("{}", serde_json::to_string_pretty(&record)?);
    } else if opts.raw {
        display::print_plain(&record, cached_at);
    } else {
        display::print_pretty(&record, !opts.no_links, cached_at);
    }

    Ok(())
}
