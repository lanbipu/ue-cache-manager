<script setup lang="ts">
import { ref } from "vue";
import { tauriApi, type EchoResult, type UecmError } from "@/services/tauri";

const result = ref<EchoResult | null>(null);
const error = ref<UecmError | null>(null);
const loading = ref(false);

async function runBridgeTest() {
  result.value = null;
  error.value = null;
  loading.value = true;
  try {
    result.value = await tauriApi.testPowerShellBridge("hello from UECM");
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="p-6">
    <h1 class="text-2xl font-semibold">Dashboard</h1>
    <p class="mt-1 text-sm text-gray-500">
      Plan 1 demonstration view. Real cluster overview comes in later plans.
    </p>

    <section class="mt-6 border-t pt-4">
      <h2 class="font-medium mb-2">PowerShell bridge smoke test</h2>
      <p class="text-sm text-gray-500 mb-2">
        Verifies frontend → Rust → PowerShell sidecar pipeline works.
        On non-Windows dev machines, this will return a "Windows-only" error — that's expected.
      </p>
      <button
        data-bridge-test-btn
        :disabled="loading"
        @click="runBridgeTest"
        class="px-3 py-1 bg-gray-200 rounded text-sm hover:bg-gray-300 disabled:opacity-50"
      >
        {{ loading ? "Running..." : "Run bridge test" }}
      </button>

      <pre
        v-if="result"
        class="mt-3 p-3 bg-gray-50 border rounded text-xs"
      >{{ JSON.stringify(result, null, 2) }}</pre>

      <p v-if="error" class="mt-3 text-sm text-red-600">
        {{ error.code }}: {{ error.message }}
      </p>
    </section>
  </div>
</template>
