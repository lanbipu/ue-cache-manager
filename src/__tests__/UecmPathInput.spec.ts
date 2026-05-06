import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import UecmPathInput from "@/components/primitives/UecmPathInput.vue";

describe("UecmPathInput", () => {
  it("updates model value", async () => {
    const wrapper = mount(UecmPathInput, { props: { modelValue: "" } });
    await wrapper.find("input").setValue("D:\\Work");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["D:\\Work"]);
  });

  it("emits validate", async () => {
    const wrapper = mount(UecmPathInput, { props: { modelValue: "D:\\Work" } });
    await wrapper.find("[data-path-validate]").trigger("click");
    expect(wrapper.emitted("validate")).toHaveLength(1);
  });
});
