import { defineStore } from "pinia";
import { ref } from "vue";
import {
  tauriApi,
  type InjectionResult,
  type ShareConfig,
  type ShareCreateResult,
  type ShareMode,
  type UecmError,
} from "@/services/tauri";

export const useSharesStore = defineStore("shares", () => {
  const shares = ref<ShareConfig[]>([]);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      shares.value = await tauriApi.listShares();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function create(
    hostMachineId: number,
    mode: ShareMode,
    shareName: string,
    localPath: string,
    operatorCredentialAlias: string | null,
    svcUsername: string | null,
  ): Promise<ShareCreateResult> {
    error.value = null;
    try {
      const result = await tauriApi.createShare(
        hostMachineId,
        mode,
        shareName,
        localPath,
        operatorCredentialAlias,
        svcUsername,
      );
      await load();
      return result;
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  async function inject(
    shareConfigId: number,
    clientMachineIds: number[],
    operatorCredentialAlias: string | null,
  ): Promise<InjectionResult[]> {
    error.value = null;
    try {
      return await tauriApi.injectShareCredentialToClients(
        shareConfigId,
        clientMachineIds,
        operatorCredentialAlias,
      );
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  async function remove(shareConfigId: number, alsoRemoveRemote: boolean) {
    error.value = null;
    try {
      await tauriApi.deleteShare(shareConfigId, alsoRemoveRemote);
      await load();
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  return { shares, isLoading, error, load, create, inject, remove };
});
