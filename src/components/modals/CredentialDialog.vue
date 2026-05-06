<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import { useCredentialsStore } from "@/stores/credentials";
import type { CredentialKind } from "@/services/tauri";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

const { t } = useI18n();
const store = useCredentialsStore();
const alias = ref("");
const kind = ref<CredentialKind>("winrm");
const username = ref("");
const password = ref("");

function normalizeUsernameForSave(value: string) {
  return value.trim().replace(/^\.\\/, "").replace(/^\.\//, "");
}

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
  const normalizedUsername = normalizeUsernameForSave(username.value);
  if (!alias.value || !normalizedUsername || !password.value) return;
  try {
    await store.save(alias.value, kind.value, normalizedUsername, password.value);
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
  <BaseModal :open="props.open" :title="t('modal.credential.title')" @close="emit('close')">
    <section>
      <h3 class="text-sm font-medium mb-2">{{ t("modal.credential.stored") }}</h3>
      <p v-if="store.credentials.length === 0" class="text-sm text-gray-500">
        {{ t("modal.credential.empty") }}
      </p>
      <table v-else class="w-full border-collapse text-sm">
        <thead class="bg-muted/40">
          <tr class="border-b">
            <th class="px-2 py-1 text-left font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("modal.credential.headerAlias") }}</th>
            <th class="px-2 py-1 text-left font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("modal.credential.headerKind") }}</th>
            <th class="px-2 py-1 text-left font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("modal.credential.headerUser") }}</th>
            <th class="px-2 py-1"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in store.credentials" :key="c.alias" data-cred-row class="border-b">
            <td class="px-2 py-1 font-mono text-xs">{{ c.alias }}</td>
            <td class="px-2 py-1">{{ c.kind }}</td>
            <td class="px-2 py-1">{{ c.username }}</td>
            <td class="px-2 py-1 text-right">
              <Button
                data-cred-delete-btn
                variant="ghost"
                size="sm"
                class="text-destructive hover:text-destructive"
                @click="onDelete(c.alias)"
              >
                {{ t("common.delete") }}
              </Button>
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="mt-6">
      <h3 class="text-sm font-medium mb-2">{{ t("modal.credential.addTitle") }}</h3>
      <div class="space-y-2">
        <Input
          data-cred-alias
          v-model="alias"
          :placeholder="t('modal.credential.aliasPlaceholder')"
          class="font-mono"
        />
        <select
          v-model="kind"
          class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50"
        >
          <option value="winrm">winrm</option>
          <option value="share">share</option>
        </select>
        <Input
          data-cred-username
          v-model="username"
          :placeholder="t('modal.credential.usernamePlaceholder')"
        />
        <Input
          data-cred-password
          v-model="password"
          type="password"
          :placeholder="t('modal.credential.passwordPlaceholder')"
        />
        <Button
          data-cred-save-btn
          class="w-full"
          :disabled="!alias || !username || !password"
          @click="onSave"
        >
          {{ t("common.save") }}
        </Button>
      </div>
      <p v-if="store.error" class="mt-2 text-xs text-destructive">
        {{ store.error.message }}
      </p>
    </section>
  </BaseModal>
</template>
