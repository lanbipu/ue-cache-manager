import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useTasksStore } from "@/stores/tasks";

describe("tasks store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("starts without a fabricated active task", () => {
    const store = useTasksStore();
    expect(store.activeTask).toBeNull();
  });
});
