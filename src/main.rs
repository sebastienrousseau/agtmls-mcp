// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `agtmls-mcp` — Model Context Protocol server for the `AgtMLS` skill registry.
//!
//! Newline-delimited JSON-RPC 2.0 over stdio. All dispatch lives in the
//! `agtmls_mcp` library crate so `cargo test` can exercise it directly; this
//! binary is the transport shim — read a line, hand it to
//! [`agtmls_mcp::handle_message`], write the reply if there is one.
//!
//! ```text
//! cargo install agtmls-mcp
//! claude mcp add agtmls -- agtmls-mcp --registry ~/dev/agtmls
//! ```

#![forbid(unsafe_code)]

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use agtmls_mcp::{HandleOutcome, handle_message, registry::Registry};

const HELP: &str = "\
agtmls-mcp — Model Context Protocol server for the AgtMLS skill registry.

USAGE:
  agtmls-mcp [--registry <path>]   Start the JSON-RPC stdio loop.
  agtmls-mcp --version | -V        Print version and exit.
  agtmls-mcp --help | -h           Print this help and exit.

ENVIRONMENT:
  AGTMLS_HOME   Registry checkout, if --registry is not given.
  AGTMLS_SPEC   agtmls-spec checkout. Required by agtmls_audit: without a
                rule set an audit would report clean because it has no rules.

The registry is only ever read. This server makes no network calls and
collects nothing.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("agtmls-mcp {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let root = args
        .iter()
        .position(|a| a == "--registry")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| std::env::var("AGTMLS_HOME").ok().map(PathBuf::from));

    // A missing registry is reported per-request rather than fatally: an MCP
    // client starts the server before it has anything to ask, and exiting here
    // would surface as "the server crashed" rather than "point me at a
    // registry".
    let registry = if let Some(path) = root {
        match Registry::open(&path) {
            Ok(registry) => {
                eprintln!(
                    "agtmls-mcp: {} skill(s), registry {}",
                    registry.skills.len(),
                    registry.version
                );
                Some(registry)
            }
            Err(message) => {
                eprintln!("agtmls-mcp: {message}");
                None
            }
        }
    } else {
        eprintln!("agtmls-mcp: no registry; set AGTMLS_HOME or pass --registry");
        None
    };

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        match handle_message(&line, registry.as_ref()) {
            HandleOutcome::Reply(reply) => {
                if writeln!(stdout, "{reply}").is_err() || stdout.flush().is_err() {
                    break; // the client went away
                }
            }
            HandleOutcome::Silent => {}
            HandleOutcome::Shutdown => break,
        }
    }
    ExitCode::SUCCESS
}
