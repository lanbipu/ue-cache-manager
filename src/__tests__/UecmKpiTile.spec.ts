import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";

describe("UecmKpiTile", () => {
  it("renders label and value", () => {
    const wrapper = mount(UecmKpiTile, { props: { label: "Critical", value: 3, tone: "critical" } });
    expect(wrapper.text()).toContain("Critical");
    expect(wrapper.text()).toContain("3");
  });
});
