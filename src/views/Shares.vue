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
  <div class="p-6 h-full overflow-auto">
    <header class="flex items-center justify-between mb-4">
      <h1 class="text-lg font-semibold">SMB shares</h1>
      <span data-share-count class="text-xs text-gray-500">
        {{ shares.shares.length }} share{{ shares.shares.length === 1 ? "" : "s" }}
      </span>
    </header>

    <p v-if="shares.isLoading" class="text-sm text-gray-500">Loading...</p>
    <p
      v-else-if="shares.shares.length === 0"
      data-shares-empty
      class="text-sm text-gray-500"
    >
      No shares yet. Create one from the Machines view (Share button).
    </p>

    <table v-else class="w-full text-sm border">
      <thead class="bg-gray-50">
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
            <span
              class="text-xs px-2 py-0.5 rounded-full"
              :class="s.mode === 'managed' ? 'bg-blue-100 text-blue-800' : 'bg-gray-100 text-gray-800'"
            >
              {{ s.mode }}
            </span>
          </td>
          <td class="px-3 py-1 text-xs text-gray-600">
            {{ s.credential_alias ?? "—" }}
          </td>
          <td class="px-3 py-1 text-right">
            <button
              data-share-delete-btn
              class="text-xs text-red-600 hover:underline"
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
      class="mt-4 p-3 border border-red-300 rounded bg-red-50"
    >
      <p class="text-sm mb-2">Confirm delete share #{{ pendingDeleteId }}?</p>
      <label class="flex items-center gap-2 text-xs text-gray-700 mb-2">
        <input
          data-also-remove-remote
          type="checkbox"
          v-model="alsoRemoveRemote"
        />
        Also remove the share on the remote host (requires credentials)
      </label>
      <p v-if="localError" class="text-xs text-red-600 mb-2">{{ localError }}</p>
      <div class="flex gap-2">
        <button
          data-confirm-delete
          :disabled="isDeleting"
          class="px-3 py-1 text-sm bg-red-600 text-white rounded hover:bg-red-700 disabled:opacity-50"
          @click="confirmDelete"
        >
          {{ isDeleting ? "Deleting..." : "Delete" }}
        </button>
        <button
          class="px-3 py-1 text-sm border rounded hover:bg-gray-100"
          @click="cancelDelete"
        >
          Cancel
        </button>
      </div>
    </div>

    <p v-if="shares.error" class="mt-3 text-xs text-red-600">
      {{ shares.error.message }}
    </p>
  </div>
</template>
