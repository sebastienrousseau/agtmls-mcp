#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Sebastien Rousseau
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -euo pipefail

BIN="${AGTMLS_MCP:-agtmls-mcp}"

{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"demo","version":"0.0.1"}}}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
} | "$BIN" 2>/dev/null
