import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import { routes } from "@/router";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    addMachine: vi.fn(),
    deleteMachine: vi.fn(),
    getMachineDetail: vi.fn(),
    refreshMachine: vi.fn(),
    bootstrapWinrm: vi.fn(),
    getWinrmBootstrapScript: vi.fn(),
    listCredentials: vi.fn(),
    scanNetwork: vi.fn(),
    addDiscoveredMachine: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({
  tauriApi: mockApi,
}));

import Machines from "@/views/Machines.vue";

async function mountWithRouter(initialPath = "/machines"): Promise<{ wrapper: ReturnType<typeof mount>; router: Router }> {
  const router = createRouter({ history: createMemoryHistory(), routes });
  await router.push(initialPath);
  await router.isReady();
  const wrapper = mount(Machines, { global: { plugins: [router] } });
  return { wrapper, router };
}

describe("Machines view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((m: any) => m.mockReset());
    mockApi.listCredentials.mockResolvedValue([]);
  });

  it("shows empty state when no machines", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    const { wrapper } = await mountWithRouter();
    await flushPromises();
    expect(wrapper.text()).toContain("No machines");
  });

  it("renders a card for each machine", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "HOST-NAS", ip: "192.168.10.2", role: "host", status: "online", last_seen_at: null },
    ]);
    const { wrapper } = await mountWithRouter();
    await flushPromises();
    const cards = wrapper.findAll("[data-machine-card]");
    expect(cards).toHaveLength(2);
    expect(wrapper.text()).toContain("RENDER-01");
  });

  it("each machine card exposes a button trigger", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 5, hostname: "RENDER-05", ip: "192.168.10.25", role: "render", status: "online", last_seen_at: null },
    ]);
    const { wrapper } = await mountWithRouter();
    await flushPromises();
    const card = wrapper.find("[data-machine-card] button");
    expect(card.exists()).toBe(true);
    expect(card.attributes("aria-label")).toBe("RENDER-05");
    await card.trigger("click");
    await flushPromises();
  });

  it("clicking a card pushes host + tab to route query", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 7, hostname: "PC-07", ip: "192.168.10.7", role: "render", status: "online", last_seen_at: null },
    ]);
    const { wrapper, router } = await mountWithRouter();
    await flushPromises();
    await wrapper.find("[data-machine-card] button").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.query.host).toBe("7");
    expect(router.currentRoute.value.query.tab).toBe("overview");
  });

  it("clicking the already-expanded card clears the query", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 7, hostname: "PC-07", ip: "192.168.10.7", role: "render", status: "online", last_seen_at: null },
    ]);
    const { wrapper, router } = await mountWithRouter("/machines?host=7&tab=overview");
    await flushPromises();
    await wrapper.find("[data-machine-card] button").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.query.host).toBeUndefined();
    expect(router.currentRoute.value.query.tab).toBeUndefined();
  });

  it("clicking Scan button reveals discovery wizard", async () => {
    mockApi.listMachines.mockResolvedValue([]);
    const { wrapper } = await mountWithRouter();
    await flushPromises();
    await wrapper.find("[data-discover-btn]").trigger("click");
    // DiscoveryWizard renders via Teleport; in tests Teleport is stubbed
    // so the modal renders inside the wrapper rather than on document.body.
    expect(wrapper.html()).toContain("data-modal");
  });

  it("renders the cluster summary bar with online/critical/warning counts", async () => {
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "A", ip: "1", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "B", ip: "2", role: "render", status: "critical", last_seen_at: null },
      { id: 3, hostname: "C", ip: "3", role: "render", status: "warning", last_seen_at: null },
    ]);
    const { wrapper } = await mountWithRouter();
    await flushPromises();
    const summary = wrapper.find("[data-cluster-summary]");
    expect(summary.exists()).toBe(true);
    expect(summary.text()).toContain("1");
    expect(summary.text()).toContain("3");
  });
});
