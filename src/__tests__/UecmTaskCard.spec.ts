import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import UecmTaskCard from "@/components/primitives/UecmTaskCard.vue";

describe("UecmTaskCard", () => {
  it("renders title and running progress", () => {
    const wrapper = mount(UecmTaskCard, {
      props: { title: "Generate", status: "running", progress: 0.5 },
    });
    expect(wrapper.find("[data-task-card]").exists()).toBe(true);
    expect(wrapper.text()).toContain("Generate");
    expect(wrapper.text()).toContain("50%");
  });

  it("emits cancel", async () => {
    const wrapper = mount(UecmTaskCard, {
      props: { title: "Generate", status: "running", cancellable: true },
    });
    await wrapper.find("[data-task-cancel]").trigger("click");
    expect(wrapper.emitted("cancel")).toHaveLength(1);
  });
});
