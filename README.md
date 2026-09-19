<!-- SPDX-FileCopyrightText: 2026 Sebastien Rousseau -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

<h1 align="center">agtmls-mcp</h1>

<p align="center">
  Model Context Protocol server for the AgtMLS skill registry — search, read,
  audit and verify skills over JSON-RPC.
</p>

---

## Why

By 2027, "an agent discovers a capability" will mean "an agent queried an MCP
server", not "a shell script made some symlinks". A registry that can only copy
files into a directory is cornered: it cannot answer *which* skill, it cannot
say whether the copy is still what was published, and it cannot be asked
anything at all.

## Install

```bash
cargo install agtmls-mcp
claude mcp add agtmls -- agtmls-mcp --registry ~/dev/agtmls
```

```jsonc
// or, for any MCP client that takes a config file
{
  "mcpServers": {
    "agtmls": {
      "command": "agtmls-mcp",
      "env": {
        "AGTMLS_HOME": "/path/to/agtmls",
        "AGTMLS_SPEC": "/path/to/agtmls-spec"
      }
    }
  }
}
```

`AGTMLS_SPEC` is required by `agtmls_audit`. Without a rule set an audit would
report clean because it has *no rules*, which is indistinguishable from a clean
skill — so the tool refuses rather than answering.

## Tools

| Tool | Answers |
| :--- | :--- |
| `agtmls_search` | Which skill? A ranked shortlist, each with its content digest and declared risk |
| `agtmls_show` | The full record for one skill |
| `agtmls_audit` | Is this content safe? Findings with stable rule ids, for a skill or for text you already have |
| `agtmls_digest` | What is this skill's content address, and does it still match the index? |
| `agtmls_verify` | Is this installed tree still what was published? |

## Resources

| URI | Content |
| :--- | :--- |
| `agtmls://index` | The registry index |
| `agtmls://skill/{name}` | That skill's `SKILL.md` |
| `agtmls://skill/{name}/{file}` | Another file belonging to the skill |

Resource URIs arrive from the client, so they are untrusted input. Paths
containing `..`, absolute paths and symlinks are refused rather than resolved —
cheaper to reason about than canonicalising, and not defeatable by a symlink
planted inside a skill.

## What it will not do

- **It never writes to the registry.** A server that can modify the registry it
  vouches for is not a useful thing to trust.
- **It makes no network calls and collects nothing.** A local-first tool that
  phones home has traded its only durable asset.

Both are properties of the code, not promises: there is no HTTP client in the
dependency tree.

## Architecture

Newline-delimited JSON-RPC 2.0 over stdio. All dispatch lives in the library
crate so `cargo test` exercises it directly; the binary is a transport shim —
read a line, hand it to `handle_message`, write the reply if there is one.

A tool failure is returned as content with `isError: true`, never as a JSON-RPC
error. The model is meant to read the failure and decide what to do next; a
protocol error takes that choice away from it.

## Relationship to the rest

```
agtmls-spec     rules + conformance corpus, the normative source
   ├── agtmls        the registry and its Python CLI
   └── agtmls-core   the Rust engine
          ├── agtmls-mcp    this server
          └── agtmls-wasm   @agtmls/wasm
```

`agtmls-core` is pinned by revision until it is published to crates.io. Strict
lockstep is deliberate: this server reports findings by rule id, so a core with
a different rule set would make its answers differ from the CLI's for the same
input.

## Licence

Apache-2.0 OR MIT.
