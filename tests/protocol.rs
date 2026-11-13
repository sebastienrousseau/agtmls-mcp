// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON-RPC surface tests.
//!
//! These drive `handle_message` directly rather than a subprocess, so a
//! failure points at a handler instead of at a pipe. `tests/stdio.rs` covers
//! the transport shim end to end.

use agtmls_mcp::{HandleOutcome, PROTOCOL_VERSION, handle_message, registry::Registry};
use serde_json::{Value, json};

fn call(message: &Value, registry: Option<&Registry>) -> Value {
    match handle_message(&message.to_string(), registry) {
        HandleOutcome::Reply(reply) => serde_json::from_str(&reply).expect("reply is valid JSON"),
        HandleOutcome::Silent => Value::Null,
        HandleOutcome::Shutdown => json!({ "shutdown": true }),
    }
}

fn registry() -> Option<Registry> {
    for candidate in ["../../Python/agtmls", "../agtmls"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(candidate);
        if path.join("index.json").exists() {
            return Registry::open(&path).ok();
        }
    }
    std::env::var("AGTMLS_HOME")
        .ok()
        .and_then(|p| Registry::open(std::path::Path::new(&p)).ok())
}

#[test]
fn initialize_reports_the_protocol_version_and_capabilities() {
    let reply = call(&json!({"jsonrpc":"2.0","id":1,"method":"initialize"}), None);
    assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(reply["result"]["serverInfo"]["name"], "agtmls-mcp");
    for capability in ["tools", "resources", "prompts"] {
        assert!(
            reply["result"]["capabilities"].get(capability).is_some(),
            "missing capability {capability}"
        );
    }
}

#[test]
fn malformed_input_gets_a_jsonrpc_error_not_a_dropped_connection() {
    // A server that closes on bad input teaches the client nothing about what
    // was wrong with it.
    let HandleOutcome::Reply(reply) = handle_message("{ this is not json", None) else {
        panic!("expected a reply");
    };
    let value: Value = serde_json::from_str(&reply).expect("the error is still valid JSON-RPC");
    assert_eq!(value["error"]["code"], -32700);

    let missing_method = call(&json!({"jsonrpc":"2.0","id":7}), None);
    assert_eq!(missing_method["error"]["code"], -32600);

    let unknown = call(&json!({"jsonrpc":"2.0","id":8,"method":"nope"}), None);
    assert_eq!(unknown["error"]["code"], -32601);
}

#[test]
fn notifications_are_not_answered() {
    assert_eq!(
        handle_message(
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
            None
        ),
        HandleOutcome::Silent,
        "a notification has no id, so answering it is a protocol violation"
    );
}

#[test]
fn every_tool_declares_a_usable_schema() {
    let reply = call(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}), None);
    let tools = reply["result"]["tools"].as_array().expect("tools array");
    assert!(
        tools.len() >= 5,
        "expected the full tool set, got {}",
        tools.len()
    );
    for tool in tools {
        let name = tool["name"].as_str().expect("tool name");
        assert!(name.starts_with("agtmls_"), "{name} is not namespaced");
        assert!(
            tool["description"].as_str().is_some_and(|d| d.len() > 40),
            "{name} has a description too short to route on"
        );
        assert_eq!(
            tool["inputSchema"]["type"], "object",
            "{name} has no object schema"
        );
    }
}

#[test]
fn a_request_needing_a_registry_says_so_when_there_is_none() {
    // The failure a user actually hits: the server started before it was
    // pointed at anything. It must be actionable, not "internal error".
    let reply = call(
        &json!({"jsonrpc":"2.0","id":3,"method":"resources/list"}),
        None,
    );
    let message = reply["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("AGTMLS_HOME"),
        "unhelpful message: {message}"
    );
}

#[test]
fn a_tool_failure_is_content_not_a_protocol_error() {
    // The model is meant to read the failure and decide what to do. A
    // JSON-RPC error takes that choice away from it.
    let reply = call(
        &json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
               "params":{"name":"agtmls_show","arguments":{"name":"nope"}}}),
        None,
    );
    assert!(
        reply["error"].is_null(),
        "tool failure leaked as a protocol error"
    );
    assert_eq!(reply["result"]["isError"], true);
    assert!(reply["result"]["content"][0]["text"].as_str().is_some());
}

