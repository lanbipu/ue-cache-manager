<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmKpiTile from "@/components/primitives/UecmKpiTile.vue";
import Button from "@/components/ui/Button.vue";
import FindingHierarchy from "@/components/diagnostics/FindingHierarchy.vue";
import FindingDetail from "@/components/diagnostics/FindingDetail.vue";
import IniScanWizard from "@/components/modals/IniScanWizard.vue";
import { useCredentialsStore } from "@/stores/credentials";
import { useDiagnosticsStore } from "@/stores/diagnostics";
import { useMachinesStore } from "@/stores/machines";
import type { IniFinding } from "@/services/tauri";

const diagnostics = useDiagnosticsStore();
const machines = useMachinesStore();
const credentials = useCredentialsStore();
const showWizard = ref(false);
const selectedFinding = ref<IniFinding | null>(null);

const hostnameById = computed<Record<number, string>>(() => {
  const out: Record<number, string> = {};
  for (const machine of machines.machines) if (machine.id != null) out[machine.id] = machine.hostname;
  return out;
});

watch(() => diagnostics.findings, (findings) => {
  if (!selectedFinding.value && findings.length > 0) selectedFinding.value = findings[0];
}, { deep: true });

onMounted(async () => {
  await machines.loadMachines();
  await credentials.load();
});

async function apply(finding: IniFinding) {
  const alias = credentials.credentials[0]?.alias;
  if (!alias || finding.id == null) return;
  await diagnostics.applyFinding(finding.id, alias);
}

async function skip(finding: IniFinding) {
  if (finding.id != null) await diagnostics.skipFinding(finding.id);
}
</script>

<template>
  <div class="flex h-full flex-col">
    <div class="space-y-4 p-6">
      <UecmPageHeader title="INI Scanner" eyebrow="Config drift" description="Scan project, user, and engine INI files for DDC and cache drift.">
        <template #actions>
          <Button data-open-ini-scan-btn @click="showWizard = true">
            <UecmIcon name="play" /> Run scan
          </Button>
        </template>
      </UecmPageHeader>
      <section class="grid grid-cols-4 gap-px overflow-hidden rounded-lg border bg-border">
        <UecmKpiTile label="Critical" :value="diagnostics.summary.critical" tone="critical" />
        <UecmKpiTile label="Warning" :value="diagnostics.summary.warning" tone="warning" />
        <UecmKpiTile label="Healthy" :value="diagnostics.summary.healthy" tone="healthy" />
        <UecmKpiTile label="Files" :value="diagnostics.summary.total_files" tone="info" />
      </section>
    </div>

    <section v-if="diagnostics.findings.length === 0" class="grid flex-1 place-items-center text-center">
      <div>
        <UecmIcon name="file-search" size="32" class="mx-auto text-muted-foreground" />
        <h2 class="mt-4 font-display text-lg font-extrabold">Run an INI scan</h2>
        <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">No diagnostics have been collected in this session.</p>
      </div>
    </section>

    <section v-else class="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[2fr_3fr]">
      <FindingHierarchy class="border-r" :findings="diagnostics.findings" :selected-id="selectedFinding?.id ?? null" :hostname-by-id="hostnameById" @select="selectedFinding = $event" />
      <FindingDetail :finding="selectedFinding" :busy="diagnostics.isApplying" @apply="apply" @skip="skip" />
    </section>

    <IniScanWizard :open="showWizard" @close="showWizard = false" />
  </div>
</template>
