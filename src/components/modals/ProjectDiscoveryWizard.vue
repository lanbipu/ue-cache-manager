<script setup lang="ts">
import { computed, ref } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import { useProjectsStore } from "@/stores/projects";
import type { CredentialRecord, Machine } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  machines: Machine[];
  credentials: CredentialRecord[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "discovered"): void;
}>();

const projects = useProjectsStore();
const machineId = ref<number | null>(null);
const rootsText = ref("D:\\Work\nE:\\Projects");
const credentialAlias = ref<string | null>(null);
const isRunning = ref(false);
const localError = ref<string | null>(null);

const winrmCredentials = computed(() => props.credentials.filter((cred) => cred.kind === "winrm"));
const roots = computed(() =>
  rootsText.value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean),
);

async function run() {
  if (!machineId.value) {
    localError.value = "Select a machine";
    return;
  }
  if (roots.value.length === 0) {
    localError.value = "Enter at least one search root";
    return;
  }
  localError.value = null;
  isRunning.value = true;
  try {
    await projects.discover(machineId.value, roots.value, credentialAlias.value);
    emit("discovered");
    emit("close");
  } finally {
    isRunning.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Discover projects" size="lg" @close="emit('close')">
    <div data-project-discovery-wizard class="space-y-4">
      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Machine</span>
        <select v-model.number="machineId" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option :value="null">Select machine</option>
          <option v-for="machine in machines" :key="machine.id ?? machine.ip" :value="machine.id">
            {{ machine.hostname }} · {{ machine.ip }}
          </option>
        </select>
      </label>

      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Search roots</span>
        <textarea
          v-model="rootsText"
          class="min-h-24 w-full rounded-md border bg-background px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        ></textarea>
      </label>

      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Credential</span>
        <select v-model="credentialAlias" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option :value="null">Current session</option>
          <option v-for="cred in winrmCredentials" :key="cred.alias" :value="cred.alias">
            {{ cred.alias }}
          </option>
        </select>
      </label>

      <p v-if="localError || projects.error" class="text-sm text-status-critical">
        {{ localError ?? projects.error?.message }}
      </p>
    </div>

    <template #footer>
      <Button variant="outline" @click="emit('close')">Cancel</Button>
      <Button data-run-discovery @click="run" :disabled="isRunning">
        {{ isRunning ? "Running" : "Run discovery" }}
      </Button>
    </template>
  </BaseModal>
</template>
