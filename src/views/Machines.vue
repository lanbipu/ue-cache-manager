<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useMachinesStore } from "@/stores/machines";
import Button from "@/components/ui/Button.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import MachineDetail from "@/components/machines/MachineDetail.vue";
import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";
import CredentialDialog from "@/components/modals/CredentialDialog.vue";
import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";
import IniEditModal from "@/components/modals/IniEditModal.vue";
import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";
import BatchEnvVarModal from "@/components/modals/BatchEnvVarModal.vue";
import BatchIniEditModal from "@/components/modals/BatchIniEditModal.vue";

const { t } = useI18n();
const store = useMachinesStore();
const showDiscovery = ref(false);
const showCredentials = ref(false);
const showEnvVar = ref(false);
const showIniEdit = ref(false);
const showShareWizard = ref(false);
const showBatchEnv = ref(false);
const showBatchIni = ref(false);

const selectedId = computed(() => store.selectedDetail?.machine.id ?? null);
const checkedIds = ref<Set<number>>(new Set());
const checkedArray = computed(() => Array.from(checkedIds.value));

onMounted(() => {
  store.loadMachines();
});

async function onSelect(id: number | null) {
  if (id === null) return;
  await store.selectMachine(id);
}

async function onDelete(id: number | null) {
  if (id === null) return;
  await store.deleteMachine(id);
  checkedIds.value.delete(id);
}

function toggleCheck(id: number | null) {
  if (id == null) return;
  const next = new Set(checkedIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  checkedIds.value = next;
}

function toggleAll() {
  const allIds = store.machines.map((machine) => machine.id).filter((id): id is number => id != null);
  checkedIds.value = checkedIds.value.size === allIds.length ? new Set() : new Set(allIds);
}

const allChecked = computed(() => {
  const total = store.machines.filter((machine) => machine.id != null).length;
  return total > 0 && checkedIds.value.size === total;
});
</script>

<template>
  <div class="grid h-full grid-rows-[auto_1fr] gap-4 p-6">
    <UecmPageHeader :title="t('machines.title')" :eyebrow="t('machines.eyebrow')" :description="t('machines.description')">
      <template #actions>
        <Button variant="outline" data-create-share-btn @click="showShareWizard = true">
          <UecmIcon name="folder-open" />
          {{ t("machines.share") }}
        </Button>
        <Button data-discover-btn @click="showDiscovery = true">
          <UecmIcon name="radar" />
          {{ t("machines.scan") }}
        </Button>
      </template>
    </UecmPageHeader>

    <div class="grid min-h-0 gap-4 lg:grid-cols-[22rem_1fr]">
      <aside class="min-h-0 overflow-hidden rounded-lg border bg-card">
        <div class="flex items-center justify-between gap-2 border-b p-3">
          <label class="flex items-center gap-2 text-xs font-bold uppercase tracking-wide text-muted-foreground">
            <input data-select-all type="checkbox" :checked="allChecked" @change="toggleAll" />
            {{ t("machines.selectedCount", { count: checkedIds.size }) }}
          </label>
          <div class="flex gap-1">
            <Button data-batch-env-btn variant="outline" size="sm" :disabled="checkedIds.size === 0" @click="showBatchEnv = true">{{ t("machines.batchEnv") }}</Button>
            <Button data-batch-ini-btn variant="outline" size="sm" :disabled="checkedIds.size === 0" @click="showBatchIni = true">{{ t("machines.batchIni") }}</Button>
          </div>
        </div>

        <div class="h-[calc(100%-3.5rem)] overflow-auto p-2">
          <p v-if="store.isLoading" class="p-3 text-sm text-muted-foreground">{{ t("common.loading") }}</p>
          <p v-else-if="store.machines.length === 0" class="p-3 text-sm text-muted-foreground">
            {{ t("machines.noMachines") }}
          </p>
          <ul v-else class="space-y-1">
            <li
              v-for="machine in store.machines"
              :key="machine.id ?? machine.ip"
              data-machine-row
              class="flex cursor-pointer items-center gap-3 rounded-md px-3 py-2 hover:bg-accent"
              :class="store.selectedDetail?.machine.id === machine.id ? 'bg-accent' : ''"
              @click="onSelect(machine.id)"
            >
              <input
                data-machine-check
                type="checkbox"
                :checked="machine.id != null && checkedIds.has(machine.id)"
                @click.stop
                @change="toggleCheck(machine.id)"
              />
              <div class="min-w-0 flex-1">
                <div class="truncate text-sm font-bold">{{ machine.hostname }}</div>
                <div class="font-mono text-xs text-muted-foreground">{{ machine.ip }}</div>
              </div>
              <button class="text-xs text-destructive hover:underline" @click.stop="onDelete(machine.id)">{{ t("common.delete") }}</button>
            </li>
          </ul>
          <p v-if="store.error" class="mt-3 px-3 text-xs text-destructive">{{ store.error.message }}</p>
        </div>
      </aside>

      <main class="min-h-0 overflow-auto rounded-lg border bg-card">
        <MachineDetail
          @open-credential-modal="showCredentials = true"
          @open-env-var-modal="showEnvVar = true"
          @open-ini-edit-modal="showIniEdit = true"
        />
      </main>
    </div>

    <DiscoveryWizard :open="showDiscovery" @close="showDiscovery = false" />
    <CredentialDialog :open="showCredentials" @close="showCredentials = false" />
    <EnvVarConfigModal :open="showEnvVar" :machine-id="selectedId" var-name="UE-SharedDataCachePath" @close="showEnvVar = false" />
    <IniEditModal :open="showIniEdit" :machine-id="selectedId" @close="showIniEdit = false" />
    <ShareCreateWizard :open="showShareWizard" @close="showShareWizard = false" />
    <BatchEnvVarModal :open="showBatchEnv" :machine-ids="checkedArray" @close="showBatchEnv = false" />
    <BatchIniEditModal :open="showBatchIni" :machine-ids="checkedArray" @close="showBatchIni = false" />
  </div>
</template>
