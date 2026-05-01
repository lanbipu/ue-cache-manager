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

  it("has 7 routes", () => {
    expect(routes).toHaveLength(7);
  });

  it("includes all required view routes", async () => {
    const expectedPaths = [
      "/",
      "/machines",
      "/projects",
      "/ddc-pak",
      "/pso-cache",
      "/ini-scanner",
      "/health-check",
    ];
    const actualPaths = routes.map((r) => r.path);
    expectedPaths.forEach((p) => {
      expect(actualPaths).toContain(p);
    });
  });

  it("dashboard route resolves to Dashboard component", async () => {
    await router.push("/");
    await router.isReady();
    const matched = router.currentRoute.value.matched[0];
    expect(matched.name).toBe("dashboard");
  });
});
