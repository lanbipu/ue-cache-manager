import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectDiscoveryWizard from "@/components/modals/ProjectDiscoveryWizard.vue";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    discoverProjects: vi.fn(),
    listProjects: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

describe("ProjectDiscoveryWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockApi.discoverProjects.mockReset();
    mockApi.listProjects.mockReset();
  });

  it("renders discovery form", () => {
    const wrapper = mount(ProjectDiscoveryWizard, {
      props: {
        open: true,
        machines: [{ id: 1, hostname: "RENDER-01", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null }],
        credentials: [],
      },
    });
    expect(wrapper.find("[data-project-discovery-wizard]").exists()).toBe(true);
  });

  it("runs discovery and closes", async () => {
    mockApi.discoverProjects.mockResolvedValue([]);
    mockApi.listProjects.mockResolvedValue([]);
    const wrapper = mount(ProjectDiscoveryWizard, {
      props: {
        open: true,
        machines: [{ id: 1, hostname: "RENDER-01", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null }],
        credentials: [],
      },
    });
    await wrapper.find("select").setValue("1");
    await wrapper.find("[data-run-discovery]").trigger("click");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mockApi.discoverProjects).toHaveBeenCalled();
    expect(wrapper.emitted("close")).toBeTruthy();
  });
});
