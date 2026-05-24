import { describe, expect, it, vi } from "vitest";

import { listBrokerConnections } from "./broker-service";
import type { BrokerConnection } from "../types";

const adapterMocks = vi.hoisted(() => ({
  logger: {
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
    trace: vi.fn(),
  },
  listBrokerConnections: vi.fn(),
  listBrokerAccounts: vi.fn(),
  syncBrokerData: vi.fn(),
  getSyncedAccounts: vi.fn(),
  getPlatforms: vi.fn(),
  getSubscriptionPlans: vi.fn(),
  getSubscriptionPlansPublic: vi.fn(),
  getUserInfo: vi.fn(),
  getBrokerSyncStates: vi.fn(),
  getImportRuns: vi.fn(),
  createBrokerLoginPortal: vi.fn(),
}));

vi.mock("@/adapters", () => adapterMocks);

/**
 * Pins down the contract the frontend service layer expects from the
 * Tauri adapter for `list_broker_connections`. Used as the upper-bound
 * "it isn't the frontend's fault" check during the bare-array
 * deserialization investigation:
 *
 * - Adapter returns a `BrokerConnection[]` array directly (no wrapper).
 * - Service layer passes it through unchanged.
 *
 * If this ever flips to expecting a `{ connections: [...] }` envelope on
 * the JS side too, that's a behavior change worth catching.
 */
describe("listBrokerConnections", () => {
  it("passes through the array returned by the Tauri adapter unchanged", async () => {
    const fixture: BrokerConnection[] = [
      {
        id: "auth-alpaca-1",
        name: "Alpaca Paper",
        status: "connected",
        disabled: false,
        brokerage: {
          id: "brk-alpaca",
          slug: "ALPACA-PAPER",
          name: "Alpaca Paper",
          display_name: "Alpaca Paper",
        },
        updated_at: "2026-05-05T10:00:01Z",
      },
    ];
    adapterMocks.listBrokerConnections.mockResolvedValue(fixture);

    const result = await listBrokerConnections();

    expect(result).toEqual(fixture);
    expect(adapterMocks.listBrokerConnections).toHaveBeenCalledTimes(1);
  });

  it("rethrows adapter errors so React Query can surface them", async () => {
    adapterMocks.listBrokerConnections.mockRejectedValue(
      new Error("Failed to parse connections: invalid type: map"),
    );

    await expect(listBrokerConnections()).rejects.toThrow(/Failed to parse connections/);
  });
});
