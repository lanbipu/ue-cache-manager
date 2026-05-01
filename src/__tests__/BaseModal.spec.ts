import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import BaseModal from "@/components/modals/BaseModal.vue";

describe("BaseModal", () => {
  it("renders title slot content", () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "Test Title" },
    });
    expect(wrapper.text()).toContain("Test Title");
  });

  it("renders default slot content", () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
      slots: { default: "<p>body content</p>" },
    });
    expect(wrapper.html()).toContain("body content");
  });

  it("does not render anything when open=false", () => {
    const wrapper = mount(BaseModal, {
      props: { open: false, title: "T" },
    });
    expect(wrapper.find("[data-modal]").exists()).toBe(false);
  });

  it("emits close on backdrop click", async () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
    });
    await wrapper.find("[data-modal-backdrop]").trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("emits close on close-button click", async () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
    });
    await wrapper.find("[data-modal-close]").trigger("click");
    expect(wrapper.emitted("close")).toBeTruthy();
  });

  it("does NOT emit close when clicking inside the panel", async () => {
    const wrapper = mount(BaseModal, {
      props: { open: true, title: "T" },
      slots: { default: '<div data-test-inner>inner</div>' },
    });
    await wrapper.find("[data-test-inner]").trigger("click");
    expect(wrapper.emitted("close")).toBeFalsy();
  });
});
