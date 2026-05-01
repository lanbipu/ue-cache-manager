import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import { useDiscoveryStore } from "@/stores/discovery";

describe("discovery store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.scanNetwork.mockReset();
    mockApi.addDiscoveredMachine.mockReset();
  });

  it("starts with default CIDR and empty probed list", () => {
    const store = useDiscoveryStore();
    expect(store.cidr).toBe("192.168.10.0/24");
    expect(store.probed).toEqual([]);
    expect(store.isScanning).toBe(false);
  });

  it("scan populates probed list", async () => {
    mockApi.scanNetwork.mockResolvedValue({
      probed: [
        { ip: "192.168.10.21", winrm_open: true, smb_open: true },
      ],
    });
    const store = useDiscoveryStore();
    await store.scan();
    expect(store.probed).toHaveLength(1);
    expect(store.probed[0].ip).toBe("192.168.10.21");
  });

  it("scan toggles isScanning", async () => {
    mockApi.scanNetwork.mockImplementation(
      () => new Promise((r) => setTimeout(() => r({ probed: [] }), 10)),
    );
    const store = useDiscoveryStore();
    const p = store.scan();
    expect(store.isScanning).toBe(true);
    await p;
    expect(store.isScanning).toBe(false);
  });

  it("scan with explicit input updates CIDR", async () => {
    mockApi.scanNetwork.mockResolvedValue({ probed: [] });
    const store = useDiscoveryStore();
    await store.scan("10.0.0.0/24");
    expect(store.cidr).toBe("10.0.0.0/24");
  });

  it("addToInventory delegates to api", async () => {
    mockApi.addDiscoveredMachine.mockResolvedValue(7);
    const store = useDiscoveryStore();
    const id = await store.addToInventory("192.168.10.99", null);
    expect(mockApi.addDiscoveredMachine).toHaveBeenCalledWith("192.168.10.99", null);
    expect(id).toBe(7);
  });

  it("captures errors during scan", async () => {
    mockApi.scanNetwork.mockRejectedValue({ code: "INVALID_INPUT", message: "bad cidr" });
    const store = useDiscoveryStore();
    await store.scan();
    expect(store.error).toEqual({ code: "INVALID_INPUT", message: "bad cidr" });
  });
});
