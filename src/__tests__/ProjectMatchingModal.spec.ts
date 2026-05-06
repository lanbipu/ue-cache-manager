import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProjectMatchingModal from "@/components/modals/ProjectMatchingModal.vue";

const { mockApi } = vi.hoisted(() => ({
  mockApi: {
    createProjectManual: vi.fn(),
    setProjectLocation: vi.fn(),
    listProjects: vi.fn(),
    listProjectLocations: vi.fn(),
  },
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));

describe("ProjectMatchingModal", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
  });

  it("renders mapping form", () => {
    const wrapper = mount(ProjectMatchingModal, {
      props: { open: true, projects: [], machines: [] },
    });
    expect(wrapper.find("[data-project-matching-modal]").exists()).toBe(true);
  });
});
