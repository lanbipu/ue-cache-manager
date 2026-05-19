<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import ConsistencyReport from "@/components/diagnostics/ConsistencyReport.vue";

const { t } = useI18n();
const machines = useMachinesStore();
const credentials = useCredentialsStore();

const selectedIds = ref<number[]>([]);
const credAlias = ref("");
const running = ref(false);
const inconsistencies = ref<unknown[]>([]);

async function onRun() {
  running.value = true;
  try {
    const hosts = machines.machines
      .filter((m) => m.id !== null && selectedIds.value.includes(m.id!))
      .map((m) => m.hostname);
    const [, found] = await invoke<[unknown[], unknown[]]>("run_consistency_check", {
      hosts,
      credentialAlias: credAlias.value || null,
    });
    inconsistencies.value = found;
  } finally {
    running.value = false;
  }
}
</script>

<template>
  <div class="p-6 max-w-4xl">
    <h1 class="font-display text-2xl mb-3">{{ t("diagnostics.pageTitle") }}</h1>
    <p class="text-muted-foreground mb-4">{{ t("diagnostics.pageDesc") }}</p>

    <section class="mb-4 rounded-md border border-border p-3">
      <h3 class="font-display text-base mb-2">{{ t("diagnostics.selectHosts") }}</h3>
      <div class="max-h-40 overflow-y-auto">
        <label
          v-for="m in machines.machines"
          :key="m.hostname"
          class="flex items-center gap-2 text-sm"
        >
          <input type="checkbox" :value="m.id" v-model="selectedIds" />
          {{ m.hostname }}
        </label>
      </div>
    </section>

    <section class="mb-4">
      <label class="block text-sm mb-1">{{ t("diagnostics.cred") }}</label>
      <select
        v-model="credAlias"
        class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm mb-3"
      >
        <option value="">--</option>
        <option
          v-for="c in credentials.credentials"
          :key="c.alias"
          :value="c.alias"
        >{{ c.alias }}</option>
      </select>
      <button
        class="px-4 py-2 rounded bg-primary text-primary-foreground text-sm disabled:opacity-50"
        :disabled="running || selectedIds.length < 2"
        @click="onRun"
      >
        {{ running ? t("diagnostics.running") : t("diagnostics.runCheck") }}
      </button>
    </section>

    <ConsistencyReport :inconsistencies="(inconsistencies as any[])" />
  </div>
</template>
