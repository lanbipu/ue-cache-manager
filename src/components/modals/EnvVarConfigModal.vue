<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import { tauriApi, type UecmError } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machineId: number | null;
  varName: string;
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const { t } = useI18n();
const currentValue = ref<string | null>(null);
const newValue = ref("");
const loading = ref(false);
const applying = ref(false);
const error = ref<UecmError | null>(null);
const applied = ref(false);

watch(
  () => [props.open, props.machineId],
  async ([open, id]) => {
    if (!open || id === null) return;
    loading.value = true;
    applied.value = false;
    error.value = null;
    try {
      currentValue.value = await tauriApi.getMachineEnvVar(id as number, props.varName);
      newValue.value = currentValue.value ?? "";
    } catch (e) {
      error.value = e as UecmError;
      currentValue.value = null;
    } finally {
      loading.value = false;
    }
  },
);

async function onApply() {
  if (props.machineId === null) return;
  applying.value = true;
  error.value = null;
  try {
    await tauriApi.setMachineEnvVar(props.machineId, props.varName, newValue.value);
    currentValue.value = newValue.value;
    applied.value = true;
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="t('modal.envVar.title', { name: props.varName })" @close="emit('close')">
    <div>
      <p class="text-xs text-muted-foreground mb-2">
        {{ t("modal.envVar.hint") }}
      </p>

      <p class="text-sm mb-1">{{ t("modal.envVar.currentValue") }}</p>
      <p data-current-value class="mb-3 break-all rounded border border-border bg-muted/40 px-2 py-1 font-mono text-sm">
        <span v-if="loading">{{ t("common.loading") }}</span>
        <span v-else>{{ currentValue ?? t("common.notSet") }}</span>
      </p>

      <p class="text-sm mb-1">{{ t("modal.envVar.newValue") }}</p>
      <input
        data-new-value
        v-model="newValue"
        placeholder="\\HOST\DDC"
        class="w-full rounded border border-input bg-transparent px-2 py-1 font-mono text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />

      <p v-if="error" class="mt-2 text-xs text-destructive">{{ error.message }}</p>
      <p v-if="applied" class="mt-2 text-xs text-emerald-600 dark:text-emerald-400">{{ t("modal.envVar.appliedVerified") }}</p>
    </div>
    <template #footer>
      <button class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground" @click="emit('close')">
        {{ t("common.cancel") }}
      </button>
      <button
        data-apply-btn
        :disabled="applying || loading"
        class="rounded bg-primary px-3 py-1 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        @click="onApply"
      >
        {{ applying ? t("common.applying") : t("common.apply") }}
      </button>
    </template>
  </BaseModal>
</template>
