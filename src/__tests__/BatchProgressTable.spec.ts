import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BatchProgressTable from "@/components/batch/BatchProgressTable.vue";
import type { BatchEvent, Machine } from "@/services/tauri";

function machine(id: number, hostname: string, ip = "1.1.1.1"): Machine {
  return { id, hostname, ip, role: "render", status: "online", last_seen_at: null };
}

describe("BatchProgressTable", () => {
  it("renders a row per machine", () => {
    const wrapper = mount(BatchProgressTable, {
      props: {
        machines: [machine(1, "A"), machine(2, "B"), machine(3, "C")],
        byMachine: new Map<number, BatchEvent>(),
      },
    });
    expect(wrapper.findAll("[data-batch-row]").length).toBe(3);
  });

  it("shows ✓ for ok events", () => {
    const map = new Map<number, BatchEvent>();
    map.set(1, { machine_id: 1, status: "ok", message: null });
    const wrapper = mount(BatchProgressTable, {
      props: { machines: [machine(1, "A")], byMachine: map },
    });
    expect(wrapper.text()).toContain("✓");
  });

  it("shows ✗ + message for err events", () => {
    const map = new Map<number, BatchEvent>();
    map.set(1, { machine_id: 1, status: "err", message: "timeout" });
    const wrapper = mount(BatchProgressTable, {
      props: { machines: [machine(1, "A")], byMachine: map },
    });
    expect(wrapper.text()).toContain("✗");
    expect(wrapper.text()).toContain("timeout");
  });

  it("shows ↻ for running events", () => {
    const map = new Map<number, BatchEvent>();
    map.set(1, { machine_id: 1, status: "running", message: null });
    const wrapper = mount(BatchProgressTable, {
      props: { machines: [machine(1, "A")], byMachine: map },
    });
    expect(wrapper.text()).toContain("↻");
  });

  it("shows — when no event yet", () => {
    const wrapper = mount(BatchProgressTable, {
      props: { machines: [machine(1, "A")], byMachine: new Map() },
    });
    expect(wrapper.text()).toContain("—");
  });
});
