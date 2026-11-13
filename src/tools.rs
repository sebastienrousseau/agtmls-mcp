// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tools, resources and prompts exposed over MCP.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use agtmls_core::{Analyzer, RuleSet, digest, lockfile, skill};
use serde_json::{Value, json};

use crate::registry::Registry;
use crate::schemas;

/// Returned when a request needs a registry and none was found.
pub const NO_REGISTRY: &str = "no registry loaded. Set AGTMLS_HOME, or pass --registry, pointing at a checkout of \
     https://github.com/sebastienrousseau/agtmls";

fn rules() -> Result<RuleSet, String> {
    // The rule set is data in agtmls-spec, loaded identically by every
    // implementation. Without it this server would report a clean audit
    // because it has no rules, which is indistinguishable from a clean skill.
    let dir = std::env::var("AGTMLS_SPEC")
        .map(std::path::PathBuf::from)
        .ok()
        .filter(|p| p.join("rules").is_dir())
        .ok_or_else(|| {
            "no rule set. Set AGTMLS_SPEC to a checkout of \
             https://github.com/sebastienrousseau/agtmls-spec. Refusing to audit with no \
             rules: the result would look exactly like a clean skill."
                .to_owned()
        })?;
    RuleSet::load(&dir.join("rules")).map_err(|e| format!("cannot load rules: {e}"))
}

/// Tool descriptors, for `tools/list`.
#[must_use]
pub fn descriptors() -> Vec<Value> {
    vec![
        schemas::search_descriptor(),
        schemas::show_descriptor(),
        schemas::audit_descriptor(),
        schemas::digest_descriptor(),
        schemas::verify_descriptor(),
    ]
}

/// Prompt descriptors, for `prompts/list`.
#[must_use]
pub fn prompts() -> Vec<Value> {
    vec![
        json!({
            "name": "review-skill",
            "description": "Review a skill for correctness, routing quality and safety before publication.",
            "arguments": [{ "name": "name", "description": "Skill to review", "required": true }],
        }),
        json!({
            "name": "harden-skill",
            "description": "Narrow a skill's declared capabilities to the minimum its text actually needs.",
            "arguments": [{ "name": "name", "description": "Skill to harden", "required": true }],
        }),
    ]
}

/// Render a named prompt for `prompts/get`.
///
/// # Errors
/// Returns a message suitable for JSON-RPC `-32602` when the prompt name is
/// unknown or the required `name` argument is missing or blank.
pub fn get_prompt(name: &str, args: &Value, registry: Option<&Registry>) -> Result<Value, String> {
    let (description, rubric) = match name {
        "review-skill" => (
            "Review a skill for correctness, routing quality and safety before publication.",
            "1. Routing clarity: verify the frontmatter `name` and `description` state when \
             to load the skill and when not to.\n\
             2. Integrity: confirm `agtmls_digest` agrees with `index.json`.\n\
             3. Static safety: run `agtmls_audit` and resolve every finding.\n\
             4. Evidence: check that commands, gates and examples are reproducible.",
        ),
        "harden-skill" => (
            "Narrow a skill's declared capabilities to the minimum its text actually needs.",
            "1. Inventory every tool, filesystem write, command execution and network access \
             implied by the skill text.\n\
             2. Compare that inventory against `allowed_tools` and `risk` via `agtmls_show`.\n\
             3. Propose the smallest `allowed_tools` set and narrowest guardrails that keep \
             the workflow functional.",
        ),
        other => {
            return Err(format!(
                "unknown prompt `{other}`; expected `review-skill` or `harden-skill`"
            ));
        }
    };
    let skill_name = args["name"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing required `name` argument for prompt `{name}`"))?;

    let context = match registry.and_then(|r| r.read_skill_file(skill_name, "SKILL.md").ok()) {
        Some(markdown) => {
            format!("Skill `{skill_name}` (`SKILL.md`):\n\n```markdown\n{markdown}\n```")
        }
        None => format!(
            "Target skill: `{skill_name}` (not preloaded from the registry; inspect \
             `agtmls_show` / `agtmls_audit` or provide its `SKILL.md` content)."
        ),
    };
    let text = format!("{description}\n\n{context}\n\nChecklist:\n{rubric}");
    Ok(json!({
        "description": description,
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": text },
        }],
    }))
}

