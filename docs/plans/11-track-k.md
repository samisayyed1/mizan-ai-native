# Track K — MCP Capability

**Status:** Pending. Last in the execution order — composes on top of every other surface.
**Estimated sprints:** 4.
**Depends on:** Tracks C (memory + tool registry), G (Gold+ entitlement).
**Source:** `docs/plans/00-master-plan.md` → "Track K — MCP Capability".

## Scope

**In:** per-user MCP gateway in Mizan Connect, `mcp_servers` registry + `mcp_call_log` audit table, dispatcher integration with read-mostly gate, `scratchpad` namespace + UI surface, public catalog with security review process, egress DLP rules for sensitive identifier patterns, Gold+ entitlement gating, `trust_level` enum schema prep.

**Out:** building actual MCP servers (consumption-only); Sharia screen on MCP servers themselves (out of scope).

## PRs

| # | Status | Title |
|---|---|---|
| K1.a | ✅ Done (cloud) | Schema (mcp_catalog, mcp_servers, mcp_call_log) — cloud | `mizan-connect/migrations/0013_mcp_capability.sql` — catalog + per-user registry + audit log with digests-only retention. `trust_level` enum column prep per ADR 0048 (NEVER honoured by code today). |
| K1.b | ⏸️ Pending | Schema (scratchpad) — desktop | The sandboxed per-user K/V store rendered as "Notes from connected tools" |
| K2 | ⏸️ Pending | Gateway skeleton in Mizan Connect + endpoint set (`POST /v1/mcp/server`, `GET /v1/mcp/servers`, `DELETE /v1/mcp/server/:id`) |
| K3 | ⏸️ Pending | Dispatcher integration — MCP-namespaced tools routed through `mcp_gate` |
| K4 | ⏸️ Pending | **The absolute read-mostly gate** + penetration tests (MCP tools rejected on any mutation to `truth_ledger` / `holdings` / `activities` / `balances`) |
| K5 | ⏸️ Pending | Scratchpad namespace + UI surface ("Notes from connected tools") |
| K6 | ⏸️ Pending | Egress DLP filter + fixture patterns (SSN, PAN, Aadhaar, card numbers) |
| K7 | ⏸️ Pending | Rate limiting (60 calls/min per user) + per-server timeouts (10s default, 30s max) |
| K8 | ⏸️ Pending | `mcp_call_log` audit table + digest computation (no raw payload retention) |
| K9 | ⏸️ Pending | Public catalog UI in Settings → AI → Connected Tools |
| K10 | ⏸️ Pending | Catalog review process + delisting workflow (24h SLA on credible reports) |
| K11 | ⏸️ Pending | Self-registration flow + warning badge + user acknowledgment |
| K12 | ⏸️ Pending | Per-call user confirmation UI for outbound financial data |
| K13 | ⏸️ Pending | ToS Gold+ clause + privacy policy update for MCP user responsibility |
| K14 | ⏸️ Pending | **CP-K-sandbox** penetration testing — adversarial servers + prompt-injection attempts; signed report; zero blockers |

## ADRs (planned)

- 0044 — MCP capability architecture
- 0045 — MCP sandbox gate (absolute) — the read-mostly rule is non-negotiable per working agreement §3.4
- 0046 — MCP egress DLP rules
- 0047 — MCP public catalog review process
- 0048 — MCP `trust_level` schema prep (column ready, never honoured)

## Security review checklist (most stringent in any track)

- [ ] `mcp_gate` rejects any mutation to financial tables — penetration tested
- [ ] Egress DLP rejects sensitive identifier patterns — fixture-tested
- [ ] Outbound MCP uses dedicated egress proxy with separate rate limits
- [ ] Every call logged with digest (no raw payload retention)
- [ ] User per-call confirmation for outbound financial data
- [ ] Public catalog entries reviewed at registration + annually
- [ ] Self-registered servers carry warning badge + require user-acknowledged confirmation
- [ ] Suspected misbehaving servers can be delisted within 24h
- [ ] ToS Gold+ clause states user responsibility for self-registered MCPs

## Definition of Done

- Gold user registers a Notion MCP, agent calls Notion read tools, results land in scratchpad with `'mcp'` badge
- Test server attempting `update_holding` rejected at the gate (test passes)
- Payload containing `xxx-xx-xxxx` SSN pattern rejected by egress DLP
- Catalog has 3+ vetted entries (recommend Notion / GitHub / Linear — all low-risk read-only by design)
- ToS + privacy policy updated and live
- Penetration test report signed off — zero blocker findings
