import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import PsoFileExplorer from "@/components/pso/PsoFileExplorer.vue";
import type { PsoCacheFile } from "@/services/tauri";

const file: PsoCacheFile = {
  id: 1,
  project_id: 10,
  source_machine_id: 2,
  file_path: "D:\\Proj\\Saved\\CollectedPSOs\\x.upipelinecache",
  file_name: "x.upipelinecache",
  size_bytes: 2048,
  gpu_signature: "nvidia:RTX 4090:551.86",
  ue_version: "5.4",
  collected_at: null,
};

describe("PsoFileExplorer", () => {
  it("renders empty state", () => {
    const wrapper = mount(PsoFileExplorer, {
      props: { files: [], machineLabel: (id: number) => `m#${id}` },
    });
    expect(wrapper.find("[data-pso-file-empty]").exists()).toBe(true);
  });

  it("emits selected file for distribution", async () => {
    const wrapper = mount(PsoFileExplorer, {
      props: { files: [file], machineLabel: () => "RENDER-02" },
    });
    expect(wrapper.find("[data-pso-file-row]").text()).toContain("x.upipelinecache");
    await wrapper.find("[data-pso-file-distribute-btn]").trigger("click");
    expect(wrapper.emitted("distribute")?.[0]?.[0]).toMatchObject({ id: 1 });
  });
});
