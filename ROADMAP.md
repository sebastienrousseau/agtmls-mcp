<!-- SPDX-FileCopyrightText: 2026 Sebastien Rousseau -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Roadmap

## Now

See [CHANGELOG.md](CHANGELOG.md) for what exists.

## Next

- **Pin `agtmls-core` by version.** It is pinned by git revision until that
  crate reaches crates.io. A revision pin is reproducible, but it is not
  something a downstream consumer can audit as easily as a registry version.
- **Publish.** The release workflow uses Trusted Publishing and needs a
  one-time registration; see `agtmls/docs/PUBLISHING.md`.

## Not planned

- **Reimplementing any rule.** Rules are data in
  [`agtmls-spec`](https://github.com/sebastienrousseau/agtmls-spec). A second
  copy is a second copy that will drift.
