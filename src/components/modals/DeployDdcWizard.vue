<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import DeployStepIndicator from "@/components/deploy/DeployStepIndicator.vue";
import DeployProgressTable from "@/components/deploy/DeployProgressTable.vue";
import DeployVerifyReport from "@/components/deploy/DeployVerifyReport.vue";
import { useDeployStore } from "@/stores/deploy";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { useCredentialsStore } from "@/stores/credentials";
import type { DeployPlan } from "@/lib/deployApi";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();
const { t } = useI18n();

const deploy = useDeployStore();
const machines = useMachinesStore();
const projects = useProjectsStore();
const credentials = useCredentialsStore();

const step = ref<1 | 2 | 3 | 4 | 5>(1);
const projectId = ref<number | null>(null);
const sourceMachineId = ref<number | null>(null);
const targetMachineIds = ref<number[]>([]);
const localPath = ref("D:\\UE-DDC-Local");
const sharedServerId = ref<number | null>(null);
const shareName = ref("DDC");
const sharePath = ref("D:\\DDC");
const shareMode = ref<"a" | "b">("b");
const enablePak = ref(true);
const enablePso = ref(true);
const psoRes = ref("1920x1080");
const psoMinutes = ref(10);
const runVerify = ref(true);
const editorExe = ref("C:\\Program Files\\Epic Games\\UE_5.5\\Engine\\Binaries\\Win64\\UnrealEditor.exe");
const uproject = ref("");
const credAlias = ref<string>("");
const stopOnFailure = ref(true);
const verifyReports = ref<Array<{ host: string; report: unknown }>>([]);

watch(() => props.open, (isOpen) => {
  if (isOpen) {
    step.value = 1;
    deploy.reset();
    verifyReports.value = [];
  }
});

watch(() => deploy.completed, async (done) => {
  if (!done || !runVerify.value || !deploy.finalOk) return;
  verifyReports.value = [];
  for (const mid of targetMachineIds.value) {
    const machine = machines.machines.find((m) => m.id === mid);
    if (!machine) continue;
    try {
      const report = await invoke("run_log_verify", {
        host: machine.hostname,
        editorExe: editorExe.value,
        project: uproject.value || `${localPath.value}\\.uproject`,
        timeout: 180,
        credentialAlias: credAlias.value || null,
      });
      verifyReports.value.push({ host: machine.hostname, report });
    } catch (e) {
      console.error(`verify for ${machine.hostname}:`, e);
    }
  }
});

const canRun = computed(() =>
  projectId.value !== null
  && sourceMachineId.value !== null
  && targetMachineIds.value.length > 0
  && sharedServerId.value !== null
  && credAlias.value
  && !deploy.running,
);

function buildPlan(): DeployPlan {
  return {
    project_id: projectId.value!,
    source_machine_id: sourceMachineId.value!,
    target_machine_ids: [...targetMachineIds.value],
    local_cache: { path: localPath.value, service_account: null },
    shared_cache: {
      server_machine_id: sharedServerId.value!,
      share_name: shareName.value,
      server_path: sharePath.value,
      mode: shareMode.value,
      unc_path: null,
    },
    ddc_pak: { enabled: enablePak.value },
    pso: { enabled: enablePso.value, resolution: psoRes.value, max_minutes: psoMinutes.value },
    verify: {
      run_log_verify: runVerify.value,
      editor_exe: editorExe.value,
      timeout_seconds: 180,
    },
  };
}

async function onPreview() { await deploy.preview(buildPlan()); step.value = 5; }
async function onRun() { await deploy.run(buildPlan(), credAlias.value, stopOnFailure.value); }
</script>

