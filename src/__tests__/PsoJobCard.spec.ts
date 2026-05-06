import { describe, it, expect, vi } from "vitest";
import { mount } from "@vue/test-utils";
import PsoJobCard from "@/components/pso/PsoJobCard.vue";
import type { CollectJobState } from "@/stores/pso";

const baseJob: CollectJobState = {
  job_id: "pso-collect-1",
  source_machine_id: 1,
  project_id: 10,
  status: "collecting",
  log_lines: ["LogShaderPipelineCache: collecting"],
  files_collected: null,
  error_message: null,
  started_at: "2026-01-01T00:00:00Z",
  finished_at: null,
};

describe("PsoJobCard", () => {
  it("renders collection state and emits cancel", async () => {
    const wrapper = mount(PsoJobCard, {
      props: { job: baseJob, sourceLabel: "RENDER-01", projectLabel: "Demo.uproject" },
    });
    expect(wrapper.find("[data-pso-job-card]").text()).toContain("Demo.uproject on RENDER-01");
    await wrapper.find("[data-task-cancel]").trigger("click");
    expect(wrapper.emitted("cancel")?.[0]).toEqual(["pso-collect-1"]);
  });

  it("renders collected file count", () => {
    const wrapper = mount(PsoJobCard, {
      props: {
        job: { ...baseJob, status: "completed", files_collected: 2 },
        sourceLabel: "RENDER-01",
        projectLabel: "Demo.uproject",
      },
      global: {
        stubs: { UecmTaskCard: { template: "<div data-task-card><slot /></div>" } },
      },
    });
    expect(wrapper.find("[data-pso-job-files]").text()).toContain("2 file");
  });
});
