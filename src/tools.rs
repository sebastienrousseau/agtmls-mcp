// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tools, resources and prompts exposed over MCP.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use agtmls_core::{Analyzer, RuleSet, digest, lockfile, skill};
use serde_json::{Value, json};

use crate::registry::Registry;

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
        json!({
            "name": "agtmls_search",
            "description": "Search the skill registry. Returns a ranked shortlist with each skill's \
                            content digest and declared risk level, so the caller can choose.",
            "inputSchema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string", "description": "Free text; terms are matched against name, description and bundle." },
                    "limit": { "type": "integer", "default": 10, "minimum": 1, "maximum": 50 },
                },
            },
        }),
        json!({
            "name": "agtmls_show",
            "description": "Full record for one skill, including its content digest and the tools \
                            its frontmatter grants.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } },
            },
        }),
        json!({
            "name": "agtmls_audit",
            "description": "Statically analyse skill content for prompt injection, invisible-character \
                            smuggling, capability escalation and unsafe execution. Pass `content` to \
                            audit text you already have, or `name` to audit a skill in the registry. \
                            Nothing is sent anywhere.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "A skill in the registry." },
                    "content": { "type": "string", "description": "Text to analyse directly." },
                    "filename": { "type": "string", "default": "SKILL.md", "description": "Used to decide which rules apply." },
                },
            },
        }),
        json!({
            "name": "agtmls_digest",
            "description": "Content address for a skill in the registry, per agtmls-spec 3.1. \
                            Identical to what the CLI computes.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } },
            },
        }),
        json!({
            "name": "agtmls_verify",
            "description": "Check an installed tree against its .agtmls/manifest.json lockfile. \
                            Reports modification, deletion and unmanaged skills; never repairs and \
                            never deletes.",
            "inputSchema": {
                "type": "object",
                "required": ["target"],
                "properties": {
                    "target": { "type": "string", "description": "Repository root containing .agtmls/manifest.json." },
                    "agent": { "type": "string", "enum": ["claude", "codex", "aider"], "default": "claude" },
                },
            },
        }),
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

fn findings_report(findings: &[agtmls_core::Finding]) -> String {
    if findings.is_empty() {
        return "No findings.".to_owned();
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
    out
}

/// Dispatch a tool call.
///
/// # Errors
/// Returns the message to surface to the model. Tool failures are content,
/// not protocol errors: the model is meant to read them and decide.
pub fn call(name: &str, args: &Value, registry: Option<&Registry>) -> Result<String, String> {
    match name {
        "agtmls_search" => {
            let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
            let query = args["query"]
                .as_str()
                .ok_or("agtmls_search needs a query")?;
            let limit = usize::try_from(args["limit"].as_u64().unwrap_or(10))
                .unwrap_or(10)
                .clamp(1, 50);
            let hits = registry.search(query, limit);
            if hits.is_empty() {
                return Ok(format!("No skill matches {query:?}."));
            }
            Ok(serde_json::to_string_pretty(&hits).unwrap_or_default())
        }
        "agtmls_show" => {
            let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
            let name = args["name"].as_str().ok_or("agtmls_show needs a name")?;
            let skill = registry
                .get(name)
                .ok_or_else(|| format!("no skill named {name:?}"))?;
            Ok(serde_json::to_string_pretty(skill).unwrap_or_default())
        }
        "agtmls_digest" => {
            let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
            let name = args["name"].as_str().ok_or("agtmls_digest needs a name")?;
            let skill = registry
                .get(name)
                .ok_or_else(|| format!("no skill named {name:?}"))?;
            let computed = digest::skill_digest(&registry.root().join(&skill.path))
                .map_err(|e| format!("cannot digest {name}: {e}"))?;
            let recorded = skill.integrity.as_deref().unwrap_or("(not in index)");
            let agrees = skill.integrity.as_deref() == Some(computed.as_str());
            Ok(format!(
                "computed: {computed}\nindex:    {recorded}\nmatch:    {agrees}"
            ))
        }
        "agtmls_audit" => {
            let analyzer = Analyzer::new(rules()?);
            if let Some(content) = args["content"].as_str() {
                let filename = args["filename"].as_str().unwrap_or("SKILL.md");
                return Ok(findings_report(&analyzer.audit_str(filename, content)));
            }
            let registry = registry.ok_or_else(|| NO_REGISTRY.to_owned())?;
            let name = args["name"]
                .as_str()
                .ok_or("agtmls_audit needs name or content")?;
            let skill_dir = {
                let skill = registry
                    .get(name)
                    .ok_or_else(|| format!("no skill named {name:?}"))?;
                registry.root().join(&skill.path)
            };
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
            Ok(findings_report(&findings))
        }
        "agtmls_verify" => {
            let target = Path::new(
                args["target"]
                    .as_str()
                    .ok_or("agtmls_verify needs a target")?,
            );
            let agent = args["agent"].as_str().unwrap_or("claude");
            let dot = match agent {
                "claude" => ".claude",
                "codex" => ".codex",
                "aider" => ".aider",
                other => return Err(format!("unknown agent {other:?}")),
            };
            let problems = lockfile::verify(target, &target.join(dot).join("skills"))?;
            if problems.is_empty() {
                return Ok("OK: every recorded skill matches the lockfile.".to_owned());
            }
            let mut out = String::new();
            for problem in &problems {
                let _ = writeln!(out, "{problem:?}");
            }
            let _ = write!(
                out,
                "\n{} problem(s); {} affect integrity.",
                problems.len(),
                problems.iter().filter(|p| p.is_integrity_failure()).count()
            );
            Ok(out)
        }
        other => Err(format!("unknown tool: {other}")),
    }
}
