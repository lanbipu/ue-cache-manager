import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import UecmProgressBar from "@/components/primitives/UecmProgressBar.vue";

describe("UecmProgressBar", () => {
  it("renders percentage for determinate value", () => {
    const wrapper = mount(UecmProgressBar, { props: { value: 0.42, label: "Saving pak" } });
    expect(wrapper.text()).toContain("Saving pak");
    expect(wrapper.text()).toContain("42%");
  });

  it("renders running label for indeterminate state", () => {
    const wrapper = mount(UecmProgressBar);
    expect(wrapper.text()).toContain("Running");
  });
});
