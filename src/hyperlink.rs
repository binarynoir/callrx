/// Wraps `text` in an OSC 8 terminal hyperlink pointing to `url`.
///
/// Works in iTerm2, `WezTerm`, Windows Terminal (1.4+), Kitty, GNOME Terminal
/// (3.26+), and any other VTE-based terminal.
///
/// Falls back to plain `text` when `enabled` is false (e.g. piped output).
///
/// Format: ESC ] 8 ; ; <url> ESC \ <text> ESC ] 8 ; ; ESC \
pub fn link(url: &str, text: &str, enabled: bool) -> String {
    if !enabled || url.is_empty() {
        return text.to_string();
    }
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

/// Returns true when stdout is likely a terminal that supports OSC 8.
///
/// We check whether stdout is a TTY and look for known `$TERM` /
/// `$TERM_PROGRAM` / `$WT_SESSION` values.
pub fn osc8_supported() -> bool {
    use std::io::IsTerminal;

    if !std::io::stdout().is_terminal() {
        return false;
    }

    // Always assume support unless we know it's a terminal that can't handle it
    let term = std::env::var("TERM").unwrap_or_default();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    // Known non-supporting terminals
    let unsupported = ["dumb", "xterm-mono"];
    if unsupported.contains(&term.as_str()) {
        return false;
    }

    // Known supporting terminals (be permissive — most modern terminals work)
    let supported_terms = [
        "xterm-256color",
        "xterm-kitty",
        "tmux-256color",
        "screen-256color",
    ];
    // Includes the VS Code integrated terminal ("vscode")
    let supported_programs = ["iTerm.app", "WezTerm", "Hyper", "vscode"];

    std::env::var_os("WT_SESSION").is_some() // Windows Terminal
        || supported_terms.contains(&term.as_str())
        || supported_programs.iter().any(|p| term_program.contains(p))
}
