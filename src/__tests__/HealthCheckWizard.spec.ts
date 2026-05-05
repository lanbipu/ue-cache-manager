import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(async () => [{ id: 11, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }]),
    listCredentials: vi.fn(async () => [{ alias: "UECM:winrm:LANPC", username: "lanpc", kind: "winrm" }]),
    runHealthCheck: vi.fn(async () => ({ scan_run_id: 9, healthy: 0, warning: 0, critical: 0, offline: 0, total: 0 })),
    listHealthResultsForRun: vi.fn(async () => []),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import HealthCheckWizard from "@/components/modals/HealthCheckWizard.vue";

describe("HealthCheckWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset?.());
    mockApi.listMachines.mockResolvedValue([{ id: 11, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }]);
    mockApi.listCredentials.mockResolvedValue([{ alias: "UECM:winrm:LANPC", username: "lanpc", kind: "winrm" }]);
  });

  it("renders title", async () => {
    const w = mount(HealthCheckWizard, { props: { open: true } });
    await new Promise(r => setTimeout(r, 0));
    expect(w.text()).toContain("Run health check");
  });
});