#[test]
fn resource_uris_cannot_escape_the_skill_directory() {
    let Some(registry) = registry() else {
        panic!(
            "no agtmls registry found. Set AGTMLS_HOME. Refusing to skip: a path-traversal \
             test that silently does not run is worse than none."
        );
    };
    let name = &registry.skills.first().expect("at least one skill").name;
    for escape in [
        "../../../../etc/passwd",
        "..%2F..%2Fetc%2Fpasswd",
        "/etc/passwd",
    ] {
        let uri = format!("agtmls://skill/{name}/{escape}");
        let reply = call(
            &json!({"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":uri}}),
            Some(&registry),
        );
        assert!(
            !reply["error"].is_null(),
            "traversal {escape:?} was not refused"
        );
    }
}

#[test]
fn search_and_show_answer_from_the_real_registry() {
    let Some(registry) = registry() else {
        panic!("no agtmls registry found. Set AGTMLS_HOME.");
    };
    let reply = call(
        &json!({"jsonrpc":"2.0","id":6,"method":"tools/call",
               "params":{"name":"agtmls_search","arguments":{"query":"debugging"}}}),
        Some(&registry),
    );
    assert_eq!(reply["result"]["isError"], false);
    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("debugging"),
        "search returned nothing useful: {text}"
    );

    // Every skill record must carry its content digest, or a caller cannot
    // tell whether what it fetched is what was published.
    assert!(text.contains("integrity"), "search results carry no digest");
}

#[test]
fn every_tool_declares_parameter_docs_annotations_and_output_schema() {
    let reply = call(
        &json!({"jsonrpc":"2.0","id":10,"method":"tools/list"}),
        None,
    );
    let tools = reply["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 5);
    for tool in tools {
        let name = tool["name"].as_str().expect("tool name");
        assert!(
            tool["title"].as_str().is_some_and(|t| !t.trim().is_empty()),
            "{name} missing title"
        );
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{name}");
        assert_eq!(tool["annotations"]["destructiveHint"], false, "{name}");
        assert_eq!(tool["annotations"]["idempotentHint"], true, "{name}");
        assert_eq!(tool["annotations"]["openWorldHint"], false, "{name}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{name}");

        let props = tool["inputSchema"]["properties"]
            .as_object()
            .expect("properties object");
        for (param, schema) in props {
            assert!(
                schema["description"]
                    .as_str()
                    .is_some_and(|d| !d.trim().is_empty()),
                "{name}.{param} must declare a non-empty description"
            );
        }
    }
}

#[test]
fn prompts_get_renders_messages_and_rejects_missing_arguments() {
    let reg = registry();
    for prompt in ["review-skill", "harden-skill"] {
        let rendered = call(
            &json!({
                "jsonrpc": "2.0",
                "id": 11,
                "method": "prompts/get",
                "params": { "name": prompt, "arguments": { "name": "example" } }
            }),
            reg.as_ref(),
        );
        assert!(rendered["error"].is_null(), "{prompt} failed: {rendered}");
        let msgs = rendered["result"]["messages"]
            .as_array()
            .expect("messages array");
        assert!(!msgs.is_empty(), "{prompt} returned empty messages");
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"]["type"], "text");
        assert!(
            msgs[0]["content"]["text"]
                .as_str()
                .is_some_and(|t| t.contains("example")),
            "{prompt} message did not reference argument: {rendered}"
        );

        let missing = call(
            &json!({
                "jsonrpc": "2.0",
                "id": 12,
                "method": "prompts/get",
                "params": { "name": prompt }
            }),
            reg.as_ref(),
        );
        assert_eq!(
            missing["error"]["code"], -32602,
            "{prompt} must reject missing required argument with -32602: {missing}"
        );
    }

    let unknown = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "prompts/get",
            "params": { "name": "no-such-prompt", "arguments": { "name": "example" } }
        }),
        reg.as_ref(),
    );
    assert_eq!(unknown["error"]["code"], -32602);
}

