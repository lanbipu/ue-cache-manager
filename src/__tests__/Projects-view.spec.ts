import { mount, flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listProjects: vi.fn(),
    listProjectLocations: vi.fn(),
    listMachines: vi.fn(),
    listCredentials: vi.fn(),
    discoverProjects: vi.fn(),
    createProjectManual: vi.fn(),
    setProjectLocation: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import Projects from "@/views/Projects.vue";

describe("Projects view", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    mockApi.listMachines.mockResolvedValue([
      { id: 1, hostname: "RENDER-01", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ]);
    mockApi.listCredentials.mockResolvedValue([]);
  });

  it("shows empty state", async () => {
    mockApi.listProjects.mockResolvedValue([]);
    const wrapper = mount(Projects);
    await flushPromises();
    expect(wrapper.find("[data-projects-empty]").exists()).toBe(true);
  });

  it("renders project rows and expands locations", async () => {
    mockApi.listProjects.mockResolvedValue([
      { id: 1, uproject_name: "Demo.uproject", display_name: null, uproject_guid: null, location_count: 1 },
    ]);
    mockApi.listProjectLocations.mockResolvedValue([
      { id: 2, project_id: 1, machine_id: 1, abs_path: "D:\\Demo", uproject_path: "D:\\Demo\\Demo.uproject", discovery_status: "auto", discovered_at: null },
    ]);
    const wrapper = mount(Projects);
    await flushPromises();
    expect(wrapper.findAll("[data-project-row]").length).toBe(1);
    await wrapper.find("[data-project-row] button").trigger("click");
    await flushPromises();
    expect(wrapper.find("[data-project-location-row]").exists()).toBe(true);
    expect(wrapper.text()).toContain("RENDER-01");
  });
});
