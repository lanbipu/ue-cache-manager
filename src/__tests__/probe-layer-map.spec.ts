import { describe, it, expect } from "vitest";
import { PROBE_LAYER_MAP, type ProbeLayer } from "@/services/tauri";

describe("PROBE_LAYER_MAP", () => {
  const expectedL1 = ["tcp_5985", "tcp_445", "tcp_135"];
  const expectedL2 = ["firewall_445", "local_account_token_filter", "long_paths_enabled", "lanman_server"];
  const expectedL3Business = [
    "share_reachable", "ntfs_perm", "cred_user", "cred_system",
    "env_vars", "system_write", "winmgmt",
  ];
  const expectedL3Derived = ["ini_consistency", "pso_precaching", "gpu_consistency"];

  it("has exactly 17 entries matching the Rust PROBE_REGISTRY", () => {
    expect(Object.keys(PROBE_LAYER_MAP).length).toBe(17);
  });

  it("L1 keys are correct", () => {
    for (const k of expectedL1) {
      expect(PROBE_LAYER_MAP[k as keyof typeof PROBE_LAYER_MAP]).toBe("l1_port");
    }
  });

  it("L2 keys are correct", () => {
    for (const k of expectedL2) {
      expect(PROBE_LAYER_MAP[k as keyof typeof PROBE_LAYER_MAP]).toBe("l2_bootstrap");
    }
  });

  it("L3 business + derived keys are correct", () => {
    for (const k of [...expectedL3Business, ...expectedL3Derived]) {
      expect(PROBE_LAYER_MAP[k as keyof typeof PROBE_LAYER_MAP]).toBe("l3_business");
    }
  });

  it("no unexpected keys", () => {
    const expected = new Set([...expectedL1, ...expectedL2, ...expectedL3Business, ...expectedL3Derived]);
    for (const k of Object.keys(PROBE_LAYER_MAP)) {
      expect(expected.has(k)).toBe(true);
    }
  });
});
