<script setup lang="ts">
import { ref } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import UecmPathInput from "@/components/primitives/UecmPathInput.vue";
import { useProjectsStore } from "@/stores/projects";
import type { Machine, ProjectSummary } from "@/services/tauri";

defineProps<{
  open: boolean;
  projects: ProjectSummary[];
  machines: Machine[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "saved"): void;
}>();

const store = useProjectsStore();
const selectedProjectId = ref<number | null>(null);
const selectedMachineId = ref<number | null>(null);
const uprojectName = ref("");
const displayName = ref("");
const absPath = ref("");
const uprojectPath = ref("");
const error = ref<string | null>(null);

async function save() {
  error.value = null;
  if (!selectedMachineId.value || !absPath.value.trim() || !uprojectPath.value.trim()) {
    error.value = "Complete machine and path fields";
    return;
  }
  let projectId = selectedProjectId.value;
  if (!projectId) {
    if (!uprojectName.value.trim()) {
      error.value = "Enter a .uproject filename";
      return;
    }
    projectId = await store.createManual(uprojectName.value.trim(), displayName.value.trim() || null);
  }
  await store.setLocation(
    projectId,
    selectedMachineId.value,
    absPath.value.trim(),
    uprojectPath.value.trim(),
    true,
  );
  emit("saved");
  emit("close");
}
</script>

<template>
  <BaseModal :open="open" title="Map project location" size="lg" @close="emit('close')">
    <div data-project-matching-modal class="space-y-4">
      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Logical project</span>
        <select v-model.number="selectedProjectId" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option :value="null">Create new project</option>
          <option v-for="project in projects" :key="project.id" :value="project.id">
            {{ project.uproject_name }}
          </option>
        </select>
      </label>

      <div v-if="!selectedProjectId" class="grid gap-3 md:grid-cols-2">
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Uproject file</span>
          <input v-model="uprojectName" class="h-9 w-full rounded-md border bg-background px-3 text-sm" placeholder="Demo.uproject" />
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Display name</span>
          <input v-model="displayName" class="h-9 w-full rounded-md border bg-background px-3 text-sm" placeholder="Demo" />
        </label>
      </div>

      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Machine</span>
        <select v-model.number="selectedMachineId" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option :value="null">Select machine</option>
          <option v-for="machine in machines" :key="machine.id ?? machine.ip" :value="machine.id">
            {{ machine.hostname }}
          </option>
        </select>
      </label>

      <UecmPathInput v-model="absPath" label="Project root" />
      <UecmPathInput v-model="uprojectPath" label=".uproject path" placeholder="D:\\Work\\Demo\\Demo.uproject" />
      <p v-if="error || store.error" class="text-sm text-status-critical">{{ error ?? store.error?.message }}</p>
    </div>

    <template #footer>
      <Button variant="outline" @click="emit('close')">Cancel</Button>
      <Button data-save-project-mapping @click="save">Save mapping</Button>
    </template>
  </BaseModal>
</template>
