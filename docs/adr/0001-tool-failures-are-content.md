<!-- SPDX-FileCopyrightText: 2026 Sebastien Rousseau -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# ADR 0001 — A tool failure is content, not a protocol error

**Status:** accepted · **Date:** 2026-09-19

## Context

When `agtmls_show` is asked for a skill that does not exist, JSON-RPC offers
two ways to say so: an `error` object, or a successful result whose content
describes the failure.

## Decision

Tool failures return a successful envelope with `isError: true` and the
message as content. JSON-RPC `error` is reserved for protocol-level faults —
malformed JSON, an unknown method, a missing required parameter.

## Consequences

**Good.** The client here is a language model, and "no skill named 'debuging'"
is information it can act on: it can correct the spelling and retry. A
protocol error is handled by the transport and frequently never reaches the
model at all, so the one participant able to recover is the one not told.

**Costly.** A caller that only checks the JSON-RPC envelope sees success and
must look at `isError` to learn otherwise. That is a real trap for a
hand-written client, and the reason every tool result sets `isError`
explicitly rather than omitting it when false.

**The boundary.** A failure the model can act on is content; a failure that
means the request was malformed is an error. "The registry is not configured"
sits deliberately on the error side: no retry the model constructs will fix
it, and the message names the environment variable a human must set.

## Alternatives rejected

**Everything as a protocol error.** Simpler and uniform. Rejected because it
takes the decision away from the only participant able to make it.
