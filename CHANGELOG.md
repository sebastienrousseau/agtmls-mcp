<!-- SPDX-FileCopyrightText: 2026 Sebastien Rousseau -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Changelog

## [0.0.3] - Unreleased

### Changed

- `KEYS.asc` trusts a second maintainer signing key,
  `SHA256:kIOPAavp1TCEauTr1tTIN3cv+tSs6F9m/4lZjuM9tqk`, so release tags can
  be signed on either maintainer machine. Each key line now carries its
  fingerprint for out-of-band checks.
- **The MSRV is now Rust 1.89.0** (was 1.86.0), the floor shared by the
  portfolio's Rust repositories. `rust-version` and the README badge move
  together.

## [0.0.2] - 2026-10-03

### Added

- Terminal demo animation `.github/demo.gif` and VHS tape `.github/demo.tape` with `make demo` target.
- Developer `Makefile` supporting `all`, `check`, `clippy`, `test`, `fmt`, `demo`, and `clean`.
- Directory and registry manifests `glama.json` and `server.json`.
- Complete dual licensing under `Apache-2.0 OR MIT` with `LICENSES/` directory and root `LICENSE`.
- Development invariants and verification gates documentation in `AGENTS.md`.

## [0.0.1] - 2026-09-19

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
