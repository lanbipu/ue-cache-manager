import { mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DdcPakWizard from "@/components/modals/DdcPakWizard.vue";

const { mockApi, listenMock } = vi.hoisted(() => ({
  mockApi: {
    generateDdcPak: vi.fn(),
    distributeDdcPak: vi.fn(),
    cancelUeJob: vi.fn(),
  },
  listenMock: vi.fn(async () => () => {}),
}));

vi.mock("@/services/tauri", () => ({ tauriApi: mockApi }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

describe("DdcPakWizard", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    listenMock.mockClear();
  });

  it("renders wizard", () => {
    const wrapper = mount(DdcPakWizard, {
      props: { open: true, projects: [], locations: {}, machines: [], credentials: [] },
    });
    expect(wrapper.find("[data-ddc-pak-wizard]").exists()).toBe(true);
  });
});
