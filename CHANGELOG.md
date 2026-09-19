<!-- SPDX-FileCopyrightText: 2026 Sebastien Rousseau -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Changelog

## Unreleased

### Added

- JSON-RPC 2.0 over stdio, MCP protocol `2025-06-18`.
- Tools: `agtmls_search`, `agtmls_show`, `agtmls_audit`, `agtmls_digest`,
  `agtmls_verify`.
- Resources: `agtmls://index` and `agtmls://skill/{name}[/{file}]`.
- Prompts: `review-skill`, `harden-skill`.
- `agtmls_audit` refuses to run without a rule set. An audit with no rules
  reports clean, which is indistinguishable from a clean skill.
- Resource URIs are treated as untrusted input: `..`, absolute paths and
  symlinks are refused rather than resolved.
- Tool failures are returned as content with `isError`, not as protocol
  errors, so the model can read them and decide.
- Eight protocol tests, including path traversal and the
  no-registry-configured path, which is the failure a user actually hits.
