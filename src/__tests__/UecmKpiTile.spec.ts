import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";

describe("UecmKpiTile", () => {
  it("renders value", () => {
    const w = mount(UecmKpiTile, { props: { label: "Healthy", value: 42 } });
    expect(w.text()).toContain("42");
    expect(w.text()).toContain("HEALTHY");
  });
  it("applies tone class", () => {
    const w = mount(UecmKpiTile, { props: { label: "X", value: 1, tone: "critical" } });
    expect(w.html()).toContain("text-status-critical");
  });
});
