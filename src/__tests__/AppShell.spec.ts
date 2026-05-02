import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { createRouter, createMemoryHistory } from "vue-router";
import { createPinia, setActivePinia } from "pinia";
import { routes } from "@/router";
import AppShell from "@/components/shell/AppShell.vue";

describe("AppShell", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("renders the activity bar with 8 nav items", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes,
    });
    await router.push("/");
    await router.isReady();

    const wrapper = mount(AppShell, {
      global: { plugins: [router] },
    });

    const navItems = wrapper.findAll("[data-nav-item]");
    expect(navItems).toHaveLength(8);
  });

  it("renders the current route's component in the slot", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes,
    });
    await router.push("/machines");
    await router.isReady();

    const wrapper = mount(AppShell, {
      global: { plugins: [router] },
    });

    const main = wrapper.find("main");
    expect(main.text()).toContain("Machines");
  });
});
