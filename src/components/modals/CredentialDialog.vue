<script setup lang="ts">
import { ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import { useCredentialsStore } from "@/stores/credentials";
import type { CredentialKind } from "@/services/tauri";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const store = useCredentialsStore();
const alias = ref("");
const kind = ref<CredentialKind>("winrm");
const username = ref("");
const password = ref("");

watch(() => props.open, async (val) => {
  if (val) {
    await store.load();
    alias.value = "";
    kind.value = "winrm";
    username.value = "";
    password.value = "";
  }
});

async function onSave() {
  if (!alias.value || !username.value || !password.value) return;
  try {
    await store.save(alias.value, kind.value, username.value, password.value);
    alias.value = "";
    username.value = "";
    password.value = "";
  } catch {
    /* error captured in store */
  }
}

async function onDelete(a: string) {
  await store.remove(a);
}
</script>

<template>
  <BaseModal :open="props.open" title="Credentials" @close="emit('close')">
    <section>
      <h3 class="text-sm font-medium mb-2">Stored credentials</h3>
      <p v-if="store.credentials.length === 0" class="text-sm text-gray-500">
        No credentials saved yet.
      </p>
      <table v-else class="w-full text-sm border">
        <thead class="bg-gray-50">
          <tr>
            <th class="text-left px-2 py-1">Alias</th>
            <th class="text-left px-2 py-1">Kind</th>
            <th class="text-left px-2 py-1">User</th>
            <th class="px-2 py-1"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in store.credentials" :key="c.alias" data-cred-row class="border-t">
            <td class="px-2 py-1 font-mono text-xs">{{ c.alias }}</td>
            <td class="px-2 py-1">{{ c.kind }}</td>
            <td class="px-2 py-1">{{ c.username }}</td>
            <td class="px-2 py-1 text-right">
              <button
                data-cred-delete-btn
                class="text-xs text-red-600 hover:underline"
                @click="onDelete(c.alias)"
              >
                Delete
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="mt-6">
      <h3 class="text-sm font-medium mb-2">Add credential</h3>
      <div class="space-y-2">
        <input
          data-cred-alias
          v-model="alias"
          placeholder="alias (e.g. UECM:winrm:RENDER-01)"
          class="w-full border rounded px-2 py-1 text-sm font-mono"
        />
        <select v-model="kind" class="w-full border rounded px-2 py-1 text-sm">
          <option value="winrm">winrm</option>
          <option value="share">share</option>
        </select>
        <input
          data-cred-username
          v-model="username"
          placeholder="username"
          class="w-full border rounded px-2 py-1 text-sm"
        />
        <input
          data-cred-password
          v-model="password"
          type="password"
          placeholder="password (write-only, never displayed)"
          class="w-full border rounded px-2 py-1 text-sm"
        />
        <button
          data-cred-save-btn
          class="w-full px-3 py-1 text-sm bg-gray-200 rounded hover:bg-gray-300"
          @click="onSave"
        >
          Save
        </button>
      </div>
      <p v-if="store.error" class="mt-2 text-xs text-red-600">
        {{ store.error.message }}
      </p>
    </section>
  </BaseModal>
</template>
