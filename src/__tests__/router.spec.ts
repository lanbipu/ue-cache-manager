import { describe, it, expect, beforeEach } from "vitest";
import { createRouter, createMemoryHistory, type Router } from "vue-router";
import { routes } from "@/router";

describe("router", () => {
  let router: Router;

  beforeEach(() => {
    router = createRouter({
      history: createMemoryHistory(),
      routes,
    });
  });

  it("exposes the machines route as the single app surface", () => {
    const named = routes.filter((r) => r.name !== undefined);
    expect(named).toHaveLength(1);
    expect(named[0].name).toBe("machines");
    expect(named[0].path).toBe("/machines");
  });

  it("redirects / to /machines", async () => {
    await router.push("/");
    await router.isReady();
    expect(router.currentRoute.value.path).toBe("/machines");
    expect(router.currentRoute.value.name).toBe("machines");
  });

  it("preserves host and tab query params on /machines", async () => {
    await router.push("/machines?host=PC-02&tab=ddc");
    await router.isReady();
    expect(router.currentRoute.value.query.host).toBe("PC-02");
    expect(router.currentRoute.value.query.tab).toBe("ddc");
  });

  it.each(["/projects", "/ddc-pak", "/pso-cache", "/ini-scanner", "/health-check", "/shares", "/anything"])(
    "redirects legacy path %s to /machines",
    async (legacy) => {
      await router.push(legacy);
      await router.isReady();
      expect(router.currentRoute.value.path).toBe("/machines");
      expect(router.currentRoute.value.name).toBe("machines");
    },
  );

  it("preserves query params when redirecting a legacy path", async () => {
    await router.push("/ini-scanner?finding=R008");
    await router.isReady();
    expect(router.currentRoute.value.path).toBe("/machines");
    expect(router.currentRoute.value.query.finding).toBe("R008");
  });
});
