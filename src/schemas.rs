// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! JSON Schema definitions and behavioural annotations for `agtmls-mcp` tools.

use serde_json::{Value, json};

pub(crate) fn read_only_annotations() -> Value {
    json!({
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false,
    })
}

pub(crate) fn skill_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "description", "path", "allowed_tools"],
        "additionalProperties": false,
        "properties": {
            "name": { "type": "string" },
            "description": { "type": "string" },
            "path": { "type": "string" },
            "bundle": { "type": ["string", "null"] },
            "integrity": { "type": ["string", "null"] },
            "risk": { "type": ["string", "null"] },
            "allowed_tools": { "type": "array", "items": { "type": "string" } },
        },
    })
}

pub(crate) fn search_descriptor() -> Value {
    json!({
        "name": "agtmls_search",
        "title": "Search skills",
        "description": "Search the skill registry. Returns a ranked shortlist with each skill's \
                        content digest and declared risk level, so the caller can choose.",
        "annotations": read_only_annotations(),
        "inputSchema": {
            "type": "object",
            "required": ["query"],
            "additionalProperties": false,
            "properties": {
                "query": {
                    "type": "string",
                    "minLength": 1,
                    "examples": ["debugging"],
                    "description": "Free text; terms are matched against name, description and bundle.",
                },
                "limit": {
                    "type": "integer",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 50,
                    "description": "Maximum number of ranked skills to return (1 to 50).",
                },
            },
        },
        "outputSchema": {
            "type": "object",
            "required": ["results"],
            "additionalProperties": false,
            "properties": {
                "results": { "type": "array", "items": skill_schema() },
            },
        },
    })
}

pub(crate) fn show_descriptor() -> Value {
    json!({
        "name": "agtmls_show",
        "title": "Show skill",
        "description": "Full record for one skill, including its content digest and the tools \
                        its frontmatter grants.",
        "annotations": read_only_annotations(),
        "inputSchema": {
            "type": "object",
            "required": ["name"],
            "additionalProperties": false,
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 1,
                    "examples": ["test-driven-development"],
                    "description": "Skill name in the registry to inspect.",
                },
            },
        },
        "outputSchema": skill_schema(),
    })
}

pub(crate) fn audit_descriptor() -> Value {
    json!({
        "name": "agtmls_audit",
        "title": "Audit skill safety",
        "description": "Statically analyse skill content for prompt injection, invisible-character \
                        smuggling, capability escalation and unsafe execution. Pass `content` to \
                        audit text you already have, or `name` to audit a skill in the registry. \
                        Nothing is sent anywhere.",
        "annotations": read_only_annotations(),
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 1,
                    "examples": ["test-driven-development"],
                    "description": "A skill name in the registry to analyse.",
                },
                "content": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Skill text to analyse directly without reading the registry.",
                },
                "filename": {
                    "type": "string",
                    "default": "SKILL.md",
                    "minLength": 1,
                    "description": "Filename used to decide which static rules apply.",
                },
            },
        },
        "outputSchema": {
            "type": "object",
            "required": ["count", "findings"],
            "additionalProperties": false,
            "properties": {
                "count": { "type": "integer", "minimum": 0 },
                "findings": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["severity", "file", "line", "rule", "message"],
                        "additionalProperties": false,
                        "properties": {
                            "severity": { "type": "string" },
                            "file": { "type": "string" },
                            "line": { "type": "integer", "minimum": 0 },
                            "rule": { "type": "string" },
                            "message": { "type": "string" },
                        },
                    },
                },
            },
        },
    })
}

pub(crate) fn digest_descriptor() -> Value {
    json!({
        "name": "agtmls_digest",
        "title": "Compute skill digest",
        "description": "Content address for a skill in the registry, per agtmls-spec 3.1. \
                        Identical to what the CLI computes.",
        "annotations": read_only_annotations(),
        "inputSchema": {
            "type": "object",
            "required": ["name"],
            "additionalProperties": false,
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 1,
                    "examples": ["test-driven-development"],
                    "description": "Skill name in the registry whose content digest to compute.",
                },
            },
        },
        "outputSchema": {
            "type": "object",
            "required": ["computed", "index", "match"],
            "additionalProperties": false,
            "properties": {
                "computed": { "type": "string" },
                "index": { "type": "string" },
                "match": { "type": "boolean" },
            },
        },
    })
}

pub(crate) fn verify_descriptor() -> Value {
    let agents: Vec<&str> = agtmls_core::lockfile::NATIVE_AGENTS
        .iter()
        .map(|(name, _)| *name)
        .collect();
    json!({
        "name": "agtmls_verify",
        "title": "Verify installed skills",
        "description": "Check an installed tree against its .agtmls/manifest.json lockfile. \
                        Reports modification, deletion and unmanaged skills; never repairs and \
                        never deletes.",
        "annotations": read_only_annotations(),
        "inputSchema": {
            "type": "object",
            "required": ["target"],
            "additionalProperties": false,
            "properties": {
                "target": {
                    "type": "string",
                    "minLength": 1,
                    "examples": ["."],
                    "description": "Repository root containing .agtmls/manifest.json.",
                },
                "agent": {
                    "type": "string",
                    "enum": agents,
                    "default": "claude",
                    "description": "Agent directory layout under the target repository root.",
                },
            },
        },
        "outputSchema": {
            "type": "object",
            "required": ["ok", "problem_count", "integrity_failures", "problems"],
            "additionalProperties": false,
            "properties": {
                "ok": { "type": "boolean" },
                "problem_count": { "type": "integer", "minimum": 0 },
                "integrity_failures": { "type": "integer", "minimum": 0 },
                "problems": { "type": "array", "items": { "type": "string" } },
            },
        },
    })
}
