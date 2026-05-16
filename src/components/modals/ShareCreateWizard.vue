<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import { useSharesStore } from "@/stores/shares";
import type { ShareMode } from "@/services/tauri";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const { t } = useI18n();
const machines = useMachinesStore();
const credentials = useCredentialsStore();
const shares = useSharesStore();

const step = ref<1 | 2 | 3 | 4>(1);
const mode = ref<ShareMode>("open");
const hostMachineId = ref<number | null>(null);
const shareName = ref("DDC");
const localPath = ref("D:\\DDC");
const svcUsername = ref("ddc-svc");
const operatorAlias = ref<string | null>(null);
const isSubmitting = ref(false);
const submitError = ref<string | null>(null);
const successUnc = ref<string | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      step.value = 1;
      mode.value = "open";
      hostMachineId.value = null;
      shareName.value = "DDC";
      localPath.value = "D:\\DDC";
      svcUsername.value = "ddc-svc";
      operatorAlias.value = null;
      isSubmitting.value = false;
      submitError.value = null;
      successUnc.value = null;
      machines.loadMachines();
      credentials.load();
    }
  },
  { immediate: true },
);

const canAdvanceFrom = computed(() => {
  if (step.value === 1) return mode.value === "open" || mode.value === "managed";
  if (step.value === 2) return hostMachineId.value !== null;
  if (step.value === 3) {
    if (!shareName.value.trim() || !localPath.value.trim()) return false;
    if (mode.value === "managed" && !svcUsername.value.trim()) return false;
    return true;
  }
  return false;
});

function next() {
  if (step.value < 4) step.value = (step.value + 1) as 1 | 2 | 3 | 4;
}
function back() {
  if (step.value > 1) step.value = (step.value - 1) as 1 | 2 | 3 | 4;
}

function onHostChange(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  hostMachineId.value = v ? Number(v) : null;
}

const previewLines = computed(() => {
  const host =
    machines.machines.find((m) => m.id === hostMachineId.value)?.hostname ?? "<host>";
  const modeLabel = mode.value === "open"
    ? t("modal.shareWizard.previewModeOpen")
    : t("modal.shareWizard.previewModeManaged");
  const lines = [
    t("modal.shareWizard.previewMode", { value: modeLabel }),
    t("modal.shareWizard.previewHost", { value: host }),
    t("modal.shareWizard.previewShare", { value: `\\\\${host}\\${shareName.value}` }),
    t("modal.shareWizard.previewLocalPath", { value: localPath.value }),
  ];
  if (mode.value === "managed") {
    lines.push(t("modal.shareWizard.previewSvc", { user: svcUsername.value }));
  }
  if (operatorAlias.value) {
    lines.push(t("modal.shareWizard.previewOperator", { value: operatorAlias.value }));
  } else {
    lines.push(t("modal.shareWizard.previewOperator", { value: t("modal.shareWizard.previewOperatorDefault") }));
  }
  return lines;
});

