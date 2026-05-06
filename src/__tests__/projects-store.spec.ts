import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    listProjects: vi.fn(),
    listProjectLocations: vi.fn(),
    discoverProjects: vi.fn(),
    setProjectLocation: vi.fn(),
    deleteProject: vi.fn(),
    deleteProjectLocation: vi.fn(),
    createProjectManual: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

import { useProjectsStore } from "@/stores/projects";

describe("projects store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
  });

  it("loads project summaries", async () => {
    mockApi.listProjects.mockResolvedValue([
      { id: 1, uproject_name: "Demo.uproject", display_name: null, uproject_guid: null, location_count: 2 },
    ]);
    const store = useProjectsStore();
    await store.load();
    expect(store.projects).toHaveLength(1);
    expect(store.projectsById.get(1)?.uproject_name).toBe("Demo.uproject");
  });

  it("loads locations by project", async () => {
    mockApi.listProjectLocations.mockResolvedValue([
      { id: 7, project_id: 1, machine_id: 2, abs_path: "D:\\Demo", uproject_path: "D:\\Demo\\Demo.uproject", discovery_status: "auto", discovered_at: null },
    ]);
    const store = useProjectsStore();
    await store.loadLocations(1);
    expect(store.locations[1]).toHaveLength(1);
  });

  it("discover calls api then reloads projects", async () => {
    mockApi.discoverProjects.mockResolvedValue([
      { project_id: 1, location_id: 2, uproject_filename: "Demo.uproject", abs_path: "D:\\Demo" },
    ]);
    mockApi.listProjects.mockResolvedValue([]);
    const store = useProjectsStore();
    const result = await store.discover(3, ["D:\\Work"], "cred");
    expect(mockApi.discoverProjects).toHaveBeenCalledWith(3, ["D:\\Work"], "cred");
    expect(result).toHaveLength(1);
    expect(mockApi.listProjects).toHaveBeenCalled();
  });
});
