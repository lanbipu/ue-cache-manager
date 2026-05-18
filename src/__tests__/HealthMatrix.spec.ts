import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import HealthMatrix from "@/components/diagnostics/HealthMatrix.vue";

const machines = [{ id: 1, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }];

describe("HealthMatrix", () => {
  it("renders all check columns and emits selection", async () => {
    const wrapper = mount(HealthMatrix, {
      props: {
        machines,
        rowsByMachine: { 1: { smb: { status: "healthy", message: "ok", sample: "", remediation: "" } } },
        selectedMachineId: null,
        selectedCheckId: null,
      },
    });
    // 13 HEALTH_CHECKS entries + 1 machine-label header = 14 columns
    expect(wrapper.findAll("th").length).toBe(14);
    await wrapper.find("[data-matrix-cell]").trigger("click");
    expect(wrapper.emitted("select")?.[0]?.[0]).toMatchObject({ machineId: 1 });
  });
});
