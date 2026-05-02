import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listShares: vi.fn(),
    createShare: vi.fn(),
    injectShareCredentialToClients: vi.fn(),
    deleteShare: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import { useSharesStore } from "@/stores/shares";

describe("shares store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m) => m.mockReset());
  });

  it("starts empty", () => {
    expect(useSharesStore().shares).toEqual([]);
  });

  it("load populates list", async () => {
    mockApi.listShares.mockResolvedValue([
      {
        id: 1,
        host_machine_id: 1,
        share_name: "DDC",
        unc_path: "\\\\H\\DDC",
        local_path: "D:\\DDC",
        mode: "open",
        credential_alias: null,
      },
    ]);
    const store = useSharesStore();
    await store.load();
    expect(store.shares).toHaveLength(1);
  });

  it("create dispatches with correct args and reloads", async () => {
    mockApi.createShare.mockResolvedValue({
      share_config_id: 1,
      unc_path: "\\\\H\\DDC",
      mode: "managed",
      credential_alias: "UECM:share:H:ddc-svc",
    });
    mockApi.listShares.mockResolvedValue([]);
    const store = useSharesStore();
    await store.create(1, "managed", "DDC", "D:\\DDC", "op", "ddc-svc");
    expect(mockApi.createShare).toHaveBeenCalledWith(
      1,
      "managed",
      "DDC",
      "D:\\DDC",
      "op",
      "ddc-svc",
    );
    expect(mockApi.listShares).toHaveBeenCalled();
  });

  it("inject returns results without reloading", async () => {
    mockApi.injectShareCredentialToClients.mockResolvedValue([
      { client_machine_id: 2, ok: true, message: "ok" },
    ]);
    const store = useSharesStore();
    const results = await store.inject(1, [2], null);
    expect(results).toHaveLength(1);
    expect(mockApi.listShares).not.toHaveBeenCalled();
  });

  it("remove calls api then reloads", async () => {
    mockApi.deleteShare.mockResolvedValue(undefined);
    mockApi.listShares.mockResolvedValue([]);
    const store = useSharesStore();
    await store.remove(1, true);
    expect(mockApi.deleteShare).toHaveBeenCalledWith(1, true);
    expect(store.shares).toEqual([]);
  });
});
