import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useClusterStore } from "@/stores/cluster";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { useMachinesStore } from "@/stores/machines";

describe("cluster store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("computes summary metrics", () => {
    const machines = useMachinesStore();
    machines.machines = [
      { id: 1, hostname: "A", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
      { id: 2, hostname: "B", ip: "1.1.1.2", role: "render", status: "offline", last_seen_at: null },
    ];
    const store = useClusterStore();
    expect(store.total).toBe(2);
    expect(store.online).toBe(1);
    expect(store.summary).toBe("1/2 online");
  });

  it("factors latest health check warnings into score", () => {
    const machines = useMachinesStore();
    machines.machines = [
      { id: 1, hostname: "A", ip: "1.1.1.1", role: "render", status: "online", last_seen_at: null },
    ];
    const health = useHealthCheckStore();
    health.results = [
      {
        id: 1,
        scan_run_id: 1,
        machine_id: 1,
        machine_results: { smb: { status: "warning", message: "warn", sample: "", remediation: "" } },
      },
    ];
    const store = useClusterStore();
    expect(store.warning).toBe(1);
    expect(store.score).toBeLessThan(100);
  });
});
