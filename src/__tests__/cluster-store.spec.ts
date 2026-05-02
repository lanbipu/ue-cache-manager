import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useClusterStore } from "@/stores/cluster";
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
});
