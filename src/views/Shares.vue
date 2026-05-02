<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useSharesStore } from "@/stores/shares";
import { useMachinesStore } from "@/stores/machines";

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
    localError.value = (e as { message?: string }).message ?? "delete failed";
  } finally {
    isDeleting.value = false;
  }
}
</script>

<template>
  <div class="h-full space-y-6 overflow-auto p-6">
    <header class="flex items-center justify-between gap-4">
      <div>
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">Shared DDC</p>
        <h1 class="mt-1 font-display text-3xl font-extrabold">SMB shares</h1>
      </div>
      <span data-share-count class="rounded-full border bg-card px-3 py-1 text-xs text-muted-foreground">
        {{ shares.shares.length }} share{{ shares.shares.length === 1 ? "" : "s" }}
      </span>
    </header>

    <p v-if="shares.isLoading" class="text-sm text-muted-foreground">Loading...</p>
    <p
      v-else-if="shares.shares.length === 0"
      data-shares-empty
      class="rounded-lg border bg-card p-6 text-sm text-muted-foreground"
    >
      No shares yet. Create one from the Machines view (Share button).
    </p>

    <table v-else class="w-full overflow-hidden rounded-lg border bg-card text-sm">
      <thead class="bg-muted text-muted-foreground">
        <tr>
          <th class="text-left px-3 py-1">Host</th>
          <th class="text-left px-3 py-1">Share</th>
          <th class="text-left px-3 py-1">UNC</th>
          <th class="text-left px-3 py-1">Mode</th>
          <th class="text-left px-3 py-1">Credential</th>
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
              Delete
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
      <p class="text-sm mb-2">Confirm delete share #{{ pendingDeleteId }}?</p>
      <label class="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
        <input
          data-also-remove-remote
          type="checkbox"
          v-model="alsoRemoveRemote"
        />
        Also remove the share on the remote host (requires credentials)
      </label>
      <p v-if="localError" class="mb-2 text-xs text-destructive">{{ localError }}</p>
      <div class="flex gap-2">
        <button
          data-confirm-delete
          :disabled="isDeleting"
          class="rounded-md bg-destructive px-3 py-1 text-sm text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
          @click="confirmDelete"
        >
          {{ isDeleting ? "Deleting..." : "Delete" }}
        </button>
        <button
          class="rounded-md border px-3 py-1 text-sm hover:bg-accent"
          @click="cancelDelete"
        >
          Cancel
        </button>
      </div>
    </div>

    <p v-if="shares.error" class="mt-3 text-xs text-destructive">
      {{ shares.error.message }}
    </p>
  </div>
</template>
