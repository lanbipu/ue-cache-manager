<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import { useCredentialsStore } from "@/stores/credentials";
import { useGpuConsistencyStore } from "@/stores/gpuConsistency";
import { useMachinesStore } from "@/stores/machines";
import { usePsoStore } from "@/stores/pso";
import type { PsoCacheFile } from "@/services/tauri";

const props = defineProps<{
  open: boolean;
  file: PsoCacheFile | null;
}>();
const emit = defineEmits<{
  (e: "close"): void;
  (e: "started"): void;
}>();

const machines = useMachinesStore();
const credentials = useCredentialsStore();
const gpu = useGpuConsistencyStore();
const pso = usePsoStore();

const targetIds = ref<number[]>([]);
const namedShareUnc = ref("");
const operatorCredentialAlias = ref<string | null>(null);
const sourceSmbCredentialAlias = ref<string | null>(null);
const forceGpuMismatch = ref(false);
const errorMessage = ref<string | null>(null);
const isSubmitting = ref(false);

const winrmCredentials = computed(() =>
  credentials.credentials.filter((credential) => credential.kind === "winrm"),
);
const shareCredentials = computed(() =>
  credentials.credentials.filter((credential) => credential.kind === "share"),
);

const machineGpuMap = computed(() => {
  const out = new Map<number, string | null>();
  for (const cell of gpu.matrix?.cells ?? []) {
    out.set(
      cell.machine_id,
      cell.signature
        ? `${cell.signature.vendor}:${cell.signature.model}:${cell.signature.driver}`
        : null,
    );
  }
  return out;
});

const candidateMachines = computed(() => {
  if (!props.file) return [];
  return machines.machines
    .filter((machine) => machine.id != null && machine.id !== props.file?.source_machine_id)
    .map((machine) => {
      const signature = machineGpuMap.value.get(machine.id ?? 0) ?? null;
      return {
        id: machine.id as number,
        hostname: machine.hostname,
        ip: machine.ip,
        signature,
        matches: signature === props.file?.gpu_signature,
      };
    });
});

const hasMismatch = computed(() =>
  targetIds.value.some((id) => {
    const target = candidateMachines.value.find((candidate) => candidate.id === id);
    return target != null && !target.matches;
  }),
);

const canSubmit = computed(
  () =>
    props.file?.id != null &&
    targetIds.value.length > 0 &&
    !isSubmitting.value &&
    (!hasMismatch.value || forceGpuMismatch.value),
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    targetIds.value = [];
    namedShareUnc.value = "";
    operatorCredentialAlias.value = null;
    sourceSmbCredentialAlias.value = null;
    forceGpuMismatch.value = false;
    errorMessage.value = null;
    void Promise.all([machines.loadMachines(), credentials.load(), gpu.load()]);
  },
  { immediate: true },
);

async function run() {
  if (props.file?.id == null) return;
  isSubmitting.value = true;
  errorMessage.value = null;
  try {
    await pso.startDistribute({
      fileId: props.file.id,
      targetMachineIds: targetIds.value,
      namedShareUnc: namedShareUnc.value.trim() || null,
      operatorCredentialAlias: operatorCredentialAlias.value,
      sourceSmbCredentialAlias: sourceSmbCredentialAlias.value,
      forceGpuMismatch: forceGpuMismatch.value,
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
  <BaseModal :open="open" title="Distribute PSO Cache" size="lg" @close="emit('close')">
    <div data-pso-dist-wizard class="space-y-4">
      <header class="space-y-1">
        <p data-pso-dist-file-name class="font-mono text-sm font-bold">{{ file?.file_name ?? "" }}</p>
        <p data-pso-dist-source-sig class="font-mono text-xs text-muted-foreground">
          {{ file?.gpu_signature ?? "" }}
        </p>
      </header>

      <section class="rounded-md border bg-card">
        <div class="border-b px-3 py-2 text-xs font-bold text-muted-foreground">Target machines</div>
        <label
          v-for="machine in candidateMachines"
          :key="machine.id"
          data-pso-dist-target-row
          class="flex items-center justify-between gap-3 border-b px-3 py-2 text-sm last:border-b-0"
        >
          <span class="flex min-w-0 items-center gap-2">
            <input v-model="targetIds" type="checkbox" :value="machine.id" />
            <span class="truncate">{{ machine.hostname }} · {{ machine.ip }}</span>
          </span>
          <span
            data-pso-dist-target-flag
            class="shrink-0 font-mono text-xs"
            :class="machine.matches ? 'text-status-healthy' : 'text-status-warning'"
          >
            {{ machine.matches ? "match" : "mismatch" }}
          </span>
        </label>
        <p v-if="candidateMachines.length === 0" class="px-3 py-4 text-sm text-muted-foreground">
          No target machines available.
        </p>
      </section>

      <label v-if="hasMismatch" class="flex items-center gap-2 text-sm text-status-warning">
        <input v-model="forceGpuMismatch" data-pso-dist-force type="checkbox" />
        Force GPU mismatch
      </label>

      <label class="block space-y-1">
        <span class="text-xs font-bold text-muted-foreground">Named share UNC</span>
        <input
          v-model="namedShareUnc"
          class="h-9 w-full rounded-md border bg-background px-3 font-mono text-sm"
          placeholder="\\\\SOURCE\\PSO"
        />
      </label>

      <div class="grid gap-3 md:grid-cols-2">
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Operator credential</span>
          <select v-model="operatorCredentialAlias" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
            <option :value="null">Current session</option>
            <option v-for="credential in winrmCredentials" :key="credential.alias" :value="credential.alias">
              {{ credential.alias }}
            </option>
          </select>
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-bold text-muted-foreground">Source SMB credential</span>
          <select v-model="sourceSmbCredentialAlias" class="h-9 w-full rounded-md border bg-background px-3 text-sm">
            <option :value="null">Same as operator</option>
            <option v-for="credential in shareCredentials" :key="credential.alias" :value="credential.alias">
              {{ credential.alias }}
            </option>
          </select>
        </label>
      </div>

      <p v-if="errorMessage || pso.error" class="text-sm text-status-critical">
        {{ errorMessage ?? pso.error?.message }}
      </p>
    </div>

    <template #footer>
      <Button variant="outline" @click="emit('close')">Cancel</Button>
      <Button data-pso-dist-run :disabled="!canSubmit" @click="run">
        {{ isSubmitting ? "Distributing" : "Distribute" }}
      </Button>
    </template>
  </BaseModal>
</template>
