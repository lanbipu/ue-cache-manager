import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { tauriApi, type BatchEvent, type UecmError } from "@/services/tauri";

const BATCH_EVENT_NAME = "batch-progress";

export const useBatchStore = defineStore("batch", () => {
  const events = ref<BatchEvent[]>([]);
  const isRunning = ref(false);
  const error = ref<UecmError | null>(null);
  let unlisten: UnlistenFn | null = null;

  async function ensureListener() {
    if (unlisten) return;
    unlisten = await listen<BatchEvent>(BATCH_EVENT_NAME, (e) => {
      events.value.push(e.payload);
    });
  }

  async function disposeListener() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  }

  function reset() {
    events.value = [];
    error.value = null;
  }

  async function runEnvVar(
    machineIds: number[],
    name: string,
    value: string,
    credentialAlias: string,
  ) {
    reset();
    isRunning.value = true;
    await ensureListener();
    try {
      await tauriApi.batchSetEnvVar(machineIds, name, value, credentialAlias);
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    } finally {
      isRunning.value = false;
    }
  }

  async function runIniKey(
    machineIds: number[],
    filePath: string,
    section: string,
    name: string,
    value: string,
    credentialAlias: string,
  ) {
    reset();
    isRunning.value = true;
    await ensureListener();
    try {
      await tauriApi.batchSetIniKey(
        machineIds,
        filePath,
        section,
        name,
        value,
        credentialAlias,
      );
    } catch (e) {
      error.value = e as UecmError;
      throw e;
    } finally {
      isRunning.value = false;
    }
  }

  // Latest event per machine_id — collapses Running/Ok/Err sequence so the
  // progress table can render current state directly.
  const byMachine = computed(() => {
    const map = new Map<number, BatchEvent>();
    for (const ev of events.value) {
      map.set(ev.machine_id, ev);
    }
    return map;
  });

  return {
    events,
    isRunning,
    error,
    runEnvVar,
    runIniKey,
    reset,
    byMachine,
    disposeListener,
  };
});
