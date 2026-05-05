<script setup lang="ts">
import { computed, ref, watch } from "vue";
import BaseModal from "./BaseModal.vue";
import Button from "@/components/ui/Button.vue";
import Input from "@/components/ui/Input.vue";
import { useMachinesStore } from "@/stores/machines";
import { useCredentialsStore } from "@/stores/credentials";
import { useDiagnosticsStore } from "@/stores/diagnostics";
import { formatUecmError } from "@/services/tauri";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const machines = useMachinesStore();
const creds = useCredentialsStore();
const diag = useDiagnosticsStore();

const selected = ref<Set<number>>(new Set());
const credAlias = ref<string>("");
const userProfile = ref<string>("C:\\Users\\lanpc");
const projectPathsRaw = ref<string>("");

watch(() => props.open, async (val) => {
  if (val) { await machines.loadMachines(); await creds.load(); }
});

const winrmCreds = computed(() => creds.credentials.filter(c => c.kind === "winrm"));
const projectPaths = computed(() => projectPathsRaw.value
  .split("\n").map(s => s.trim()).filter(Boolean));

async function onRun() {
  const ids = Array.from(selected.value);
  if (ids.length === 0 || !credAlias.value) return;
  const perMachine: Record<number, string[]> = {};
  for (const id of ids) perMachine[id] = projectPaths.value;
  await diag.runScan(ids, perMachine, userProfile.value, credAlias.value);
  if (diag.error) {
    window.alert(`Scan failed: ${formatUecmError(diag.error)}`);
    return;
  }
  emit("close");
}

function toggle(id: number) {
  const s = new Set(selected.value);
  if (s.has(id)) s.delete(id); else s.add(id);
  selected.value = s;
}
</script>

<template>
  <BaseModal :open="open" title="Run INI scan" size="lg" @close="emit('close')">
    <div class="space-y-4">
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Machines</p>
        <ul class="grid grid-cols-2 gap-1 text-sm">
          <li v-for="m in machines.machines" :key="m.id ?? m.ip" class="flex items-center gap-2">
            <input type="checkbox" :checked="m.id != null && selected.has(m.id)" @change="m.id != null && toggle(m.id)" />
            <span>{{ m.hostname }} <span class="font-mono text-xs text-muted-foreground">{{ m.ip }}</span></span>
          </li>
        </ul>
      </div>
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Credential</p>
        <select data-cred-select v-model="credAlias" class="w-full rounded-md border bg-background px-2 py-1 text-sm">
          <option value="">— pick —</option>
          <option v-for="c in winrmCreds" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
        </select>
      </div>
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">User profile</p>
        <Input v-model="userProfile" placeholder="C:\\Users\\lanpc" />
      </div>
      <div>
        <p class="mb-2 font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Project paths (optional, one per line)</p>
        <textarea v-model="projectPathsRaw" rows="3" class="w-full rounded-md border bg-background px-2 py-1 font-mono text-xs"
                  placeholder="E:\\Work\\EXLY"></textarea>
      </div>
    </div>
    <template #footer>
      <Button variant="outline" @click="emit('close')">Cancel</Button>
      <Button data-run-scan-btn :disabled="!credAlias || selected.size === 0 || diag.isScanning" @click="onRun">
        {{ diag.isScanning ? "Scanning…" : `Run on ${selected.size} machine(s)` }}
      </Button>
    </template>
  </BaseModal>
</template>
