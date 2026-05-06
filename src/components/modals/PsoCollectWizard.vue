<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import { useCredentialsStore } from "@/stores/credentials";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { usePsoStore } from "@/stores/pso";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  (e: "close"): void;
  (e: "started"): void;
}>();

const machines = useMachinesStore();
const projects = useProjectsStore();
const credentials = useCredentialsStore();
const pso = usePsoStore();

const step = ref<1 | 2 | 3 | 4>(1);
const sourceMachineId = ref<number | null>(null);
const projectId = ref<number | null>(null);
const resolutionW = ref(1920);
const resolutionH = ref(1080);
const windowed = ref(true);
const maxMinutes = ref(10);
const credentialAlias = ref<string | null>(null);
const errorMessage = ref<string | null>(null);
const isSubmitting = ref(false);

const sourceMachines = computed(() =>
  machines.machines.filter((machine) => machine.id != null),
);
const winrmCredentials = computed(() =>
  credentials.credentials.filter((credential) => credential.kind === "winrm"),
);
const selectedMachine = computed(() =>
  sourceMachines.value.find((machine) => machine.id === sourceMachineId.value),
);
const selectedProject = computed(() =>
  projects.projects.find((project) => project.id === projectId.value),
);

const canAdvance = computed(() => {
  if (step.value === 1) return sourceMachineId.value != null;
  if (step.value === 2) return projectId.value != null;
  if (step.value === 3) {
    return resolutionW.value > 0 && resolutionH.value > 0 && maxMinutes.value > 0;
  }
  return !isSubmitting.value;
});

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    step.value = 1;
    sourceMachineId.value = null;
    projectId.value = null;
    resolutionW.value = 1920;
    resolutionH.value = 1080;
    windowed.value = true;
    maxMinutes.value = 10;
    credentialAlias.value = null;
    errorMessage.value = null;
    void Promise.all([machines.loadMachines(), projects.load(), credentials.load()]);
  },
  { immediate: true },
);

function nextStep() {
  if (!canAdvance.value || step.value === 4) return;
  step.value = (step.value + 1) as 1 | 2 | 3 | 4;
}

function previousStep() {
  if (step.value === 1) return;
  step.value = (step.value - 1) as 1 | 2 | 3 | 4;
}

async function run() {
  if (sourceMachineId.value == null || projectId.value == null) return;
  isSubmitting.value = true;
  errorMessage.value = null;
  try {
    await pso.startCollection({
      sourceMachineId: sourceMachineId.value,
      projectId: projectId.value,
      ueVersion: null,
      resolutionW: resolutionW.value,
      resolutionH: resolutionH.value,
      windowed: windowed.value,
      maxMinutes: maxMinutes.value,
      operatorCredentialAlias: credentialAlias.value,
    });
    emit("started");
    emit("close");
  } catch (e) {
    errorMessage.value = (e as { message?: string }).message ?? "submit failed";
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <BaseModal :open="open" title="Collect PSO Cache" size="lg" @close="emit('close')">
    <div data-pso-collect-wizard class="space-y-4">
      <div class="flex flex-wrap items-center gap-2 text-xs">
        <span :class="step >= 1 ? 'text-primary' : 'text-muted-foreground'">1. Source</span>
        <span class="text-muted-foreground">/</span>
        <span :class="step >= 2 ? 'text-primary' : 'text-muted-foreground'">2. Project</span>
        <span class="text-muted-foreground">/</span>
        <span :class="step >= 3 ? 'text-primary' : 'text-muted-foreground'">3. Options</span>
        <span class="text-muted-foreground">/</span>
        <span :class="step >= 4 ? 'text-primary' : 'text-muted-foreground'">4. Review</span>
      </div>

      <section v-if="step === 1" class="space-y-2">
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Source machine</span>
          <select
            v-model.number="sourceMachineId"
            data-pso-source-select
            class="h-9 w-full rounded-md border bg-background px-3 text-sm"
          >
            <option :value="null">Select source</option>
            <option v-for="machine in sourceMachines" :key="machine.id ?? machine.ip" :value="machine.id">
              {{ machine.hostname }} · {{ machine.ip }}
            </option>
          </select>
        </label>
        <p class="text-xs text-muted-foreground">PSO collection must run on a machine with a real display GPU.</p>
      </section>

      <section v-else-if="step === 2" class="space-y-2">
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Project</span>
          <select
            v-model.number="projectId"
            data-pso-project-select
            class="h-9 w-full rounded-md border bg-background px-3 text-sm"
          >
            <option :value="null">Select project</option>
            <option v-for="project in projects.projects" :key="project.id" :value="project.id">
              {{ project.uproject_name }}
            </option>
          </select>
        </label>
      </section>

      <section v-else-if="step === 3" class="space-y-3">
        <div class="grid gap-3 md:grid-cols-2">
          <label class="block space-y-1">
            <span class="text-xs font-bold text-muted-foreground">Width</span>
            <input
              v-model.number="resolutionW"
              data-pso-resw
              type="number"
              min="1"
              class="h-9 w-full rounded-md border bg-background px-3 text-sm"
            />
          </label>
          <label class="block space-y-1">
            <span class="text-xs font-bold text-muted-foreground">Height</span>
            <input
              v-model.number="resolutionH"
              data-pso-resh
              type="number"
              min="1"
              class="h-9 w-full rounded-md border bg-background px-3 text-sm"
            />
          </label>
        </div>
        <label class="flex items-center gap-2 text-sm">
          <input v-model="windowed" type="checkbox" />
          <span>Windowed mode</span>
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Max minutes</span>
          <input
            v-model.number="maxMinutes"
            data-pso-max-min
            type="number"
            min="1"
            class="h-9 w-full rounded-md border bg-background px-3 text-sm"
          />
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Credential</span>
          <select
            v-model="credentialAlias"
            data-pso-cred-select
            class="h-9 w-full rounded-md border bg-background px-3 text-sm"
          >
            <option :value="null">Current session</option>
            <option v-for="credential in winrmCredentials" :key="credential.alias" :value="credential.alias">
              {{ credential.alias }}
            </option>
          </select>
        </label>
      </section>

      <section v-else class="space-y-3">
        <div class="rounded-md border bg-muted/40 p-3 text-sm">
          <div class="font-mono text-xs text-muted-foreground">Source</div>
          <p>{{ selectedMachine?.hostname ?? sourceMachineId }}</p>
          <div class="mt-2 font-mono text-xs text-muted-foreground">Project</div>
          <p>{{ selectedProject?.uproject_name ?? projectId }}</p>
          <div class="mt-2 font-mono text-xs text-muted-foreground">Run</div>
          <p>{{ resolutionW }}x{{ resolutionH }} / {{ windowed ? "windowed" : "fullscreen" }} / {{ maxMinutes }} min</p>
        </div>
        <p v-if="errorMessage || pso.error" class="text-sm text-status-critical">
          {{ errorMessage ?? pso.error?.message }}
        </p>
      </section>
    </div>

    <template #footer>
      <Button v-if="step > 1" variant="outline" @click="previousStep">Back</Button>
      <Button v-if="step < 4" data-pso-wizard-next :disabled="!canAdvance" @click="nextStep">
        Next
      </Button>
      <Button v-else data-pso-wizard-run :disabled="!canAdvance" @click="run">
        {{ isSubmitting ? "Starting" : "Run" }}
      </Button>
    </template>
  </BaseModal>
</template>
