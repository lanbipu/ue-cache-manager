import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import HealthMatrix from "@/components/diagnostics/HealthMatrix.vue";

const machines = [{ id: 1, hostname: "RENDER-01", ip: "192.168.10.21", role: "render", status: "online", last_seen_at: null }];
const rows = { 1: { smb: { status: "healthy", message: "ok", sample: "" } } };

describe("HealthMatrix", () => {
  it("renders a header row + 11 columns", () => {
    const w = mount(HealthMatrix, { props: { machines, rowsByMachine: rows, selectedMachineId: null, selectedCheckId: null } });
    expect(w.findAll("th").length).toBeGreaterThanOrEqual(12);
  });
  it("emits select on cell click", async () => {
    const w = mount(HealthMatrix, { props: { machines, rowsByMachine: rows, selectedMachineId: null, selectedCheckId: null } });
    await w.find("[data-matrix-cell]").trigger("click");
    expect(w.emitted("select")?.[0]?.[0]).toMatchObject({ machineId: 1 });
  });
});
