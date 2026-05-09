import { defineStore } from "pinia";
import { ref } from "vue";
import {
  tauriApi,
  type Machine,
  type MachineDetail,
  type UecmError,
  type WinrmBootstrapResult,
} from "@/services/tauri";

export const useMachinesStore = defineStore("machines", () => {
  const machines = ref<Machine[]>([]);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  const selectedDetail = ref<MachineDetail | null>(null);
  const isDetailLoading = ref(false);

  const refreshError = ref<string | null>(null);
  const isRefreshing = ref(false);
  const bootstrapResult = ref<WinrmBootstrapResult | null>(null);
  const bootstrapError = ref<string | null>(null);
  const isBootstrapping = ref(false);
  const bootstrapScript = ref<string | null>(null);
  const isLoadingBootstrapScript = ref(false);

  function clearBootstrapState() {
    bootstrapResult.value = null;
    bootstrapError.value = null;
    bootstrapScript.value = null;
  }

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
      if (selectedDetail.value?.machine.id === id) {
        selectedDetail.value = null;
        clearBootstrapState();
      }
      await loadMachines();
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function renameMachine(id: number, hostname: string) {
    error.value = null;
    try {
      await tauriApi.renameMachine(id, hostname);
      await loadMachines();
      if (selectedDetail.value?.machine.id === id) {
        await selectMachine(id);
      }
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function selectMachine(id: number) {
    isDetailLoading.value = true;
    error.value = null;
    if (selectedDetail.value?.machine.id !== id) {
      clearBootstrapState();
    }
    try {
      selectedDetail.value = await tauriApi.getMachineDetail(id);
    } catch (e) {
      error.value = e as UecmError;
      selectedDetail.value = null;
      clearBootstrapState();
    } finally {
      isDetailLoading.value = false;
    }
  }

  function clearSelection() {
    selectedDetail.value = null;
    clearBootstrapState();
  }

  async function refreshSelected() {
    if (!selectedDetail.value?.machine.id) return;
    const id = selectedDetail.value.machine.id;
    isRefreshing.value = true;
    error.value = null;
    refreshError.value = null;
    try {
      const result = await tauriApi.refreshMachine(id);
      refreshError.value = result.winrm_ok ? null : result.error ?? "WinRM unreachable";
      // Re-read detail to pick up updated UE/GPU rows
      await selectMachine(id);
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isRefreshing.value = false;
    }
  }

  async function bootstrapSelected(credentialAlias: string, enableLocalAccountRemoteAdmin = false) {
    if (!selectedDetail.value?.machine.id) return;
    const id = selectedDetail.value.machine.id;
    isBootstrapping.value = true;
    error.value = null;
    bootstrapError.value = null;
    bootstrapResult.value = null;
    try {
      const result = await tauriApi.bootstrapWinrm(id, credentialAlias, enableLocalAccountRemoteAdmin);
      bootstrapResult.value = result;
      if (!result.ok) {
        bootstrapError.value = result.message;
        if (result.manual_script) {
          bootstrapScript.value = result.manual_script;
        }
      }
      await selectMachine(id);
    } catch (e) {
      error.value = e as UecmError;
      bootstrapError.value = (e as UecmError).message;
    } finally {
      isBootstrapping.value = false;
    }
  }

  async function loadBootstrapScript() {
    isLoadingBootstrapScript.value = true;
    error.value = null;
    try {
      bootstrapScript.value = await tauriApi.getWinrmBootstrapScript();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoadingBootstrapScript.value = false;
    }
  }

  return {
    machines,
    isLoading,
    error,
    selectedDetail,
    isDetailLoading,
    refreshError,
    isRefreshing,
    bootstrapResult,
    bootstrapError,
    isBootstrapping,
    bootstrapScript,
    isLoadingBootstrapScript,
    loadMachines,
    addMachine,
    deleteMachine,
    renameMachine,
    selectMachine,
    clearSelection,
    refreshSelected,
    bootstrapSelected,
    loadBootstrapScript,
  };
});
