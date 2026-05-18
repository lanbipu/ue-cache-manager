import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import HealthCheck from "@/views/HealthCheck.vue";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(async () => []),
    getGpuConsistencyMatrix: vi.fn(async () => ({ signatures: [], baseline: null, cells: [] })),
    testPowerShellBridge: vi.fn(),
  },
}));

vi.mock("@/services/tauri", async () => {
  const actual = await vi.importActual<typeof import("@/services/tauri")>("@/services/tauri");
  return {
    ...actual,
    tauriApi: mockApi,
  };
});

describe("HealthCheck view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.testPowerShellBridge.mockReset();
  });

  it("renders full check action", () => {
    const wrapper = mount(HealthCheck);
    expect(wrapper.find("[data-open-health-wizard-btn]").exists()).toBe(true);
  });

  it("clicking the bridge test button calls the backend and shows result", async () => {
    mockApi.testPowerShellBridge.mockResolvedValue({
      received: "hello",
      timestamp: "2026-05-01T12:00:00",
      machine: "TESTPC",
    });
    const wrapper = mount(HealthCheck);

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(mockApi.testPowerShellBridge).toHaveBeenCalledWith("hello from UECM");
    const result = wrapper.find("[data-bridge-result]");
    expect(result.exists()).toBe(true);
    expect(result.text()).toContain("TESTPC");
    expect(result.text()).toContain("hello");
  });

  it("shows error when bridge call fails", async () => {
    mockApi.testPowerShellBridge.mockRejectedValue({
      code: "POWERSHELL",
      message: "not on windows",
    });
    const wrapper = mount(HealthCheck);

    await wrapper.find("[data-bridge-test-btn]").trigger("click");
    await flushPromises();

    expect(wrapper.find("[data-bridge-result]").text()).toContain("not on windows");
  });

  it("renders L1 / L2 / L3 layer sections when a scan run is loaded", async () => {
    const wrapper = mount(HealthCheck);
    const { useHealthCheckStore } = await import("@/stores/healthCheck");
    const { useMachinesStore } = await import("@/stores/machines");
    const store = useHealthCheckStore();
    const machines = useMachinesStore();
    machines.machines = [{ id: 7, hostname: "RENDER-A", ip: "192.168.10.21", status: "online" } as any];
    store.scanRunId = 42 as any;
    store.results = [{
      scan_run_id: 42,
      machine_id: 7,
      machine_results: {
        tcp_5985:        { status: "healthy",  message: "open",       sample: "open",    remediation: "" },
        firewall_445:    { status: "critical", message: "rule off",   sample: "false",
                           remediation: "Run uecm-cli winrm bootstrap <host>" },
        share_reachable: { status: "warning",  message: "slow",       sample: "",        remediation: "" },
      },
    }] as any;
    await flushPromises();

    const text = wrapper.text();
    // Layer labels render — match either i18n key string OR L1/L2/L3 prefix.
    expect(text).toMatch(/L1|端口/);
    expect(text).toMatch(/L2|Bootstrap/);
    expect(text).toMatch(/L3|业务|Business/);
    // Probe keys render
    expect(text).toContain("tcp_5985");
    expect(text).toContain("firewall_445");
    expect(text).toContain("share_reachable");
  });

  it("renders remediation text under a critical row", async () => {
    const wrapper = mount(HealthCheck);
    const { useHealthCheckStore } = await import("@/stores/healthCheck");
    const { useMachinesStore } = await import("@/stores/machines");
    const store = useHealthCheckStore();
    const machines = useMachinesStore();
    machines.machines = [{ id: 1, hostname: "RENDER-B", ip: "192.168.10.22", status: "online" } as any];
    store.scanRunId = 1 as any;
    store.results = [{
      scan_run_id: 1,
      machine_id: 1,
      machine_results: {
        cred_user: { status: "critical", message: "missing", sample: "",
                     remediation: "Run uecm-cli share inject-system-cred --host <host>" },
      },
    }] as any;
    await flushPromises();

    expect(wrapper.text()).toContain("share inject-system-cred");
  });
});
