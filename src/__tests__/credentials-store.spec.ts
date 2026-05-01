import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listCredentials: vi.fn(),
    saveCredential: vi.fn(),
    deleteCredential: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import { useCredentialsStore } from "@/stores/credentials";

describe("credentials store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.listCredentials.mockReset();
    mockApi.saveCredential.mockReset();
    mockApi.deleteCredential.mockReset();
  });

  it("starts empty", () => {
    const store = useCredentialsStore();
    expect(store.credentials).toEqual([]);
  });

  it("load populates list", async () => {
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:H1", kind: "winrm", username: "admin" },
    ]);
    const store = useCredentialsStore();
    await store.load();
    expect(store.credentials).toHaveLength(1);
  });

  it("save calls api then reloads", async () => {
    mockApi.saveCredential.mockResolvedValue(1);
    mockApi.listCredentials.mockResolvedValue([
      { id: 1, alias: "UECM:winrm:H1", kind: "winrm", username: "admin" },
    ]);
    const store = useCredentialsStore();
    await store.save("UECM:winrm:H1", "winrm", "admin", "p");
    expect(mockApi.saveCredential).toHaveBeenCalledWith("UECM:winrm:H1", "winrm", "admin", "p");
    expect(store.credentials).toHaveLength(1);
  });

  it("remove calls api then reloads", async () => {
    mockApi.deleteCredential.mockResolvedValue(undefined);
    mockApi.listCredentials.mockResolvedValue([]);
    const store = useCredentialsStore();
    await store.remove("UECM:winrm:H1");
    expect(mockApi.deleteCredential).toHaveBeenCalledWith("UECM:winrm:H1");
    expect(store.credentials).toEqual([]);
  });
});