async function onCreate() {
  if (hostMachineId.value === null) return;
  isSubmitting.value = true;
  submitError.value = null;
  try {
    const result = await shares.create(
      hostMachineId.value,
      mode.value,
      shareName.value.trim(),
      localPath.value.trim(),
      operatorAlias.value,
      mode.value === "managed" ? svcUsername.value.trim() : null,
    );
    successUnc.value = result.unc_path;
  } catch (e) {
    submitError.value = (e as { message?: string }).message ?? t("modal.shareWizard.createFailed");
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="t('modal.shareWizard.title')" @close="emit('close')">
    <div data-share-wizard>
      <div v-if="step === 1">
        <p class="text-sm mb-3">{{ t("modal.shareWizard.step1Hint") }}</p>
        <label class="flex items-start gap-2 mb-2 cursor-pointer">
          <input
            data-mode-open
            type="radio"
            value="open"
            v-model="mode"
            class="mt-1"
          />
          <span>
            <span class="font-medium">{{ t("modal.shareWizard.modeOpenTitle") }}</span>
            <span class="block text-xs text-muted-foreground">
              {{ t("modal.shareWizard.modeOpenHint") }}
            </span>
          </span>
        </label>
        <label class="flex items-start gap-2 cursor-pointer">
          <input
            data-mode-managed
            type="radio"
            value="managed"
            v-model="mode"
            class="mt-1"
          />
          <span>
            <span class="font-medium">{{ t("modal.shareWizard.modeManagedTitle") }}</span>
            <span class="block text-xs text-muted-foreground">
              {{ t("modal.shareWizard.modeManagedHint") }}
            </span>
          </span>
        </label>
      </div>

      <div v-else-if="step === 2">
        <p class="text-sm mb-3">{{ t("modal.shareWizard.step2Hint") }}</p>
        <select
          data-host-select
          :value="hostMachineId ?? ''"
          class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          @change="onHostChange"
        >
          <option value="" disabled>{{ t("modal.shareWizard.selectMachinePlaceholder") }}</option>
          <option v-for="m in machines.machines" :key="m.id ?? m.ip" :value="m.id">
            {{ m.hostname }} ({{ m.ip }})
          </option>
        </select>
        <p v-if="machines.machines.length === 0" class="mt-2 text-xs text-muted-foreground">
          {{ t("modal.shareWizard.noMachinesHint") }}
        </p>
      </div>

      <div v-else-if="step === 3">
        <label class="block text-sm mb-1">{{ t("modal.shareWizard.shareName") }}</label>
        <input
          data-share-name-input
          v-model="shareName"
          class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <label class="block text-sm mb-1">{{ t("modal.shareWizard.localPath") }}</label>
        <input
          data-local-path-input
          v-model="localPath"
          class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <template v-if="mode === 'managed'">
          <label class="block text-sm mb-1">{{ t("modal.shareWizard.svcUsername") }}</label>
          <input
            data-svc-user-input
            v-model="svcUsername"
            class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        </template>
        <label class="block text-sm mb-1">{{ t("modal.shareWizard.operatorCredential") }}</label>
        <select
          data-operator-cred-select
          v-model="operatorAlias"
          class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <option :value="null">{{ t("modal.shareWizard.operatorPlaceholder") }}</option>
          <option
            v-for="c in credentials.credentials.filter((cred) => cred.kind === 'winrm')"
            :key="c.id ?? c.alias"
            :value="c.alias"
          >
            {{ c.alias }} ({{ c.username }})
          </option>
        </select>
        <p class="mt-1 text-xs text-muted-foreground">
          {{ t("modal.shareWizard.operatorOnlyWinrm") }}
        </p>
      </div>

      <div v-else-if="step === 4">
        <p class="text-sm mb-2">{{ t("modal.shareWizard.previewHint") }}</p>
        <pre
          data-preview
          class="whitespace-pre-wrap rounded bg-muted/40 p-2 font-mono text-xs text-foreground"
        >{{ previewLines.join("\n") }}</pre>
        <p v-if="submitError" data-submit-error class="mt-2 text-xs text-destructive">
          {{ submitError }}
        </p>
        <p
          v-if="successUnc"
          data-submit-success
          class="mt-2 text-xs text-emerald-600 dark:text-emerald-400"
        >
          {{ t("modal.shareWizard.createdPrefix") }}{{ successUnc }}
        </p>
      </div>
    </div>

    <template #footer>
      <button
        v-if="step > 1 && !successUnc"
        class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground"
        @click="back"
      >
        {{ t("common.back") }}
      </button>
      <button
        v-if="step < 4"
        data-next-btn
        :disabled="!canAdvanceFrom"
        class="rounded bg-secondary px-3 py-1 text-sm font-medium text-secondary-foreground hover:bg-secondary/80 disabled:opacity-50"
        @click="next"
      >
        {{ t("common.next") }}
      </button>
      <button
        v-if="step === 4 && !successUnc"
        data-create-btn
        :disabled="isSubmitting"
        class="rounded bg-primary px-3 py-1 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        @click="onCreate"
      >
        {{ isSubmitting ? t("common.creating") : t("common.create") }}
      </button>
      <button
        class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground"
        @click="emit('close')"
      >
        {{ successUnc ? t("common.done") : t("common.cancel") }}
      </button>
    </template>
  </BaseModal>
</template>
