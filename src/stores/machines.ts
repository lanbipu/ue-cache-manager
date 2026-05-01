import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type Machine, type UecmError } from "@/services/tauri";

export const useMachinesStore = defineStore("machines", () => {
  const machines = ref<Machine[]>([]);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function loadMachines() {
    isLoading.value = true;
    error.value = null;
    try {
      machines.value = await tauriApi.listMachines();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function addMachine(hostname: string, ip: string) {
    error.value = null;
    try {
      await tauriApi.addMachine(hostname, ip);
      await loadMachines();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function deleteMachine(id: number) {
    error.value = null;
    try {
      await tauriApi.deleteMachine(id);
      await loadMachines();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  return {
    machines,
    isLoading,
    error,
    loadMachines,
    addMachine,
    deleteMachine,
  };
});
