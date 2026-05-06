<script setup lang="ts">
import { computed, ref } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import UecmPathInput from "@/components/primitives/UecmPathInput.vue";
import { useDdcPakStore } from "@/stores/ddcPak";
import type { BackendChoice, CredentialRecord, Machine, ProjectLocation, ProjectSummary } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  projects: ProjectSummary[];
  locations: Record<number, ProjectLocation[]>;
  machines: Machine[];
  credentials: CredentialRecord[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "started"): void;
}>();

const store = useDdcPakStore();
const backend = ref<BackendChoice>("remote");
const projectId = ref<number | null>(null);
const sourceMachineId = ref<number | null>(null);
const localUprojectPath = ref("");
const localEnginePath = ref("");
const ueVersion = ref("5.4");
const credentialAlias = ref<string | null>(null);
const autoDistribute = ref(false);
const targetMachineIds = ref<number[]>([]);
const namedShareUnc = ref("");
const error = ref<string | null>(null);

const winrmCredentials = computed(() => props.credentials.filter((cred) => cred.kind === "winrm"));
const projectLocations = computed(() => (projectId.value ? props.locations[projectId.value] ?? [] : []));
const sourceCandidates = computed(() => {
  const ids = new Set(projectLocations.value.map((loc) => loc.machine_id));
  return props.machines.filter((machine) => machine.id != null && ids.has(machine.id));
});
const targetCandidates = computed(() =>
  sourceCandidates.value.filter((machine) => machine.id !== sourceMachineId.value),
);

async function run() {
  error.value = null;
  if (!projectId.value) {
    error.value = "Select a project";
    return;
  }
  if (backend.value === "remote" && !sourceMachineId.value) {
    error.value = "Select a source machine";
    return;
  }
  if (backend.value === "local" && (!localUprojectPath.value.trim() || !localEnginePath.value.trim())) {
    error.value = "Enter local project and engine paths";
    return;
  }
  if (autoDistribute.value && (!sourceMachineId.value || targetMachineIds.value.length === 0)) {
    error.value = "Select source inventory row and at least one target";
    return;
  }
  await store.startGenerate(
    {
      backend: backend.value,
      sourceMachineId: sourceMachineId.value,
      projectId: projectId.value,
      localUprojectPath: backend.value === "local" ? localUprojectPath.value.trim() : null,
      localEnginePath: backend.value === "local" ? localEnginePath.value.trim() : null,
      ueVersion: ueVersion.value.trim() || null,
      operatorCredentialAlias: credentialAlias.value,
    },
    autoDistribute.value && sourceMachineId.value
      ? {
          source_machine_id: sourceMachineId.value,
          target_machine_ids: targetMachineIds.value,
          named_share_unc: namedShareUnc.value.trim() || null,
          operator_credential_alias: credentialAlias.value,
          source_smb_credential_alias: null,
        }
      : null,
  );
  emit("started");
  emit("close");
}
</script>

<template>
  <BaseModal :open="open" title="Generate DDC Pak" size="xl" @close="emit('close')">
    <div data-ddc-pak-wizard class="space-y-5">
      <div class="grid gap-3 md:grid-cols-3">
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Project</span>
          <select v-model.number="projectId" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
            <option :value="null">Select project</option>
            <option v-for="project in projects" :key="project.id" :value="project.id">
              {{ project.uproject_name }}
            </option>
          </select>
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Backend</span>
          <select v-model="backend" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
            <option value="remote">Remote machine</option>
            <option value="local">Self</option>
          </select>
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">UE version</span>
          <input v-model="ueVersion" class="h-9 w-full rounded-md border bg-background px-3 text-sm" placeholder="5.4" />
        </label>
      </div>

      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Source machine</span>
        <select v-model.number="sourceMachineId" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option :value="null">Select source</option>
          <option v-for="machine in sourceCandidates" :key="machine.id ?? machine.ip" :value="machine.id">
            {{ machine.hostname }} · {{ machine.ip }}
          </option>
        </select>
      </label>

      <div v-if="backend === 'local'" class="grid gap-3 md:grid-cols-2">
        <UecmPathInput v-model="localUprojectPath" label="Local .uproject path" placeholder="D:\\Work\\Demo\\Demo.uproject" />
        <UecmPathInput v-model="localEnginePath" label="Local engine root" placeholder="C:\\Program Files\\Epic Games\\UE_5.4" />
      </div>

      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Credential</span>
        <select v-model="credentialAlias" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
          <option :value="null">Current session</option>
          <option v-for="cred in winrmCredentials" :key="cred.alias" :value="cred.alias">
            {{ cred.alias }}
          </option>
        </select>
      </label>

      <section class="space-y-3 border-t pt-4">
        <label class="flex items-center gap-2 text-sm font-bold">
          <input v-model="autoDistribute" type="checkbox" />
          Auto-distribute after verify
        </label>
        <div v-if="autoDistribute" class="space-y-3">
          <input v-model="namedShareUnc" class="h-9 w-full rounded-md border bg-background px-3 text-sm" placeholder="Optional named share UNC" />
          <div class="grid gap-2 md:grid-cols-2">
            <label v-for="machine in targetCandidates" :key="machine.id ?? machine.ip" class="flex items-center gap-2 rounded-md border p-2 text-sm">
              <input v-model="targetMachineIds" type="checkbox" :value="machine.id" />
              <span>{{ machine.hostname }}</span>
            </label>
          </div>
        </div>
      </section>

      <p v-if="error || store.error" class="text-sm text-status-critical">{{ error ?? store.error?.message }}</p>
    </div>

    <template #footer>
      <Button variant="outline" @click="emit('close')">Cancel</Button>
      <Button data-run-ddc-pak @click="run">Run</Button>
    </template>
  </BaseModal>
</template>
