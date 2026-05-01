import { invoke } from "@tauri-apps/api/core";

export interface Machine {
  id: number | null;
  hostname: string;
  ip: string;
  role: string;
  status: string;
  last_seen_at: string | null;
}

export interface EchoResult {
  received: string;
  timestamp: string;
  machine: string;
}

export interface UecmError {
  code: string;
  message: string;
}

/**
 * Typed wrapper around Tauri invoke calls. All frontend code should
 * use this rather than calling invoke() directly. Centralizes:
 * - Type safety
 * - Mocking surface for tests
 * - Error normalization (later)
 */
export const tauriApi = {
  async listMachines(): Promise<Machine[]> {
    return invoke<Machine[]>("list_machines");
  },

  async addMachine(hostname: string, ip: string): Promise<number> {
    return invoke<number>("add_machine", { hostname, ip });
  },

  async deleteMachine(id: number): Promise<void> {
    return invoke<void>("delete_machine", { id });
  },

  async testPowerShellBridge(message: string): Promise<EchoResult> {
    return invoke<EchoResult>("test_powershell_bridge", { message });
  },
};
