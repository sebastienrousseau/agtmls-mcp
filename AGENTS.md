<!-- SPDX-FileCopyrightText: 2026 Sebastien Rousseau <sebastian.rousseau@gmail.com> -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# AGENTS.md

Invariants for AI-assisted contributions to `agtmls-mcp`. Read this before changing anything.

Everything here applies equally to humans and automated agents. It is addressed to agents because agents can make breaking changes across multiple files before anyone notices.

## 1. Core Invariants

1. **Strict SemVer sequencing policy**: Public releases stay on the `0.0.x` line and increment strictly by `0.0.1`. Never manually edit version numbers outside the active release branch `feat/v<next-version>`. `v0.1.0` is forbidden until `v0.0.999` exists.
2. **Single Active Release PR Invariant**: Across all repositories, there MUST be at most ONE active pull request targeting `main`, which MUST be the release iteration branch `feat/v<next-version>`.
3. **Dual licensing**: The repository is dual-licensed under Apache-2.0 OR MIT. All files must declare an SPDX license header.
4. **Single source of truth**: The version in `Cargo.toml` is the single source of truth. It must agree with `glama.json`, `server.json`, `CITATION.cff`, and `CHANGELOG.md`.
5. **Zero unsafe code**: The entire crate enforces `#![forbid(unsafe_code)]`. No unsafe blocks are permitted under any circumstances.
6. **Deterministic skill registry access**: Skills search, display, digest computation, static audit, and lockfile verification must remain offline, deterministic, and read-only.

## 2. Before You Claim To Be Done (Verification Gates)

Before concluding any task or preparing a commit, run:

```console
make check
```

Or run the individual gates:

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

All unit tests and protocol tests must pass with 0 failures.

## 3. Hygiene First

Before any feature, fix, or release work, check repository health:
1. Verify CI is green on `main`.
2. Ensure linter and formatter pass without warnings or new suppressions.
3. Preserve all documentation figures, architecture records, and test counts.

## 4. Things That Look Like Bugs and Are Not

- **Missing registry behavior**: If `AGTMLS_HOME` is not set and `--registry` is not passed, the server launches and reports missing registry per-request rather than crashing on startup. This allows MCP clients to connect before configuring the registry path.
- **Rule set requirement**: `agtmls_audit` requires `AGTMLS_SPEC` pointing to rules; without rules it intentionally refuses to run so as not to report false clean audits.