#[test]
fn tool_calls_emit_structured_content_on_success_and_omit_it_on_error() {
    let reg = registry().expect("agtmls registry available");
    let search = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 14,
            "method": "tools/call",
            "params": { "name": "agtmls_search", "arguments": { "query": "debugging" } }
        }),
        Some(&reg),
    );
    assert_eq!(search["result"]["isError"], false);
    assert!(search["result"]["structuredContent"]["results"].is_array());

    let skill_name = reg.skills.first().expect("skill").name.clone();
    let show = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 15,
            "method": "tools/call",
            "params": { "name": "agtmls_show", "arguments": { "name": skill_name } }
        }),
        Some(&reg),
    );
    assert_eq!(show["result"]["isError"], false);
    assert_eq!(show["result"]["structuredContent"]["name"], skill_name);

    let digest = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 16,
            "method": "tools/call",
            "params": { "name": "agtmls_digest", "arguments": { "name": skill_name } }
        }),
        Some(&reg),
    );
    assert_eq!(digest["result"]["isError"], false);
    assert_eq!(digest["result"]["structuredContent"]["match"], true);

    let failed = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 17,
            "method": "tools/call",
            "params": { "name": "agtmls_show", "arguments": { "name": "no-such-skill" } }
        }),
        Some(&reg),
    );
    assert_eq!(failed["result"]["isError"], true);
    assert!(failed["result"].get("structuredContent").is_none());
}

#[test]
fn verify_supports_native_agents() {
    let list = call(
        &json!({"jsonrpc":"2.0","id":18,"method":"tools/list"}),
        None,
    );
    let verify_tool = list["result"]["tools"]
        .as_array()
        .and_then(|ts| ts.iter().find(|t| t["name"] == "agtmls_verify"))
        .expect("agtmls_verify descriptor");
    let agents = verify_tool["inputSchema"]["properties"]["agent"]["enum"]
        .as_array()
        .expect("agent enum");
    assert!(agents.contains(&json!("antigravity")), "{agents:?}");

    let root = std::env::temp_dir().join(format!("agtmls-mcp-verify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let skill_dir = root.join(".agents/skills/sample");
    std::fs::create_dir_all(&skill_dir).expect("create skill dir");
    std::fs::write(skill_dir.join("SKILL.md"), "# sample\n").expect("write skill");
    let digest = agtmls_core::skill_digest(&skill_dir).expect("digest");
    std::fs::create_dir_all(root.join(".agtmls")).expect("create .agtmls");
    std::fs::write(
        root.join(".agtmls/manifest.json"),
        json!({
            "schema_version": 1,
            "spec_version": "0.1.0",
            "installed_at": "2026-11-13T00:00:00Z",
            "source": { "registry": "https://example.com", "registry_version": "0.0.0" },
            "mode": "copy",
            "skills": [{ "name": "sample", "integrity": digest, "path": "sample", "agents": ["antigravity"] }],
        })
        .to_string(),
    )
    .expect("write manifest");

    let verified = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 19,
            "method": "tools/call",
            "params": {
                "name": "agtmls_verify",
                "arguments": { "target": root.display().to_string(), "agent": "antigravity" }
            }
        }),
        None,
    );
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(verified["result"]["isError"], false, "{verified}");
    assert_eq!(verified["result"]["structuredContent"]["ok"], true);
}

#[test]
fn audit_runs_structural_rules_against_the_registry() {
    if std::env::var_os("AGTMLS_SPEC").is_none() {
        return;
    }
    let reg = registry().expect("agtmls registry available");
    let skill_name = reg.skills.first().expect("skill").name.clone();
    let audited = call(
        &json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": { "name": "agtmls_audit", "arguments": { "name": skill_name } }
        }),
        Some(&reg),
    );
    assert_eq!(audited["result"]["isError"], false, "{audited}");
    assert!(audited["result"]["structuredContent"]["findings"].is_array());
}
