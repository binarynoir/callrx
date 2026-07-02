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
    after_help = "EXAMPLES:\n  callrx W1AW\n  callrx W1AW --json\n  callrx W1AW --weather --neighbors\n  callrx lookup W1AW --raw\n  callrx bandplan\n  callrx bandplan --service G"
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
    /// Show the amateur radio / GMRS band plan reference (47 CFR Part 97 / 95)
    Bandplan {
        /// Restrict to one service: `A` (amateur) or `G` (GMRS). Omit for both.
        #[arg(long, value_parser = ["A", "G"])]
        service: Option<String>,
        /// Output the raw JSON response from the callrx-service API
        #[arg(long)]
        json: bool,
        /// Output plain text without color or formatting
        #[arg(long)]
        raw: bool,
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

    /// Show current weather at the licensee's mailing address. Reuses the same
    /// lookup as local time (always fetched), so this adds no extra API call —
    /// it just displays the weather that comes back with it.
    #[arg(long)]
    weather: bool,

    /// Show other active licensees near the same mailing address (address and
    /// street). One extra API call; never cached.
    #[arg(long)]
    neighbors: bool,
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
        (None, Some(Commands::Bandplan { service, json, raw })) => {
            run_bandplan(service.as_deref(), json, raw)?
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

fn run_bandplan(service: Option<&str>, json: bool, raw: bool) -> Result<()> {
    let spinner = display::make_spinner("Fetching band plan…");
    let result = api::lookup_bandplan(service);
    spinner.finish_and_clear();

    let data = match result {
        Ok(d) => d,
        Err(e) => {
            display::print_error("bandplan", &e.to_string());
            std::process::exit(1);
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&data)?);
    } else if raw {
        display::print_bandplan_plain(&data);
    } else {
        display::print_bandplan(&data);
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

    // Local time can't be derived without geocoding the mailing address, so
    // it's core: fetched on every lookup (best-effort — a failure here
    // shouldn't hide the record that already succeeded). It's never cached,
    // since time changes on every call. Weather comes back in the same
    // response at no extra cost; --weather only controls whether it's shown.
    let location_info = record.call_sign.as_deref().and_then(|cs| {
        let spinner = display::make_spinner("Fetching local time…");
        let result = api::lookup_location_info(cs).ok();
        spinner.finish_and_clear();
        result
    });
    let local_time = location_info.as_ref().and_then(|l| l.time.as_ref());
    // --weather also gates alerts: both come back in the same location-info
    // response as weather, so showing them together adds no extra API cost.
    let weather = opts
        .weather
        .then(|| location_info.as_ref().and_then(|l| l.weather.as_ref()))
        .flatten();
    let alerts = opts
        .weather
        .then(|| location_info.as_ref().and_then(|l| l.alerts.as_deref()))
        .flatten()
        .unwrap_or(&[]);

    // --neighbors is a separate, opt-in API call; also never cached.
    let neighbors = if opts.neighbors {
        record.call_sign.as_deref().and_then(|cs| {
            let spinner = display::make_spinner("Fetching neighbors…");
            let result = api::lookup_neighbors(cs).ok();
            spinner.finish_and_clear();
            result
        })
    } else {
        None
    };

    if opts.json {
        let mut value = serde_json::to_value(&record)?;
        if let Some(map) = value.as_object_mut() {
            if let Some(time) = local_time {
                map.insert("local_time".to_string(), serde_json::to_value(time)?);
            }
            if let Some(w) = weather {
                map.insert("weather".to_string(), serde_json::to_value(w)?);
            }
            if !alerts.is_empty() {
                map.insert("alerts".to_string(), serde_json::to_value(alerts)?);
            }
            if let Some(n) = &neighbors {
                map.insert("neighbors".to_string(), serde_json::to_value(n)?);
            }
        }
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else if opts.raw {
        display::print_plain(&record, cached_at, local_time);
        if let Some(w) = weather {
            display::print_weather_plain(w);
        }
        display::print_alerts_plain(alerts);
        if let Some(n) = &neighbors {
            display::print_neighbors_plain(n);
        }
    } else {
        display::print_pretty(&record, !opts.no_links, cached_at, local_time);
        if let Some(w) = weather {
            display::print_weather(w);
        }
        display::print_alerts(alerts);
        if let Some(n) = &neighbors {
            display::print_neighbors(n);
        }
    }

    Ok(())
}
