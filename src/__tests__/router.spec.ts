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

  it("exposes the machines, deploy, and diagnostics routes as named surfaces", () => {
    const named = routes.filter((r) => r.name !== undefined);
    expect(named).toHaveLength(3);
    const machinesRoute = named.find((r) => r.name === "machines");
    expect(machinesRoute).toBeDefined();
    expect(machinesRoute!.path).toBe("/machines");
    const deployRoute = named.find((r) => r.name === "deploy");
    expect(deployRoute).toBeDefined();
    expect(deployRoute!.path).toBe("/deploy");
    const diagnosticsRoute = named.find((r) => r.name === "diagnostics");
    expect(diagnosticsRoute).toBeDefined();
    expect(diagnosticsRoute!.path).toBe("/diagnostics");
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