<template>
  <BaseModal :open="props.open" :title="t('deploy.title')" size="lg" @close="emit('close')">
    <div class="space-y-4 max-h-[80vh] overflow-y-auto">
      <section v-if="step === 1">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s1.title") }}</h3>
        <label class="block text-sm mb-1">{{ t("deploy.s1.project") }}</label>
        <select v-model="projectId" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option :value="null">--</option>
          <option v-for="p in projects.projects" :key="p.id" :value="p.id">{{ p.display_name ?? p.uproject_name }}</option>
        </select>
        <label class="block text-sm mb-1">{{ t("deploy.s1.sourceHost") }}</label>
        <select v-model="sourceMachineId" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option :value="null">--</option>
          <option v-for="m in machines.machines" :key="m.hostname" :value="m.id">{{ m.hostname }}</option>
        </select>
        <label class="block text-sm mb-1">{{ t("deploy.s1.targets") }}</label>
        <div class="max-h-40 overflow-y-auto border border-border rounded p-2">
          <label v-for="m in machines.machines" :key="m.hostname" class="flex items-center gap-2 text-sm">
            <input type="checkbox" :value="m.id" v-model="targetMachineIds" />
            {{ m.hostname }}
          </label>
        </div>
        <button class="mt-3 px-3 py-1 rounded bg-primary text-primary-foreground text-sm"
                :disabled="projectId === null || sourceMachineId === null || !targetMachineIds.length"
                @click="step = 2">{{ t("common.next") }}</button>
      </section>

      <section v-if="step === 2">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s2.title") }}</h3>
        <label class="block text-sm mb-1">{{ t("deploy.s2.localPath") }}</label>
        <input v-model="localPath" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        <div class="flex gap-2">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 1">{{ t("common.back") }}</button>
          <button class="px-3 py-1 rounded bg-primary text-primary-foreground text-sm" @click="step = 3">{{ t("common.next") }}</button>
        </div>
      </section>

      <section v-if="step === 3">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s3.title") }}</h3>
        <label class="block text-sm mb-1">{{ t("deploy.s3.serverHost") }}</label>
        <select v-model="sharedServerId" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option :value="null">--</option>
          <option v-for="m in machines.machines" :key="m.hostname" :value="m.id">{{ m.hostname }}</option>
        </select>
        <label class="block text-sm mb-1">{{ t("deploy.s3.shareName") }}</label>
        <input v-model="shareName" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        <label class="block text-sm mb-1">{{ t("deploy.s3.sharePath") }}</label>
        <input v-model="sharePath" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        <label class="block text-sm mb-1">{{ t("deploy.s3.mode") }}</label>
        <select v-model="shareMode" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option value="a">{{ t("deploy.s3.modeOpen") }}</option>
          <option value="b">{{ t("deploy.s3.modeManaged") }}</option>
        </select>
        <div class="flex gap-2">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 2">{{ t("common.back") }}</button>
          <button class="px-3 py-1 rounded bg-primary text-primary-foreground text-sm" @click="step = 4">{{ t("common.next") }}</button>
        </div>
      </section>

      <section v-if="step === 4">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s4.title") }}</h3>
        <label class="flex items-center gap-2 text-sm mb-2"><input type="checkbox" v-model="enablePak" /> {{ t("deploy.s4.pak") }}</label>
        <label class="flex items-center gap-2 text-sm mb-2"><input type="checkbox" v-model="enablePso" /> {{ t("deploy.s4.pso") }}</label>
        <div v-if="enablePso" class="ml-6 mb-3 space-y-2">
          <div class="flex items-center gap-2 text-sm">
            <label class="w-32">{{ t("deploy.s4.psoRes") }}</label>
            <input v-model="psoRes" class="flex-1 rounded border border-input bg-transparent px-2 py-1 text-sm" />
          </div>
          <div class="flex items-center gap-2 text-sm">
            <label class="w-32">{{ t("deploy.s4.psoMinutes") }}</label>
            <input type="number" v-model.number="psoMinutes" min="1" max="120"
                   class="flex-1 rounded border border-input bg-transparent px-2 py-1 text-sm" />
          </div>
        </div>
        <label class="flex items-center gap-2 text-sm mb-2"><input type="checkbox" v-model="runVerify" /> {{ t("deploy.s4.verify") }}</label>
        <div v-if="runVerify" class="ml-6 mb-3 space-y-2">
          <input v-model="editorExe" :placeholder="t('deploy.s4.editorExe')" class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
          <input v-model="uproject" :placeholder="t('deploy.s4.uproject')" class="w-full rounded border border-input bg-transparent px-2 py-1 text-sm" />
        </div>
        <label class="block text-sm mb-1 mt-3">{{ t("deploy.s4.cred") }}</label>
        <select v-model="credAlias" class="mb-3 w-full rounded border border-input bg-transparent px-2 py-1 text-sm">
          <option value="">--</option>
          <option v-for="c in credentials.credentials" :key="c.alias" :value="c.alias">{{ c.alias }}</option>
        </select>
        <label class="flex items-center gap-2 text-sm mb-3"><input type="checkbox" v-model="stopOnFailure" /> {{ t("deploy.s4.stopOnFailure") }}</label>
        <div class="flex gap-2">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 3">{{ t("common.back") }}</button>
          <button class="px-3 py-1 rounded bg-primary text-primary-foreground text-sm" :disabled="!canRun" @click="onPreview">{{ t("deploy.s4.preview") }}</button>
        </div>
      </section>

      <section v-if="step === 5">
        <h3 class="font-display text-base mb-2">{{ t("deploy.s5.title") }}</h3>
        <div class="grid grid-cols-[max-content_1fr] gap-4">
          <DeployStepIndicator :steps="deploy.steps" :status="deploy.progress" />
          <DeployProgressTable :steps="deploy.steps" :status="deploy.progress" />
        </div>
        <div class="mt-4 flex items-center gap-3">
          <button v-if="!deploy.running && !deploy.completed" class="px-4 py-1.5 rounded bg-primary text-primary-foreground text-sm" @click="onRun">{{ t("deploy.s5.run") }}</button>
          <span v-if="deploy.running" class="text-sm text-status-info">{{ t("deploy.s5.running") }}</span>
          <span v-if="deploy.completed" :class="deploy.finalOk ? 'text-status-healthy' : 'text-status-critical'">{{ deploy.summary }}</span>
        </div>
        <div v-if="verifyReports.length" class="mt-4 space-y-2">
          <h4 class="font-display text-sm">{{ t("deploy.s5.verifyResults") }}</h4>
          <DeployVerifyReport
            v-for="vr in verifyReports"
            :key="vr.host"
            :report="(vr.report as any)"
          />
        </div>
        <div class="mt-3">
          <button class="px-3 py-1 rounded border border-border text-sm" @click="step = 4" :disabled="deploy.running">{{ t("common.back") }}</button>
        </div>
      </section>
    </div>
  </BaseModal>
</template>
