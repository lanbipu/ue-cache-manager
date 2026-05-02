<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import { useCredentialsStore } from "@/stores/credentials";
import { useHealthCheckStore } from "@/stores/healthCheck";
import { useMachinesStore } from "@/stores/machines";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const machines = useMachinesStore();
const credentials = useCredentialsStore();
const health = useHealthCheckStore();
const selectedIds = ref<number[]>([]);
const credentialAlias = ref("");
const projectPathsText = ref("");

watch(() => props.open, (open) => {
  if (open) {
    machines.loadMachines();
    credentials.load();
    selectedIds.value = machines.machines.map((m) => m.id).filter((id): id is number => id != null);
  }
});

const canRun = computed(() => selectedIds.value.length > 0 && credentialAlias.value !== "" && !health.isRunning);
const projectPaths = computed(() => projectPathsText.value.split(/\r?\n/).map((p) => p.trim()).filter(Boolean));

function toggle(id: number, checked: boolean) {
  selectedIds.value = checked ? [...new Set([...selectedIds.value, id])] : selectedIds.value.filter((x) => x !== id);
}

async function run() {
  await health.run({ machine_ids: selectedIds.value, credential_alias: credentialAlias.value, project_paths: projectPaths.value });
  emit("close");
}
</script>

<template>
  <BaseModal :open="open" title="Run health check" size="lg" @close="emit('close')">
    <div data-health-check-wizard class="space-y-4">
      <section class="grid gap-2">
        <label v-for="machine in machines.machines" :key="machine.id ?? machine.ip" class="flex items-center gap-2 rounded-md border bg-card p-2 text-sm">
          <input type="checkbox" :checked="machine.id != null && selectedIds.includes(machine.id)" @change="machine.id != null && toggle(machine.id, ($event.target as HTMLInputElement).checked)" />
          <span class="font-mono">{{ machine.hostname }}</span>
          <span class="text-muted-foreground">{{ machine.ip }}</span>
        </label>
      </section>
      <label class="block text-sm">
        <span class="mb-1 block text-muted-foreground">Credential alias</span>
        <select v-model="credentialAlias" data-health-cred class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option value="" disabled>Select credential</option>
          <option v-for="cred in credentials.credentials" :key="cred.alias" :value="cred.alias">{{ cred.alias }}</option>
        </select>
      </label>
      <label class="block text-sm">
        <span class="mb-1 block text-muted-foreground">Project paths</span>
        <textarea v-model="projectPathsText" class="min-h-24 w-full rounded-md border bg-background p-3 font-mono text-sm" placeholder="E:\Project"></textarea>
      </label>
    </div>
    <template #footer>
      <Button data-run-health-check-btn :disabled="!canRun" @click="run">{{ health.isRunning ? "Running" : "Run full check" }}</Button>
      <Button variant="outline" @click="emit('close')">Close</Button>
    </template>
  </BaseModal>
</template>
