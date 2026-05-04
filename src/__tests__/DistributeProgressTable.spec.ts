import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import DistributeProgressTable from "@/components/ddcpak/DistributeProgressTable.vue";

describe("DistributeProgressTable", () => {
  it("renders target rows and emits retry", async () => {
    const wrapper = mount(DistributeProgressTable, {
      props: {
        job: {
          job_id: "dist-1",
          project_id: 1,
          source_machine_id: 2,
          status: "completed",
          started_at: "now",
          finished_at: null,
          targets: [{ target_machine_id: 3, target_host: "3.3.3.3", status: "err", message: "failed" }],
        },
      },
    });
    expect(wrapper.find("[data-distribute-progress-table]").exists()).toBe(true);
    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("retry")?.[0]).toEqual([3]);
  });
});
