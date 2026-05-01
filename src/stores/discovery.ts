import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type ProbedHost, type UecmError } from "@/services/tauri";

export const useDiscoveryStore = defineStore("discovery", () => {
  const cidr = ref("192.168.10.0/24");
  const probed = ref<ProbedHost[]>([]);
  const isScanning = ref(false);
  const error = ref<UecmError | null>(null);

  async function scan(input?: string) {
    if (input) cidr.value = input;
    isScanning.value = true;
    error.value = null;
    probed.value = [];
    try {
      const result = await tauriApi.scanNetwork(cidr.value);
      probed.value = result.probed;
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isScanning.value = false;
    }
  }

  async function addToInventory(ip: string, hostname: string | null) {
    error.value = null;
    try {
      return await tauriApi.addDiscoveredMachine(ip, hostname);
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  return {
    cidr,
    probed,
    isScanning,
    error,
    scan,
    addToInventory,
  };
});
