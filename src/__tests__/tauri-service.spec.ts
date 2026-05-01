import { describe, it, expect, vi, beforeEach } from "vitest";

// vi.hoisted ensures mockInvoke is initialized before vi.mock factory runs
const { mockInvoke } = vi.hoisted(() => ({ mockInvoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));

import { tauriApi, type Machine } from "@/services/tauri";

describe("tauri service", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("listMachines invokes the correct command", async () => {
    const fake: Machine[] = [
      {
        id: 1,
        hostname: "RENDER-01",
        ip: "192.168.10.21",
        role: "render",
        status: "online",
        last_seen_at: null,
      },
    ];
    mockInvoke.mockResolvedValue(fake);
    const result = await tauriApi.listMachines();
    expect(mockInvoke).toHaveBeenCalledWith("list_machines");
    expect(result).toEqual(fake);
  });

  it("addMachine passes hostname and ip", async () => {
    mockInvoke.mockResolvedValue(42);
    const id = await tauriApi.addMachine("RENDER-02", "192.168.10.22");
    expect(mockInvoke).toHaveBeenCalledWith("add_machine", {
      hostname: "RENDER-02",
      ip: "192.168.10.22",
    });
    expect(id).toBe(42);
  });

  it("deleteMachine passes id", async () => {
    mockInvoke.mockResolvedValue(undefined);
    await tauriApi.deleteMachine(7);
    expect(mockInvoke).toHaveBeenCalledWith("delete_machine", { id: 7 });
  });

  it("testPowerShellBridge passes message", async () => {
    const fake = { received: "hi", timestamp: "2026-01-01", machine: "PC" };
    mockInvoke.mockResolvedValue(fake);
    const result = await tauriApi.testPowerShellBridge("hi");
    expect(mockInvoke).toHaveBeenCalledWith("test_powershell_bridge", { message: "hi" });
    expect(result).toEqual(fake);
  });
});