/// Resource descriptors, for `resources/list`.
#[must_use]
pub fn resources(registry: &Registry) -> Vec<Value> {
    let mut items = vec![json!({
        "uri": "agtmls://index",
        "name": "Registry index",
        "description": format!("All {} skills, version {}", registry.skills.len(), registry.version),
        "mimeType": "application/json",
    })];
    items.extend(registry.skills.iter().map(|skill| {
        json!({
            "uri": format!("agtmls://skill/{}", skill.name),
            "name": skill.name,
            "description": skill.description,
            "mimeType": "text/markdown",
        })
    }));
    items
}

/// Resolve a resource URI to its content and MIME type.
///
/// # Errors
/// Returns a message if the URI is unknown or the file cannot be read.
pub fn read_resource(registry: &Registry, uri: &str) -> Result<(String, String), String> {
    if uri == "agtmls://index" {
        let text = std::fs::read_to_string(registry.root().join("index.json"))
            .map_err(|e| format!("cannot read index.json: {e}"))?;
        return Ok((text, "application/json".to_owned()));
    }
    let Some(rest) = uri.strip_prefix("agtmls://skill/") else {
        return Err(format!("unknown resource uri: {uri}"));
    };
    let (name, relative) = rest.split_once('/').unwrap_or((rest, "SKILL.md"));
    let text = registry.read_skill_file(name, relative)?;
    let mime = if relative.to_ascii_lowercase().ends_with(".json") {
        "application/json"
    } else {
        "text/markdown"
    };
    Ok((text, mime.to_owned()))
}

fn findings_payload(findings: &[agtmls_core::Finding]) -> (String, Value) {
    let structured_items: Vec<Value> = findings
        .iter()
        .map(|f| {
            json!({
                "severity": f.severity,
                "file": f.file.display().to_string(),
                "line": f.line,
                "rule": f.rule,
                "message": f.message,
            })
        })
        .collect();
    let structured = json!({
        "count": findings.len(),
        "findings": structured_items,
    });
    if findings.is_empty() {
        return ("No findings.".to_owned(), structured);
    }
    let mut out = format!("{} finding(s):\n", findings.len());
    for f in findings {
        let _ = writeln!(
            out,
            "  [{}] {}:{} ({}) {}",
            f.severity,
            f.file.display(),
            f.line,
            f.rule,
            f.message
        );
    }
    (out, structured)
}

fn required_str<'a>(args: &'a Value, key: &str, tool: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{tool} missing required non-empty `{key}` string argument"))
}

fn call_search(args: &Value, registry: Option<&Registry>) -> Result<(String, Value), String> {
    let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
    let query = required_str(args, "query", "agtmls_search")?;
    let limit = usize::try_from(args["limit"].as_u64().unwrap_or(10))
        .unwrap_or(10)
        .clamp(1, 50);
    let hits = registry.search(query, limit);
    let structured = json!({ "results": hits });
    let text = if hits.is_empty() {
        format!("No skill matches {query:?}.")
    } else {
        serde_json::to_string_pretty(&hits).unwrap_or_default()
    };
    Ok((text, structured))
}

fn call_show(args: &Value, registry: Option<&Registry>) -> Result<(String, Value), String> {
    let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
    let name = required_str(args, "name", "agtmls_show")?;
    let skill = registry
        .get(name)
        .ok_or_else(|| format!("no skill named {name:?} in registry; pass a valid skill `name`"))?;
    let structured = serde_json::to_value(skill).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(skill).unwrap_or_default();
    Ok((text, structured))
}

fn call_digest(args: &Value, registry: Option<&Registry>) -> Result<(String, Value), String> {
    let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
    let name = required_str(args, "name", "agtmls_digest")?;
    let skill = registry
        .get(name)
        .ok_or_else(|| format!("no skill named {name:?} in registry; pass a valid skill `name`"))?;
    let computed = digest::skill_digest(&registry.root().join(&skill.path))
        .map_err(|e| format!("cannot digest skill `{name}`: {e}"))?;
    let recorded = skill.integrity.as_deref().unwrap_or("(not in index)");
    let agrees = skill.integrity.as_deref() == Some(computed.as_str());
    let text = format!("computed: {computed}\nindex:    {recorded}\nmatch:    {agrees}");
    let structured = json!({
        "computed": computed,
        "index": recorded,
        "match": agrees,
    });
    Ok((text, structured))
}

