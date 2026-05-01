import { defineStore } from "pinia";
import { ref } from "vue";
import { tauriApi, type CredentialRecord, type UecmError } from "@/services/tauri";

export const useCredentialsStore = defineStore("credentials", () => {
  const credentials = ref<CredentialRecord[]>([]);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      credentials.value = await tauriApi.listCredentials();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function save(alias: string, kind: string, username: string, password: string) {
    error.value = null;
    try {
      await tauriApi.saveCredential(alias, kind, username, password);
      await load();
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  async function remove(alias: string) {
    error.value = null;
    try {
      await tauriApi.deleteCredential(alias);
      await load();
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    }
  }

  return {
    credentials,
    isLoading,
    error,
    load,
    save,
    remove,
  };
});
