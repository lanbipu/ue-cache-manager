<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import Button from "@/components/ui/Button.vue";
import FindingHierarchy from "@/components/diagnostics/FindingHierarchy.vue";
import FindingDetail from "@/components/diagnostics/FindingDetail.vue";
import IniScanWizard from "@/components/modals/IniScanWizard.vue";
import { useDiagnosticsStore } from "@/stores/diagnostics";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import type { IniFinding } from "@/services/tauri";

const diag = useDiagnosticsStore();
const machines = useMachinesStore();
const creds = useCredentialsStore();

const showWizard = ref(false);
const selectedFinding = ref<IniFinding | null>(null);
const grouping = ref<"machine" | "category">("machine");
const applying = ref(false);

const hostnameById = computed<Record<number, string>>(() => {
  const out: Record<number, string> = {};
  for (const m of machines.machines) if (m.id != null) out[m.id] = m.hostname;
  return out;
});

onMounted(async () => {
  await machines.loadMachines();
  await creds.load();
});

async function onApply(f: IniFinding) {
  if (creds.credentials.length === 0) return;
  applying.value = true;
  await diag.applyFinding(f.id!, creds.credentials[0].alias);
  applying.value = false;
}
async function onSkip(f: IniFinding) { await diag.skipFinding(f.id!); }
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="space-y-4 p-6">
      <UecmPageHeader title="INI Scanner" eyebrow="Config drift"
        description="Scan project / user / engine INI files across machines, diagnose conflicts, apply fixes with auto-backup.">
        <template #actions>
          <Button data-open-ini-scan-btn @click="showWizard = true">
            <UecmIcon name="play" /> Run scan
          </Button>
        </template>
      </UecmPageHeader>
      <section class="grid grid-cols-4 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmKpiTile label="Critical" :value="diag.summary.critical" tone="critical" />
        <UecmKpiTile label="Warning"  :value="diag.summary.warning"  tone="warning" />
        <UecmKpiTile label="Healthy"  :value="diag.summary.healthy"  tone="healthy" />
        <UecmKpiTile label="Open"     :value="diag.open.length"      tone="info" />
      </section>
    </div>

    <section v-if="diag.findings.length === 0" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="file-search" size="32" class="mx-auto text-muted-foreground" />
        <p class="mt-2 font-display text-lg font-extrabold">Run an INI scan</p>
        <p class="mt-1 text-sm text-muted-foreground">No scan results yet. Click "Run scan" above.</p>
      </div>
    </section>

    <section v-else class="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[2fr_3fr]">
      <FindingHierarchy class="border-r" :findings="diag.findings" :selected-id="selectedFinding?.id ?? null"
                        :hostname-by-id="hostnameById" :group-by="grouping"
                        @select="selectedFinding = $event" />
      <FindingDetail :finding="selectedFinding" :busy="applying"
                     @apply="onApply" @skip="onSkip" />
    </section>

    <IniScanWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
