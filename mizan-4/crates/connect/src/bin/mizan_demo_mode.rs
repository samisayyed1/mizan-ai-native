//! `mizan-demo-mode` — investor-pitch entitlement override printer.
//!
//! The spec (`Mizan_Demo_Uncle_Feroz.md` §PR-DEMO-TIER) calls for a
//! one-shot CLI that flips the local install into Gold for the
//! Uncle Feroz walkthrough. The actual override lives in
//! `connect::entitlements::demo_mode_active` — it reads
//! `MIZAN_DEMO_MODE=1` + `MIZAN_ALLOW_PRODUCTION=1` per call and
//! short-circuits to `demo_mode_gold()`.
//!
//! This binary doesn't mutate any persistent state. It prints the
//! shell-export line the user runs in the terminal that launches the
//! Tauri dev server, so the override is bound to ONE session and
//! never lingers in a config file or DB row that could leak into a
//! shipped build. Idempotent. Auditable. Greppable in shell history.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let tier = parse_tier(&args);

    match tier.as_deref() {
        Some("gold") | None => {
            print_enable();
            ExitCode::SUCCESS
        }
        Some("off") | Some("free") => {
            print_disable();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "mizan-demo-mode: unsupported tier '{other}'. Use --tier=gold or --tier=off."
            );
            ExitCode::from(2)
        }
    }
}

fn parse_tier(args: &[String]) -> Option<String> {
    for a in args {
        if let Some(v) = a.strip_prefix("--tier=") {
            return Some(v.to_string());
        }
    }
    // Bare `mizan-demo-mode` (no flags) defaults to enable-gold per
    // the spec's "cargo run --bin mizan-demo-mode -- --tier=gold"
    // canonical invocation; we tolerate the omission.
    None
}

fn print_enable() {
    println!("# Mizan demo mode — Gold tier unlocked for this shell.");
    println!("# Re-run with --tier=off to revert. The override lasts");
    println!("# only for processes started from this shell.");
    println!();
    println!("export MIZAN_DEMO_MODE=1");
    println!("export MIZAN_ALLOW_PRODUCTION=1");
    println!();
    println!("# Now start the Tauri dev server from this same shell:");
    println!("#   pnpm tauri dev");
    println!();
    println!("# To verify in-process: any handler that resolves entitlements");
    println!("# will return plan=\"gold-demo\" until both env vars are unset.");
    eprintln!("Demo mode enabled — all Pro/Gold features unlocked.");
}

fn print_disable() {
    println!("# Mizan demo mode — disabled.");
    println!();
    println!("unset MIZAN_DEMO_MODE");
    println!("unset MIZAN_ALLOW_PRODUCTION");
    eprintln!("Demo mode disabled.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_tier_gold() {
        let args = vec!["--tier=gold".to_string()];
        assert_eq!(parse_tier(&args), Some("gold".to_string()));
    }

    #[test]
    fn parses_explicit_tier_off() {
        let args = vec!["--tier=off".to_string()];
        assert_eq!(parse_tier(&args), Some("off".to_string()));
    }

    #[test]
    fn bare_invocation_returns_none_and_defaults_to_enable() {
        let args: Vec<String> = vec![];
        assert_eq!(parse_tier(&args), None);
    }

    #[test]
    fn ignores_unrelated_args() {
        let args = vec!["--verbose".to_string(), "--tier=gold".to_string()];
        assert_eq!(parse_tier(&args), Some("gold".to_string()));
    }
}
