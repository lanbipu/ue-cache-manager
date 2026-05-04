import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmGpuMatrix from "@/components/primitives/UecmGpuMatrix.vue";

describe("UecmGpuMatrix", () => {
  it("renders empty state when there are no signatures", () => {
    const wrapper = mount(UecmGpuMatrix, {
      props: { matrix: { signatures: [], baseline: null, cells: [] } },
    });
    expect(wrapper.find("[data-gpu-matrix-empty]").exists()).toBe(true);
  });

  it("highlights baseline row and marks matching machines", () => {
    const wrapper = mount(UecmGpuMatrix, {
      props: {
        matrix: {
          signatures: [
            { signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" }, count: 2 },
          ],
          baseline: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
          cells: [
            {
              machine_id: 1,
              hostname: "A",
              signature: { vendor: "nvidia", model: "RTX 3080", driver: "535.98" },
              status: "match",
            },
          ],
        },
      },
    });
    const row = wrapper.find("[data-gpu-matrix-row]");
    expect(row.classes()).toContain("bg-status-healthy/10");
    expect(wrapper.find("[data-gpu-matrix-cell]").text()).toBe("OK");
  });
});
