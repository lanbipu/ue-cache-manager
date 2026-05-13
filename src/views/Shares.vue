<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSharesStore } from "@/stores/shares";
import { useMachinesStore } from "@/stores/machines";

const { t } = useI18n();
const shares = useSharesStore();
const machines = useMachinesStore();

const pendingDeleteId = ref<number | null>(null);
const alsoRemoveRemote = ref(false);
const isDeleting = ref(false);
const localError = ref<string | null>(null);

onMounted(async () => {
  await Promise.all([shares.load(), machines.loadMachines()]);
});

const hostnameById = computed(() => {
  const map = new Map<number, string>();
  for (const m of machines.machines) {
    if (m.id != null) map.set(m.id, m.hostname);
  }
  return map;
});

const shareCountLabel = computed(() => {
  const count = shares.shares.length;
  return count === 1 ? t("shares.countOne", { count }) : t("shares.countMany", { count });
});

function startDelete(id: number) {
  pendingDeleteId.value = id;
  alsoRemoveRemote.value = false;
  localError.value = null;
}

function cancelDelete() {
  pendingDeleteId.value = null;
}

async function confirmDelete() {
  if (pendingDeleteId.value == null) return;
  isDeleting.value = true;
  try {
    await shares.remove(pendingDeleteId.value, alsoRemoveRemote.value);
    pendingDeleteId.value = null;
  } catch (e) {
    localError.value = (e as { message?: string }).message ?? t("shares.deleteFailed");
  } finally {
    isDeleting.value = false;
  }
}
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">{{ t("shares.eyebrow") }}</p>
        <h1 class="mt-1 font-display text-3xl font-extrabold">{{ t("shares.title") }}</h1>
      </div>
      <span data-share-count class="rounded-full border bg-card px-3 py-1 text-xs text-muted-foreground">
        {{ shareCountLabel }}
      </span>
    </header>

    <p v-if="shares.isLoading" class="text-sm text-muted-foreground">{{ t("common.loading") }}</p>
    <p
      v-else-if="shares.shares.length === 0"
      data-shares-empty
      class="rounded-lg border bg-card p-6 text-sm text-muted-foreground"
    >
      {{ t("shares.empty") }}
    </p>

    <table v-else class="w-full overflow-hidden rounded-lg border bg-card text-sm">
      <thead class="bg-muted text-muted-foreground">
        <tr>
          <th class="text-left px-3 py-1">{{ t("shares.headerHost") }}</th>
          <th class="text-left px-3 py-1">{{ t("shares.headerShare") }}</th>
          <th class="text-left px-3 py-1">{{ t("shares.headerUnc") }}</th>
          <th class="text-left px-3 py-1">{{ t("shares.headerMode") }}</th>
          <th class="text-left px-3 py-1">{{ t("shares.headerCredential") }}</th>
          <th class="px-3 py-1"></th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="s in shares.shares"
          :key="s.id ?? s.unc_path"
          data-share-row
          class="border-t"
        >
          <td class="px-3 py-1">
            {{ hostnameById.get(s.host_machine_id) ?? `#${s.host_machine_id}` }}
          </td>
          <td class="px-3 py-1 font-medium">{{ s.share_name }}</td>
          <td class="px-3 py-1 font-mono text-xs">{{ s.unc_path }}</td>
          <td class="px-3 py-1">
            <span class="rounded-full border px-2 py-0.5 text-xs" :class="s.mode === 'managed' ? 'border-primary/30 bg-primary/10 text-primary' : 'border-muted-foreground/20 bg-muted text-muted-foreground'">
              {{ s.mode }}
            </span>
          </td>
          <td class="px-3 py-1 text-xs text-muted-foreground">
            {{ s.credential_alias ?? "—" }}
          </td>
          <td class="px-3 py-1 text-right">
            <button
              data-share-delete-btn
              class="text-xs text-destructive hover:underline"
              @click="startDelete(s.id!)"
            >
              {{ t("common.delete") }}
            </button>
          </td>
        </tr>
      </tbody>
    </table>

    <div
      v-if="pendingDeleteId !== null"
      data-delete-confirm
      class="rounded-lg border border-destructive/30 bg-destructive/10 p-4"
    >
      <p class="text-sm mb-2">{{ t("shares.confirmDeletePrefix", { id: pendingDeleteId }) }}</p>
      <label class="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
        <input
          data-also-remove-remote
          type="checkbox"
          v-model="alsoRemoveRemote"
        />
        {{ t("shares.alsoRemoveRemote") }}
      </label>
      <p v-if="localError" class="mb-2 text-xs text-destructive">{{ localError }}</p>
      <div class="flex gap-2">
        <button
          data-confirm-delete
          :disabled="isDeleting"
          class="rounded-md bg-destructive px-3 py-1 text-sm text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
          @click="confirmDelete"
        >
          {{ isDeleting ? t("common.deleting") : t("common.delete") }}
        </button>
        <button
          class="rounded-md border px-3 py-1 text-sm hover:bg-accent"
          @click="cancelDelete"
        >
          {{ t("common.cancel") }}
        </button>
      </div>
    </div>

    <p v-if="shares.error" class="mt-3 text-xs text-destructive">
      {{ shares.error.message }}
    </p>
  </div>
</template>
