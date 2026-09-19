// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Library surface for `agtmls-mcp`.
//!
//! Hosts the JSON-RPC 2.0 dispatch and the tool implementations. The binary in
//! `main.rs` is a thin stdio loop that drives [`handle_message`]; tests reach
//! the same handlers directly, so coverage does not depend on standing up a
//! process.
//!
//! # What this server is for
//!
//! By 2027, "an agent discovers a capability" will mean "an agent queried an
//! MCP server", not "a shell script made some symlinks". A registry that can
//! only copy files into a directory is cornered: it cannot answer *which*
//! skill, it cannot say whether the copy is still what was published, and it
//! cannot be asked anything.
//!
//! # What it deliberately does not do
//!
//! It never writes to the registry, and it makes no network calls. A server
//! that can modify the registry it vouches for is not a useful thing to trust,
//! and a local-first tool that phones home has traded its only durable asset.

#![forbid(unsafe_code)]

pub mod registry;
pub mod tools;

use serde_json::{Value, json};

use registry::Registry;

/// The MCP protocol revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// What the stdio loop should do with a handled message.
#[derive(Debug, PartialEq, Eq)]
pub enum HandleOutcome {
    /// Write this JSON-RPC reply.
    Reply(String),
    /// A notification: acknowledge nothing, keep reading.
    Silent,
    /// The client asked the server to stop.
    Shutdown,
}

fn error(id: Option<&Value>, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id.cloned().unwrap_or(Value::Null),
        "error": { "code": code, "message": message },
    })
    .to_string()
}

fn reply(id: Option<&Value>, result: Value) -> String {
    // Built by hand rather than with json!, which would borrow `result` and
    // leave it unconsumed. Moving it in is both cheaper and what
    // clippy::needless_pass_by_value is asking for.
    let mut envelope = serde_json::Map::with_capacity(3);
    envelope.insert("jsonrpc".to_owned(), Value::from("2.0"));
    envelope.insert("id".to_owned(), id.cloned().unwrap_or(Value::Null));
    envelope.insert("result".to_owned(), result);
    Value::Object(envelope).to_string()
}

/// Tool call results are reported inside a successful envelope with
/// `isError`, not as a JSON-RPC error: the model is meant to read the failure
/// and decide what to do, and a protocol-level error takes that choice away.
fn tool_result(id: Option<&Value>, text: &str, is_error: bool) -> String {
    reply(
        id,
        json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
    )
}

/// Handle one JSON-RPC message.
///
/// Every failure path returns a well-formed JSON-RPC envelope. A server that
/// closes the connection on bad input teaches the client nothing about what
/// was wrong with it.
#[must_use]
pub fn handle_message(line: &str, registry: Option<&Registry>) -> HandleOutcome {
    let Ok(message): Result<Value, _> = serde_json::from_str(line) else {
        return HandleOutcome::Reply(error(None, -32700, "parse error: not valid JSON"));
    };
    let id = message.get("id");
    let Some(method) = message["method"].as_str() else {
        return HandleOutcome::Reply(error(id, -32600, "invalid request: no method"));
    };
    let params = &message["params"];

    match method {
        "initialize" => HandleOutcome::Reply(reply(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
                "serverInfo": { "name": "agtmls-mcp", "version": env!("CARGO_PKG_VERSION") },
            }),
        )),
        "notifications/initialized" | "notifications/cancelled" => HandleOutcome::Silent,
        "ping" => HandleOutcome::Reply(reply(id, json!({}))),
        "shutdown" => HandleOutcome::Shutdown,

        "tools/list" => HandleOutcome::Reply(reply(id, json!({ "tools": tools::descriptors() }))),
        "prompts/list" => HandleOutcome::Reply(reply(id, json!({ "prompts": tools::prompts() }))),
        "resources/list" => match registry {
            Some(registry) => HandleOutcome::Reply(reply(
                id,
                json!({ "resources": tools::resources(registry) }),
            )),
            None => HandleOutcome::Reply(error(id, -32002, tools::NO_REGISTRY)),
        },

        "resources/read" => {
            let Some(uri) = params["uri"].as_str() else {
                return HandleOutcome::Reply(error(id, -32602, "resources/read needs a uri"));
            };
            let Some(registry) = registry else {
                return HandleOutcome::Reply(error(id, -32002, tools::NO_REGISTRY));
            };
            match tools::read_resource(registry, uri) {
                Ok((text, mime)) => HandleOutcome::Reply(reply(
                    id,
                    json!({ "contents": [{ "uri": uri, "mimeType": mime, "text": text }] }),
                )),
                Err(message) => HandleOutcome::Reply(error(id, -32002, &message)),
            }
        }

        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return HandleOutcome::Reply(error(id, -32602, "tools/call needs a name"));
            };
            match tools::call(name, &params["arguments"], registry) {
                Ok(text) => HandleOutcome::Reply(tool_result(id, &text, false)),
                Err(message) => HandleOutcome::Reply(tool_result(id, &message, true)),
            }
        }

        other => HandleOutcome::Reply(error(id, -32601, &format!("method not found: {other}"))),
    }
}
