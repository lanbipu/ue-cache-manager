<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import { tauriApi, type IniKey, type UecmError } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machineId: number | null;
}>();
const emit = defineEmits<{ (e: "close"): void }>();

const { t } = useI18n();
const filePath = ref("");
const section = ref("");
const keyName = ref("");
const keyValue = ref("");
const loadedKeys = ref<IniKey[]>([]);
const lastBackup = ref<string | null>(null);
const reading = ref(false);
const applying = ref(false);
const error = ref<UecmError | null>(null);

watch(
  () => props.open,
  (val) => {
    if (val) {
      filePath.value = "";
      section.value = "";
      keyName.value = "";
      keyValue.value = "";
      loadedKeys.value = [];
      lastBackup.value = null;
      error.value = null;
    }
  },
);

async function onRead() {
  if (props.machineId === null || !filePath.value || !section.value) return;
  reading.value = true;
  error.value = null;
  try {
    loadedKeys.value = await tauriApi.readIniSection(props.machineId, filePath.value, section.value);
  } catch (e) {
    error.value = e as UecmError;
    loadedKeys.value = [];
  } finally {
    reading.value = false;
  }
}

async function onApply() {
  if (props.machineId === null || !filePath.value || !section.value || !keyName.value) return;
  applying.value = true;
  error.value = null;
  try {
    const result = await tauriApi.setIniKey(
      props.machineId,
      filePath.value,
      section.value,
      keyName.value,
      keyValue.value,
    );
    lastBackup.value = result.backup_path;
    // Re-read to confirm
    await onRead();
  } catch (e) {
    error.value = e as UecmError;
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <BaseModal :open="props.open" :title="t('modal.iniEdit.title')" @close="emit('close')">
    <div>
      <p class="text-xs text-muted-foreground mb-2">
        {{ t("modal.iniEdit.hint") }}
      </p>

      <label class="block text-sm mb-1">{{ t("modal.iniEdit.filePath") }}</label>
      <input
        data-ini-path
        v-model="filePath"
        placeholder="C:\\Path\\To\\Project\\Config\\DefaultEngine.ini"
        class="mb-2 w-full rounded border border-input bg-transparent px-2 py-1 font-mono text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />

      <label class="block text-sm mb-1">{{ t("modal.iniEdit.sectionLabel") }}</label>
      <input
        data-ini-section
        v-model="section"
        placeholder="Core.System"
        class="mb-2 w-full rounded border border-input bg-transparent px-2 py-1 font-mono text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />

      <button
        data-ini-read-btn
        :disabled="reading"
        class="mb-3 rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
        @click="onRead"
      >
        {{ reading ? t("common.reading") : t("modal.iniEdit.readSection") }}
      </button>

      <div v-if="loadedKeys.length > 0" class="mb-3">
        <p class="text-sm font-medium mb-1">{{ t("modal.iniEdit.existingKeys") }}</p>
        <table class="w-full text-xs border">
          <thead class="bg-muted/40 text-muted-foreground">
            <tr><th class="text-left px-2 py-1 font-medium">{{ t("modal.iniEdit.headerName") }}</th><th class="text-left px-2 py-1 font-medium">{{ t("modal.iniEdit.headerValue") }}</th></tr>
          </thead>
          <tbody>
            <tr v-for="k in loadedKeys" :key="k.name" data-ini-row class="border-t">
              <td class="px-2 py-1 font-mono">{{ k.name }}</td>
              <td class="px-2 py-1 font-mono break-all">{{ k.value }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <hr class="my-3 border-border" />

      <label class="block text-sm mb-1">{{ t("modal.iniEdit.keyName") }}</label>
      <input
        data-ini-key
        v-model="keyName"
        placeholder="DDCStrategy"
        class="mb-2 w-full rounded border border-input bg-transparent px-2 py-1 font-mono text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />

      <label class="block text-sm mb-1">{{ t("modal.iniEdit.newValue") }}</label>
      <input
        data-ini-value
        v-model="keyValue"
        placeholder="Filesystem"
        class="mb-2 w-full rounded border border-input bg-transparent px-2 py-1 font-mono text-sm text-foreground placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />

      <p v-if="lastBackup" class="text-xs text-emerald-600 dark:text-emerald-400">
        {{ t("modal.iniEdit.backupSavedPrefix") }}<span class="font-mono">{{ lastBackup }}</span>
      </p>
      <p v-if="error" class="text-xs text-destructive">{{ error.message }}</p>
    </div>
    <template #footer>
      <button class="rounded border border-input px-3 py-1 text-sm hover:bg-accent hover:text-accent-foreground" @click="emit('close')">
        {{ t("common.cancel") }}
      </button>
      <button
        data-ini-apply-btn
        :disabled="applying"
        class="rounded bg-primary px-3 py-1 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        @click="onApply"
      >
        {{ applying ? t("common.applying") : t("common.apply") }}
      </button>
    </template>
  </BaseModal>
</template>