fn audit_skill_dir(analyzer: &Analyzer, skill_dir: &Path) -> (String, Value) {
    let mut files: skill::SkillFiles = BTreeMap::new();
    for entry in ["SKILL.md", "metadata.json"] {
        if let Ok(text) = std::fs::read_to_string(skill_dir.join(entry)) {
            files.insert(entry.to_owned(), text);
        }
    }
    let mut findings = Vec::new();
    for (path, content) in &files {
        findings.extend(analyzer.audit_str(path, content));
    }
    findings.extend(skill::audit_skill(&files));
    findings_payload(&findings)
}

fn non_empty_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn call_audit(args: &Value, registry: Option<&Registry>) -> Result<(String, Value), String> {
    let analyzer = Analyzer::new(rules()?);
    if let Some(content) = args["content"].as_str().filter(|s| !s.is_empty()) {
        let filename = non_empty_arg(args, "filename").unwrap_or("SKILL.md");
        return Ok(findings_payload(&analyzer.audit_str(filename, content)));
    }
    let Some(name) = non_empty_arg(args, "name") else {
        return Err(
            "agtmls_audit missing required input: pass `name` to audit a registry skill or `content` to audit text directly"
                .to_owned(),
        );
    };
    let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
    let skill = registry
        .get(name)
        .ok_or_else(|| format!("no skill named {name:?} in registry; pass a valid skill `name`"))?;
    Ok(audit_skill_dir(
        &analyzer,
        &registry.root().join(&skill.path),
    ))
}

fn agent_dot_dir(agent: &str) -> Result<&'static str, String> {
    match agent {
        "claude" => Ok(".claude"),
        "codex" => Ok(".codex"),
        "aider" => Ok(".aider"),
        other => Err(format!(
            "invalid `agent` value {other:?}; must be one of `claude`, `codex`, or `aider`"
        )),
    }
}

fn format_verify_problems(problems: &[String], integrity_failures: usize) -> String {
    if problems.is_empty() {
        return "OK: every recorded skill matches the lockfile.".to_owned();
    }
    let mut out = String::new();
    for line in problems {
        let _ = writeln!(out, "{line}");
    }
    let _ = write!(
        out,
        "\n{} problem(s); {integrity_failures} affect integrity.",
        problems.len(),
    );
    out
}

fn call_verify(args: &Value) -> Result<(String, Value), String> {
    let target = Path::new(required_str(args, "target", "agtmls_verify")?);
    let dot = agent_dot_dir(args["agent"].as_str().unwrap_or("claude"))?;
    let skills_dir = target.join(dot).join("skills");
    let problems = match lockfile::verify(target, &skills_dir) {
        Ok(items) => items,
        Err(e) => {
            return Err(format!(
                "cannot verify `target` directory {}: {e}; pass a repository root containing .agtmls/manifest.json",
                target.display()
            ));
        }
    };
    let mut integrity_failures = 0usize;
    let mut problem_strings = Vec::with_capacity(problems.len());
    for problem in &problems {
        if problem.is_integrity_failure() {
            integrity_failures += 1;
        }
        problem_strings.push(format!("{problem:?}"));
    }
    let text = format_verify_problems(&problem_strings, integrity_failures);
    let structured = json!({
        "ok": problems.is_empty(),
        "problem_count": problems.len(),
        "integrity_failures": integrity_failures,
        "problems": problem_strings,
    });
    Ok((text, structured))
}

/// Dispatch a tool call and return both human-readable text and structured JSON.
///
/// # Errors
/// Returns the actionable message to surface to the model when a call cannot complete.
pub fn call_structured(
    name: &str,
    args: &Value,
    registry: Option<&Registry>,
) -> Result<(String, Value), String> {
    match name {
        "agtmls_search" => call_search(args, registry),
        "agtmls_show" => call_show(args, registry),
        "agtmls_digest" => call_digest(args, registry),
        "agtmls_audit" => call_audit(args, registry),
        "agtmls_verify" => call_verify(args),
        other => Err(format!(
            "unknown tool: {other}; expected agtmls_search, agtmls_show, agtmls_audit, agtmls_digest, or agtmls_verify"
        )),
    }
}

/// Dispatch a tool call, returning its text representation.
///
/// # Errors
/// Returns the message to surface to the model. Tool failures are content,
/// not protocol errors: the model is meant to read them and decide.
pub fn call(name: &str, args: &Value, registry: Option<&Registry>) -> Result<String, String> {
    call_structured(name, args, registry).map(|(text, _)| text)
}
