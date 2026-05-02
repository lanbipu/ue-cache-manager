<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Button from "@/components/ui/Button.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmStat from "@/components/primitives/UecmStat.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";
import { useMachinesStore } from "@/stores/machines";
import { tauriApi, type EchoResult, type UecmError } from "@/services/tauri";

const machines = useMachinesStore();
const result = ref<EchoResult | null>(null);
const error = ref<UecmError | null>(null);
const loading = ref(false);
const showShareWizard = ref(false);

const online = computed(() => machines.machines.filter((machine) => machine.status.toLowerCase() === "online").length);
const critical = computed(() => machines.machines.filter((machine) => ["critical", "error", "failed"].includes(machine.status.toLowerCase())).length);
const warning = computed(() => machines.machines.filter((machine) => ["warning", "warn", "degraded"].includes(machine.status.toLowerCase())).length);
function alertTone(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "offline") return "offline" as const;
  if (["critical", "error", "failed"].includes(normalized)) return "critical" as const;
  return "warning" as const;
}
const alerts = computed(() =>
  machines.machines
    .filter((machine) => machine.status.toLowerCase() !== "online")
    .map((machine) => ({
      tone: alertTone(machine.status),
      title: `${machine.hostname} ${machine.status}`,
      detail: `${machine.ip} last seen ${machine.last_seen_at ?? "unknown"}`,
    })),
);

onMounted(() => {
  machines.loadMachines();
});

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
  <div class="space-y-6 p-6">
    <UecmPageHeader
      title="Dashboard"
      eyebrow="Cluster overview"
      description="Operational snapshot for Shared DDC, remote credentials, INI drift, and render-node health."
    >
      <template #actions>
        <Button variant="outline" data-bridge-test-btn :disabled="loading" @click="runBridgeTest">
          <UecmIcon name="terminal" />
          {{ loading ? "Running..." : "Run bridge test" }}
        </Button>
        <Button @click="showShareWizard = true">
          <UecmIcon name="plus" />
          Create Shared DDC
        </Button>
      </template>
    </UecmPageHeader>

    <section class="grid gap-4 md:grid-cols-4">
      <UecmStat label="Machines" :value="machines.machines.length" icon="server" :detail="`${online} online`" />
      <UecmStat label="Critical" :value="critical" icon="alert-triangle" detail="SYSTEM / INI blockers" />
      <UecmStat label="Warnings" :value="warning" icon="info" detail="Driver or config drift" />
      <UecmStat label="DDC Path" value="\\HOST-01" icon="hard-drive" detail="Shared cache baseline" />
    </section>

    <section class="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
      <article class="rounded-lg border bg-card p-4">
        <div class="mb-3 flex items-center justify-between">
          <h2 class="font-display text-lg font-extrabold">Cluster Alerts</h2>
          <UecmStatusBadge tone="warning" :label="`${alerts.length} open`" size="sm" />
        </div>
        <p v-if="machines.machines.length === 0" class="py-6 text-sm text-muted-foreground">
          No machines registered yet. Use Machines > Scan to build the inventory.
        </p>
        <div v-else-if="alerts.length === 0" class="py-6 text-sm text-muted-foreground">
          No open machine status alerts.
        </div>
        <div v-else class="divide-y">
          <div v-for="alert in alerts" :key="alert.title" class="flex items-start gap-3 py-3">
            <UecmStatusBadge :tone="alert.tone" :label="alert.tone" size="sm" />
            <div>
              <div class="text-sm font-bold">{{ alert.title }}</div>
              <div class="text-sm text-muted-foreground">{{ alert.detail }}</div>
            </div>
          </div>
        </div>
      </article>

      <article class="rounded-lg border bg-card p-4">
        <h2 class="font-display text-lg font-extrabold">PowerShell Bridge</h2>
        <p class="mt-1 text-sm text-muted-foreground">
          Verifies frontend to Rust to PowerShell sidecar pipeline. Non-Windows machines may return a Windows-only error.
        </p>
        <pre v-if="result" class="mt-4 overflow-auto rounded-md border bg-muted p-3 font-mono text-xs">{{ JSON.stringify(result, null, 2) }}</pre>
        <p v-if="error" class="mt-4 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive">
          {{ error.code }}: {{ error.message }}
        </p>
      </article>
    </section>

    <ShareCreateWizard v-if="showShareWizard" :open="showShareWizard" @close="showShareWizard = false" />
  </div>
</template>
