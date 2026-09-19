// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading the registry this server serves.
//!
//! The registry is a checkout of
//! [`agtmls`](https://github.com/sebastienrousseau/agtmls), located by
//! `AGTMLS_HOME` or `--registry`. It is read, never written: an MCP server
//! that can modify the registry it vouches for is not a useful thing to
//! trust.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// A skill, as recorded in `index.json`.
#[derive(Debug, Clone, Serialize)]
pub struct Skill {
    /// Skill name, unique within the registry.
    pub name: String,
    /// One-line description; the text a router matches against.
    pub description: String,
    /// Path relative to the registry root.
    pub path: String,
    /// Bundle membership, or `None` for a general skill.
    pub bundle: Option<String>,
    /// Content address, per `agtmls-spec` 3.1.
    pub integrity: Option<String>,
    /// Declared risk level.
    pub risk: Option<String>,
    /// Tools the published frontmatter grants.
    pub allowed_tools: Vec<String>,
}

/// The registry index.
#[derive(Debug, Clone)]
pub struct Registry {
    root: PathBuf,
    /// Skills, in index order.
    pub skills: Vec<Skill>,
    /// Registry version.
    pub version: String,
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

impl Registry {
    /// Load `index.json` from a registry checkout.
    ///
    /// # Errors
    /// Returns a message if the path is not a registry or the index is
    /// unreadable. The caller surfaces it rather than serving an empty
    /// registry: a server that answers "no skills" because it could not find
    /// any is indistinguishable, to a model, from one that found none.
    pub fn open(root: &Path) -> Result<Self, String> {
        let index_path = root.join("index.json");
        let text = std::fs::read_to_string(&index_path)
            .map_err(|e| format!("cannot read {}: {e}", index_path.display()))?;
        let index: Value = serde_json::from_str(&text)
            .map_err(|e| format!("index.json is not valid JSON: {e}"))?;

        let skills = index["skills"]
            .as_array()
            .ok_or_else(|| "index.json has no skills array".to_owned())?
            .iter()
            .map(|s| Skill {
                name: s["name"].as_str().unwrap_or_default().to_owned(),
                description: s["description"].as_str().unwrap_or_default().to_owned(),
                path: s["path"].as_str().unwrap_or_default().to_owned(),
                bundle: s["bundle"].as_str().map(str::to_owned),
                integrity: s["integrity"].as_str().map(str::to_owned),
                risk: s["safety_policy"]["risk_level"].as_str().map(str::to_owned),
                allowed_tools: strings(s.get("allowed_tools")),
            })
            .collect();

        Ok(Self {
            root: root.to_path_buf(),
            skills,
            version: index["registry_version"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned(),
        })
    }

    /// The registry root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Look a skill up by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Rank skills against a query.
    ///
    /// Deliberately a substring-and-term score rather than anything cleverer:
    /// the client is a language model, which is far better at choosing between
    /// ten plausible candidates than this function is at guessing which one it
    /// meant. Returning a defensible shortlist beats returning a confident
    /// wrong answer.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Skill> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|t| t.len() > 1)
            .collect();
        if terms.is_empty() {
            return self.skills.iter().take(limit).collect();
        }

        let mut scored: Vec<(usize, &Skill)> = self
            .skills
            .iter()
            .filter_map(|skill| {
                let name = skill.name.to_lowercase();
                let haystack = format!(
                    "{name} {} {}",
                    skill.description.to_lowercase(),
                    skill.bundle.as_deref().unwrap_or("")
                );
                let score: usize = terms
                    .iter()
                    .map(|term| {
                        if name == *term {
                            100 // an exact name match is what the caller meant
                        } else if name.contains(term) {
                            10
                        } else {
                            usize::from(haystack.contains(term))
                        }
                    })
                    .sum();
                (score > 0).then_some((score, skill))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        scored.into_iter().take(limit).map(|(_, s)| s).collect()
    }

    /// Read a file belonging to a skill.
    ///
    /// # Errors
    /// Returns a message if the skill is unknown, the path escapes the skill
    /// directory, or the file cannot be read.
    pub fn read_skill_file(&self, name: &str, relative: &str) -> Result<String, String> {
        let skill = self
            .get(name)
            .ok_or_else(|| format!("no skill named {name:?}"))?;
        let base = self.root.join(&skill.path);

        // A resource URI arrives from the client, so it is untrusted input.
        // Rejecting traversal components outright is cheaper to reason about
        // than canonicalising and comparing, and cannot be defeated by a
        // symlink planted inside the skill.
        if relative
            .split('/')
            .any(|part| part == ".." || part.is_empty() || part.starts_with('/'))
        {
            return Err(format!(
                "refusing a path that escapes the skill: {relative:?}"
            ));
        }
        let target = base.join(relative);
        if std::fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!("refusing to follow a symlink: {relative:?}"));
        }
        std::fs::read_to_string(&target).map_err(|e| format!("cannot read {relative}: {e}"))
    }
}
