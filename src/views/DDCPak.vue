<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Button from "@/components/ui/Button.vue";
import DdcPakWizard from "@/components/modals/DdcPakWizard.vue";
import DistributeProgressTable from "@/components/ddcpak/DistributeProgressTable.vue";
import PakJobCard from "@/components/ddcpak/PakJobCard.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmStat from "@/components/primitives/UecmStat.vue";
import { useCredentialsStore } from "@/stores/credentials";
import { useDdcPakStore } from "@/stores/ddcPak";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";

const projects = useProjectsStore();
const machines = useMachinesStore();
const credentials = useCredentialsStore();
const ddcPak = useDdcPakStore();

const showWizard = ref(false);

onMounted(async () => {
  await Promise.all([projects.load(), machines.loadMachines(), credentials.load(), ddcPak.attach()]);
  await Promise.all(projects.projects.map((project) => projects.loadLocations(project.id)));
});

const runningGenerates = computed(() =>
  ddcPak.generateJobs.filter((job) => ["spawning", "running", "verifying"].includes(job.status)).length,
);
const completedGenerates = computed(() =>
  ddcPak.generateJobs.filter((job) => job.status === "completed").length,
);
</script>

<template>
  <div class="grid h-full grid-rows-[auto_auto_1fr] gap-4 p-6">
    <UecmPageHeader title="DDC Pak" eyebrow="Artifact workflow" description="Generate and fan out portable Derived Data Cache pak files.">
      <template #actions>
        <Button data-open-ddc-pak-wizard @click="showWizard = true">
          <UecmIcon name="package" />
          Generate
        </Button>
      </template>
    </UecmPageHeader>

    <section class="grid gap-3 md:grid-cols-3">
      <UecmStat label="Projects" :value="projects.projects.length" icon="folder-git-2" />
      <UecmStat label="Running" :value="runningGenerates" icon="loader-2" />
      <UecmStat label="Completed" :value="completedGenerates" icon="check-circle-2" />
    </section>

    <main class="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(24rem,0.8fr)]">
      <section class="min-h-0 overflow-auto rounded-lg border bg-card">
        <div class="border-b p-4">
          <h2 class="font-display text-sm font-extrabold">Generation jobs</h2>
        </div>
        <div v-if="ddcPak.generateJobs.length === 0" data-ddc-pak-empty class="p-6 text-sm text-muted-foreground">
          No DDC pak job queued.
        </div>
        <div v-else class="space-y-3 p-4">
          <PakJobCard
            v-for="job in ddcPak.generateJobs"
            :key="job.job_id"
            :job="job"
            @cancel="ddcPak.cancel"
          />
        </div>
      </section>

      <section class="min-h-0 overflow-auto rounded-lg border bg-card">
        <div class="border-b p-4">
          <h2 class="font-display text-sm font-extrabold">Distribution jobs</h2>
        </div>
        <div v-if="ddcPak.distributeJobs.length === 0" class="p-6 text-sm text-muted-foreground">
          No distribution running.
        </div>
        <div v-else class="space-y-4 p-4">
          <DistributeProgressTable
            v-for="job in ddcPak.distributeJobs"
            :key="job.job_id"
            :job="job"
          />
        </div>
      </section>
    </main>

    <DdcPakWizard
      :open="showWizard"
      :projects="projects.projects"
      :locations="projects.locations"
      :machines="machines.machines"
      :credentials="credentials.credentials"
      @close="showWizard = false"
    />
  </div>
</template>
