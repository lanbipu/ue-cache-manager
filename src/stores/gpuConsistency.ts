import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { tauriApi, type GpuMatrix, type UecmError } from "@/services/tauri";

export const useGpuConsistencyStore = defineStore("gpuConsistency", () => {
  const matrix = ref<GpuMatrix | null>(null);
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  const baselineLabel = computed(() => {
    const baseline = matrix.value?.baseline;
    return baseline
      ? `${baseline.vendor} ${baseline.model} (driver ${baseline.driver})`
      : "-";
  });

  const deviationCount = computed(
    () => matrix.value?.cells.filter((cell) => cell.status === "deviation").length ?? 0,
  );

  const unknownCount = computed(
    () => matrix.value?.cells.filter((cell) => cell.status === "unknown").length ?? 0,
  );

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      matrix.value = await tauriApi.getGpuConsistencyMatrix();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  return {
    matrix,
    isLoading,
    error,
    baselineLabel,
    deviationCount,
    unknownCount,
    load,
  };
});
