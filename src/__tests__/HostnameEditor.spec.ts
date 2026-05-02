import { describe, it, expect } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import HostnameEditor from "@/components/machines/HostnameEditor.vue";

describe("HostnameEditor", () => {
  it("renders display span by default with value", () => {
    const wrapper = mount(HostnameEditor, { props: { value: "RENDER-01" } });
    const display = wrapper.find("[data-hostname-display]");
    expect(display.exists()).toBe(true);
    expect(display.text()).toBe("RENDER-01");
    expect(wrapper.find("[data-hostname-input]").exists()).toBe(false);
  });

  it("clicking display switches to input", async () => {
    const wrapper = mount(HostnameEditor, { props: { value: "RENDER-01" } });
    await wrapper.find("[data-hostname-display]").trigger("click");
    await flushPromises();
    expect(wrapper.find("[data-hostname-display]").exists()).toBe(false);
    const input = wrapper.find("[data-hostname-input]");
    expect(input.exists()).toBe(true);
    expect((input.element as HTMLInputElement).value).toBe("RENDER-01");
  });

  it("pressing Enter emits save with trimmed new value", async () => {
    const wrapper = mount(HostnameEditor, { props: { value: "RENDER-01" } });
    await wrapper.find("[data-hostname-display]").trigger("click");
    await flushPromises();
    const input = wrapper.find("[data-hostname-input]");
    await input.setValue("  RENDER-02  ");
    await input.trigger("keyup.enter");
    const events = wrapper.emitted("save");
    expect(events).toBeTruthy();
    expect(events![0]).toEqual(["RENDER-02"]);
  });

  it("pressing Escape cancels without emitting save", async () => {
    const wrapper = mount(HostnameEditor, { props: { value: "RENDER-01" } });
    await wrapper.find("[data-hostname-display]").trigger("click");
    await flushPromises();
    const input = wrapper.find("[data-hostname-input]");
    await input.setValue("RENDER-99");
    await input.trigger("keyup.escape");
    expect(wrapper.emitted("save")).toBeFalsy();
    expect(wrapper.find("[data-hostname-display]").exists()).toBe(true);
  });

  it("blur commits unchanged value without emitting (no-op)", async () => {
    const wrapper = mount(HostnameEditor, { props: { value: "RENDER-01" } });
    await wrapper.find("[data-hostname-display]").trigger("click");
    await flushPromises();
    const input = wrapper.find("[data-hostname-input]");
    // value remains "RENDER-01" (unchanged)
    await input.trigger("blur");
    expect(wrapper.emitted("save")).toBeFalsy();
    // Returns to display mode
    expect(wrapper.find("[data-hostname-display]").exists()).toBe(true);
  });
});
