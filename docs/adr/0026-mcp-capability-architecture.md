# ADR 0026 — MCP capability architecture

| Status | ✅ Accepted (autonomous-execution authority — Track K foundation) |
|---|---|
| Date | 2026-06-03 |
| Author | ai (auditor; under autonomous-execution authorization) |
| Related | [docs/plans/11-track-k.md](../plans/11-track-k.md), [ADR 0014 — MCP defaults](0014-mcp-defaults.md), [ADR 0020 — AI Tool Registry](0020-ai-tool-registry-expansion.md), Working-agreement §21 (MCP rules), Spec §21.3 (MCP sandbox bright line) |

## Context

The Model Context Protocol (MCP) lets users connect third-party MCP servers (e.g. Notion's official MCP server, GitHub's, Linear's) to extend the AI agent's tool surface beyond Mizan's built-in tools. The Mizan Evolution Spec §21 prescribes a gateway architecture where MCP calls go through Mizan Connect (not direct desktop-to-MCP) for egress DLP + rate-limiting + audit logging.

**The MCP sandbox bright line (Spec §21.3, working-agreement §0 rule 6) is non-negotiable**: MCP tools may NEVER write to `truth_ledger`, `holdings`, `activities`, `balances`, or any other financial-state table. They may only read non-financial state + write to a sandboxed `scratchpad` namespace.

This ADR locks the gateway shape, the sandbox enforcement layer, the egress DLP rules, the public catalog process, and the initial-90-day defaults (per memory note `project-mcp-defaults`).

## Decision

### Gateway architecture

```
Desktop  ──RPC──►  Mizan Connect MCP gateway  ──HTTPS──►  Third-party MCP server
                          │
                          ├── Sandbox enforcement layer (BLOCKS truth_ledger / holdings / activities / balances writes)
                          ├── Egress DLP filter (BLOCKS SSN / PAN / Aadhaar / credit-card patterns in outbound payloads)
                          ├── Per-call audit log (mcp_call_log: server_id, tool, params_digest, response_digest, duration_ms, timestamp)
                          ├── Per-user rate limit (60 calls/min)
                          └── Per-call timeout (10s default, 30s max)
```

### Data model (foundation migration already shipped — task tracker #35)

| Table | Purpose |
|---|---|
| `mcp_servers` | (user_id, server_url, auth_method, name, trust_level enum default 'untrusted', last_reviewed_at) |
| `mcp_call_log` | (server_id, tool, params_digest, response_digest, duration_ms, timestamp) |
| `mcp_catalog` | Curated public catalog with security review metadata |
| `mcp_scratchpad` | Sandboxed per-user K/V store; MCP tools' ONLY write target |

### Sandbox enforcement (bright line per working-agreement §0 rule 6)

The MCP gate (`crates/ai/src/dispatcher/mcp_gate.rs`) is the sole code path for MCP-namespaced tools. It enforces:

1. **Write target whitelist:** `mcp_scratchpad` is the ONLY writable table from an MCP tool. Any attempt to write to `truth_ledger`, `holdings`, `activities`, `balances`, `user_memory` (except via the existing memory writer per Track C), or any other table is REJECTED at the dispatcher level — never reaches the handler.
2. **Compile-time check:** the `register_mcp_tool!` macro fails compilation when a tool's handler signature includes any write trait other than `Scratchpad::write`.
3. **Run-time check:** even if a tool somehow tries to circumvent (e.g. through unsafe Rust — banned anyway per working-agreement §17), the trait bounds make the dispatcher unable to pass it a writer for the forbidden tables.

### Egress DLP filter

Outbound payloads pass through the DLP filter (`crates/ai/src/mcp/egress_filter.rs`):

| Pattern | Action |
|---|---|
| US SSN (`\d{3}-\d{2}-\d{4}`) | REJECT outbound; log to `mcp_call_log` with `egress_dlp_blocked` flag |
| Indian PAN (`[A-Z]{5}\d{4}[A-Z]`) | REJECT |
| Indian Aadhaar (`\d{4}\s\d{4}\s\d{4}`) | REJECT |
| Credit card (any major brand Luhn-valid) | REJECT |
| IBAN | REJECT |

A REJECT response surfaces in the agent UI as "Cannot send: data masked by privacy filter" — the user can review what was masked + decide whether to bypass (Enterprise-tier override only).

### 90-day default policy (per memory note `project-mcp-defaults`)

For the first 90 days of MCP capability availability:

1. **Adversarial test suite** runs in CI on every PR touching `mcp/` — covers sandbox escape attempts, egress DLP bypass attempts, prompt-injection via MCP response.
2. **5-server hand-vetted catalog** — Mizan team curates the initial 5 public servers (likely: Notion / GitHub / Linear / Slack / Filesystem). Each gets a per-server ADR + security review.
3. **Self-registered MCP servers** allowed but flagged with a `'untrusted'` warning badge per ADR 0023's `'mcp'` modifier; users must explicitly acknowledge the warning before each connection.

After 90 days: review + relax-or-tighten per actual usage data.

### Catalog review process

Each public catalog entry requires:
1. **Per-server ADR** documenting the server's claimed capabilities + security review verdict
2. **Annual re-review** + ability to delist within 24h on credible reports
3. **Self-registration** stays available but never gets the "public catalog" badge

### Per-user trust levels

The `mcp_servers.trust_level` enum is **schema-prepared but never wired** in the initial release. The column exists so future "trusted MCP server" tiers (e.g. ones that could mutate non-financial state like reminders or alerts) don't require a migration. Until a security review explicitly authorises wiring `trust_level = 'trusted'` paths, every MCP server is treated as `untrusted` — sandbox bright line applies absolutely.

### Entitlement gating

MCP capability is Gold+ per working-agreement §A11. Gating happens at the gateway endpoint.

### Per-tool registration (per ADR 0020 pattern)

Every MCP-backed tool the agent can call registers via `register_mcp_tool!` with all four AI Safety Runtime properties + an additional `mcp_server_id` linking back to the originating server.

## Rationale

**Why a gateway architecture (not direct desktop-to-MCP)?**
- **Centralised egress DLP** — every MCP call passes the same filter; no chance of a desktop-side bypass
- **Audit trail** — every call is logged with cryptographic digests of params + response
- **Rate limiting** — per-user 60 calls/min is enforced at one chokepoint
- **Telemetry** — usage data informs the catalog review process + the 90-day default review

**Why the sandbox bright line (no MCP writes to financial state)?**
- Spec §21.3 + working-agreement §0 rule 6 + the CLAUDE.md memory note all reinforce: **MCP tools may never mutate financial state**. The bright line is non-negotiable because compromising even one MCP server (or one prompt-injection via MCP response) could otherwise pivot to writing arbitrary values into the user's holdings.
- The compile-time + run-time double check is defence-in-depth.

**Why egress DLP at the gateway (not at the LLM dispatcher)?**
- The dispatcher already handles output sanitisation for built-in tools. MCP responses are arbitrary third-party data; treating them with extra paranoia means the DLP filter at the gateway catches the data BEFORE it ever reaches the LLM context (preventing prompt-injection that tries to exfil sensitive data via the user's question).

**Why the 5-server initial catalog?**
Per memory note `project-mcp-defaults` — the user explicitly accepted "5-server hand-vetted catalog for first 90 days" as the default that closes the MCP pen-test gate. Five is small enough for thorough per-server review; large enough to demonstrate the capability.

**Why annual catalog re-review + 24h delisting?**
Standard CVE response cadence. A discovered vulnerability in a catalog server gets removed from the public catalog within 24h; existing user connections get a warning notification but aren't forcibly disconnected (user choice).

## Consequences

**Positive:**
- Gold+ users get a meaningful capability expansion without compromising the financial-state integrity
- Egress DLP catches the most common privacy leak patterns at the source
- 90-day default policy gives the team data to refine the catalog process

**Negative / accepted:**
- Gateway latency: every MCP call has a desktop → Mizan Connect → MCP server hop. Mitigation: gateway adds < 100ms p99 (working-agreement §A19 budget); measured in PR-K7.
- Catalog review burden: each new public server requires a per-server ADR + security review. Mitigation: queue-based review + the existing AAOIFI / OAuth review cadence covers it.

**Risks:**
- A clever prompt-injection via MCP response could try to escalate sandbox scope. Mitigation: the adversarial test suite (PR-K4) specifically tests this attack class + the dispatcher's prompt template includes anti-prompt-injection guardrails per Spec §16.3.
- Public catalog reputation risk if a curated server gets compromised. Mitigation: 24h delisting + per-server ADR + audit log entries surface unusual behaviour patterns.

## Alternatives considered

- **Allow MCP write to financial state with extra confirmation** — REJECTED. Spec §21.3 + working-agreement §0 rule 6 absolute bright line.
- **No gateway — direct desktop-to-MCP** — rejected per §"Why a gateway" above.
- **No public catalog — only self-registered servers** — rejected; curated catalog is the trust on-ramp for users who don't know which MCP servers to add.

## Implementation map

| PR | What lands |
|---|---|
| PR-K1 | Schema (mcp_servers / mcp_call_log / mcp_catalog / scratchpad) — foundation already shipped (#35) |
| PR-K2 | Gateway skeleton + endpoint set |
| PR-K3 | Dispatcher integration — MCP-namespaced tools routed through `mcp_gate` |
| PR-K4 | Adversarial test suite (sandbox-escape + DLP-bypass + prompt-injection attempts) |
| PR-K4.b | The absolute read-mostly gate + penetration tests |
| PR-K5 | Scratchpad namespace + UI surface ("Notes from connected tools") |
| PR-K6 | Egress DLP filter + fixture patterns |
| PR-K7 | Rate limiting + per-server timeouts |
| PR-K8 | `mcp_call_log` audit table + digest computation |
| PR-K9 | Public catalog UI in Settings → AI → Connected Tools |
| PR-K10 | Catalog review process + delisting workflow |
| PR-K11 | Self-registration flow + warning badge + acknowledgment |
| PR-K12 | Per-call user confirmation UI for outbound financial data |
| PR-K13 | ToS Gold+ clause + privacy policy update |
| PR-K14 | CP-K-sandbox penetration testing — adversarial servers, prompt-injection attempts |

Each PR ≤ 500 lines per working-agreement §A21.

## Notes

The MCP pen-test gate from the original three-gate plan was removed per memory note `project-mcp-defaults` (2026-06-03) — the adversarial test suite (PR-K4) + 5-server hand-vetted catalog (PR-K10) discharge that gate's intent.
