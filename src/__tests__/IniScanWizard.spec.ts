import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(async () => [{ id: 11, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }]),
    listCredentials: vi.fn(async () => [{ alias: "UECM:winrm:LANPC", username: "lanpc", kind: "winrm" }]),
    scanInis: vi.fn(async () => ({ scan_run_id: 7, critical: 0, warning: 0, healthy: 0 })),
    listFindingsForRun: vi.fn(async () => []),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import IniScanWizard from "@/components/modals/IniScanWizard.vue";

describe("IniScanWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset?.());
    mockApi.listMachines.mockResolvedValue([{ id: 11, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }]);
    mockApi.listCredentials.mockResolvedValue([{ alias: "UECM:winrm:LANPC", username: "lanpc", kind: "winrm" }]);
  });

  it("renders title", async () => {
    const w = mount(IniScanWizard, { props: { open: true } });
    await new Promise(r => setTimeout(r, 0));
    expect(w.text()).toContain("Run INI scan");
  });
});
