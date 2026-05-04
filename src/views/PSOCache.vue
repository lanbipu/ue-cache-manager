<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import Button from "@/components/ui/Button.vue";
import PsoCollectWizard from "@/components/modals/PsoCollectWizard.vue";
import PsoDistributeWizard from "@/components/modals/PsoDistributeWizard.vue";
import PsoFileExplorer from "@/components/pso/PsoFileExplorer.vue";
import PsoJobCard from "@/components/pso/PsoJobCard.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmStat from "@/components/primitives/UecmStat.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";
import { usePsoStore } from "@/stores/pso";
import type { PsoCacheFile } from "@/services/tauri";

const machines = useMachinesStore();
const projects = useProjectsStore();
const pso = usePsoStore();

const showCollect = ref(false);
const showDistribute = ref(false);
const distributeFile = ref<PsoCacheFile | null>(null);
const selectedProjectId = ref<number | null>(null);

onMounted(async () => {
  await Promise.all([machines.loadMachines(), projects.load(), pso.attach()]);
});

onUnmounted(() => {
  void pso.detach();
});

watch(selectedProjectId, async (projectId) => {
  if (projectId != null) await pso.loadFiles(projectId);
});

const files = computed(() =>
  selectedProjectId.value != null
    ? pso.cacheFilesByProject[selectedProjectId.value] ?? []
    : [],
);
const activeCollectJobs = computed(() =>
  pso.collectJobs.filter((job) => ["spawning", "collecting", "completing"].includes(job.status)).length,
);
const completedCollectJobs = computed(
  () => pso.collectJobs.filter((job) => job.status === "completed").length,
);

function machineLabel(id: number) {
  return machines.machines.find((machine) => machine.id === id)?.hostname ?? `m#${id}`;
}

function projectLabel(id: number) {
  return projects.projects.find((project) => project.id === id)?.uproject_name ?? `project#${id}`;
}

async function cancel(jobId: string) {
  await pso.cancelCollection(jobId);
}

function openDistribute(file: PsoCacheFile) {
  distributeFile.value = file;
  showDistribute.value = true;
}

function targetTone(status: string) {
  if (status === "ok") return "healthy";
  if (status === "err") return "critical";
  if (status === "running") return "progress";
  return "unknown";
}
</script>

<template>
  <div class="grid h-full grid-rows-[auto_auto_1fr] gap-4 p-6">
    <UecmPageHeader title="PSO Cache" eyebrow="Shader pipeline" description="Collect and fan out GPU-specific pipeline cache files.">
      <template #actions>
        <Button data-pso-collect-btn @click="showCollect = true">
          <UecmIcon name="database" />
          Collect
        </Button>
      </template>
    </UecmPageHeader>

    <section class="grid gap-3 md:grid-cols-3">
      <UecmStat label="Projects" :value="projects.projects.length" icon="folder-git-2" />
      <UecmStat label="Active" :value="activeCollectJobs" icon="loader-2" />
      <UecmStat label="Collected" :value="completedCollectJobs" icon="check-circle-2" />
    </section>

    <main class="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(24rem,0.8fr)]">
      <section class="min-h-0 overflow-auto rounded-lg border bg-card">
        <div class="flex items-center justify-between gap-3 border-b p-4">
          <h2 class="font-display text-sm font-extrabold">Collected files</h2>
          <select
            v-model.number="selectedProjectId"
            data-pso-project-select
            class="h-9 max-w-xs rounded-md border bg-background px-3 text-sm"
          >
            <option :value="null">Select project</option>
            <option v-for="project in projects.projects" :key="project.id" :value="project.id">
              {{ project.uproject_name }}
            </option>
          </select>
        </div>
        <div v-if="selectedProjectId == null" data-pso-cache-empty class="p-6 text-sm text-muted-foreground">
          Select a project or start a collection job.
        </div>
        <PsoFileExplorer
          v-else
          class="border-0"
          :files="files"
          :machine-label="machineLabel"
          @distribute="openDistribute"
        />
      </section>

      <section class="min-h-0 overflow-auto rounded-lg border bg-card">
        <div class="border-b p-4">
          <h2 class="font-display text-sm font-extrabold">Jobs</h2>
        </div>
        <div v-if="pso.collectJobs.length === 0 && pso.distributeJobs.length === 0" class="p-6 text-sm text-muted-foreground">
          No PSO jobs queued.
        </div>
        <div v-else class="space-y-4 p-4">
          <PsoJobCard
            v-for="job in pso.collectJobs"
            :key="job.job_id"
            :job="job"
            :source-label="machineLabel(job.source_machine_id)"
            :project-label="projectLabel(job.project_id)"
            @cancel="cancel"
          />
          <div
            v-for="job in pso.distributeJobs"
            :key="job.job_id"
            data-pso-distribute-job
            class="rounded-lg border p-4"
          >
            <div class="mb-3 flex items-center justify-between gap-3">
              <h3 class="truncate font-display text-sm font-extrabold">Distribute · {{ job.job_id }}</h3>
              <UecmStatusBadge :tone="job.status === 'completed' ? 'healthy' : 'progress'" :label="job.status" size="sm" />
            </div>
            <table class="w-full text-sm">
              <tbody>
                <tr v-for="target in job.targets" :key="target.target_machine_id" class="border-t">
                  <td class="py-2 font-mono">{{ target.target_host }}</td>
                  <td class="py-2 text-right">
                    <UecmStatusBadge :tone="targetTone(target.status)" :label="target.status" size="sm" />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </section>
    </main>

    <PsoCollectWizard :open="showCollect" @close="showCollect = false" />
    <PsoDistributeWizard :open="showDistribute" :file="distributeFile" @close="showDistribute = false" />
  </div>
</template>
