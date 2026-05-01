<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useMachinesStore } from "@/stores/machines";

const store = useMachinesStore();
const newHostname = ref("");
const newIp = ref("");

onMounted(() => {
  store.loadMachines();
});

async function onAdd() {
  if (!newHostname.value || !newIp.value) return;
  await store.addMachine(newHostname.value, newIp.value);
  newHostname.value = "";
  newIp.value = "";
}

async function onDelete(id: number | null) {
  if (id === null) return;
  await store.deleteMachine(id);
}
</script>

<template>
  <div class="p-6">
    <h1 class="text-2xl font-semibold">Machines</h1>
    <p class="mt-1 text-sm text-gray-500">
      Plan 1 demonstration view. Real discovery + detail come in Plan 2.
    </p>

    <section class="mt-6 border-t pt-4">
      <h2 class="font-medium mb-2">Add a machine manually</h2>
      <form class="flex gap-2 items-center" @submit.prevent="onAdd">
        <input
          data-input-hostname
          v-model="newHostname"
          placeholder="hostname (e.g. RENDER-01)"
          class="border rounded px-2 py-1 text-sm"
        />
        <input
          data-input-ip
          v-model="newIp"
          placeholder="ip (e.g. 192.168.10.21)"
          class="border rounded px-2 py-1 text-sm"
        />
        <button
          data-add-btn
          type="button"
          @click="onAdd"
          class="px-3 py-1 bg-gray-200 rounded text-sm hover:bg-gray-300"
        >
          Add
        </button>
      </form>
    </section>

    <section class="mt-6">
      <h2 class="font-medium mb-2">Machine list</h2>

      <p v-if="store.isLoading" class="text-sm text-gray-500">Loading...</p>
      <p v-else-if="store.machines.length === 0" class="text-sm text-gray-500">
        No machines yet. Add one above.
      </p>
      <table v-else class="w-full text-sm border">
        <thead class="bg-gray-50">
          <tr>
            <th class="text-left px-3 py-2">Hostname</th>
            <th class="text-left px-3 py-2">IP</th>
            <th class="text-left px-3 py-2">Role</th>
            <th class="text-left px-3 py-2">Status</th>
            <th class="px-3 py-2"></th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="m in store.machines"
            :key="m.id ?? m.ip"
            data-machine-row
            class="border-t"
          >
            <td class="px-3 py-2">{{ m.hostname }}</td>
            <td class="px-3 py-2">{{ m.ip }}</td>
            <td class="px-3 py-2">{{ m.role }}</td>
            <td class="px-3 py-2">{{ m.status }}</td>
            <td class="px-3 py-2 text-right">
              <button
                @click="onDelete(m.id)"
                class="text-xs text-red-600 hover:underline"
              >
                Delete
              </button>
            </td>
          </tr>
        </tbody>
      </table>

      <p v-if="store.error" class="mt-3 text-sm text-red-600">
        Error: {{ store.error.message }}
      </p>
    </section>
  </div>
</template>
