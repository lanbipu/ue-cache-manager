import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import PakJobCard from "@/components/ddcpak/PakJobCard.vue";

describe("PakJobCard", () => {
  it("renders job and emits cancel", async () => {
    const wrapper = mount(PakJobCard, {
      props: {
        job: {
          job_id: "gen-1",
          source_machine_id: 1,
          project_id: 2,
          backend: "remote",
          status: "running",
          log_lines: ["line"],
          progress_pct: null,
          progress_label: null,
          exit_code: null,
          error_message: null,
          output: null,
          pending_distribute: null,
          started_at: "now",
          finished_at: null,
        },
      },
    });
    await wrapper.find("[data-task-cancel]").trigger("click");
    expect(wrapper.emitted("cancel")?.[0]).toEqual(["gen-1"]);
  });
});
