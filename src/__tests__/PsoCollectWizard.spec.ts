import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import PsoCollectWizard from "@/components/modals/PsoCollectWizard.vue";

const { mockApi, listenMock } = vi.hoisted(() => ({
  mockApi: {
    listMachines: vi.fn(),
    listProjects: vi.fn(),
    listCredentials: vi.fn(),
    startPsoCollection: vi.fn(),
    listPsoCacheFiles: vi.fn(),
    cancelUeJob: vi.fn(),
  },
  listenMock: vi.fn(async () => () => {}),
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

describe("PsoCollectWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "RENDER-01", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ]);
    mockApi.listProjects.mockResolvedValue([
      { id: 10, uproject_name: "X.uproject", display_name: null, uproject_guid: null, location_count: 1 },
    ]);
    mockApi.listCredentials.mockResolvedValue([]);
    mockApi.startPsoCollection.mockResolvedValue({
      job_id: "pso-1",
      source_machine_id: 1,
      project_id: 10,
    });
  });

  it("renders step 1 when open", async () => {
    const wrapper = mount(PsoCollectWizard, { props: { open: true } });
    await flushPromises();
    expect(wrapper.find("[data-pso-collect-wizard]").exists()).toBe(true);
    expect(wrapper.find("[data-pso-source-select]").exists()).toBe(true);
  });

  it("blocks Next when no source is selected", () => {
    const wrapper = mount(PsoCollectWizard, { props: { open: true } });
    const next = wrapper.find("[data-pso-wizard-next]");
    expect((next.element as HTMLButtonElement).disabled).toBe(true);
  });
});
