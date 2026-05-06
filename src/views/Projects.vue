<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Button from "@/components/ui/Button.vue";
import ProjectDiscoveryWizard from "@/components/modals/ProjectDiscoveryWizard.vue";
import ProjectMatchingModal from "@/components/modals/ProjectMatchingModal.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmPageHeader from "@/components/primitives/UecmPageHeader.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import { useCredentialsStore } from "@/stores/credentials";
import { useMachinesStore } from "@/stores/machines";
import { useProjectsStore } from "@/stores/projects";

const projects = useProjectsStore();
const machines = useMachinesStore();
const credentials = useCredentialsStore();

const showDiscovery = ref(false);
const showMatching = ref(false);
const expanded = ref<Set<number>>(new Set());

onMounted(async () => {
  await Promise.all([projects.load(), machines.loadMachines(), credentials.load()]);
});

const machineNameById = computed(() => {
  const map = new Map<number, string>();
  for (const machine of machines.machines) {
    if (machine.id != null) map.set(machine.id, machine.hostname);
  }
  return map;
});

async function toggleProject(projectId: number) {
  const next = new Set(expanded.value);
  if (next.has(projectId)) {
    next.delete(projectId);
  } else {
    next.add(projectId);
    await projects.loadLocations(projectId);
  }
  expanded.value = next;
}
</script>

<template>
  <div class="grid h-full grid-rows-[auto_1fr] gap-4 p-6">
    <UecmPageHeader title="Projects" eyebrow="Unreal projects" description="Logical project identity and per-machine project paths.">
      <template #actions>
        <Button variant="outline" data-map-project-btn @click="showMatching = true">
          <UecmIcon name="plus" />
          Map
        </Button>
        <Button data-discover-projects-btn @click="showDiscovery = true">
          <UecmIcon name="radar" />
          Discover
        </Button>
      </template>
    </UecmPageHeader>

    <main class="min-h-0 overflow-auto rounded-lg border bg-card">
      <div class="flex items-center justify-between border-b p-4">
        <div>
          <h2 class="font-display text-sm font-extrabold">Project inventory</h2>
          <p class="mt-1 text-xs text-muted-foreground">{{ projects.projects.length }} logical projects</p>
        </div>
        <UecmStatusBadge :tone="projects.error ? 'critical' : 'info'" :label="projects.error ? 'error' : 'ready'" size="sm" />
      </div>

      <p v-if="projects.isLoading" class="p-4 text-sm text-muted-foreground">Loading...</p>
      <p v-else-if="projects.projects.length === 0" data-projects-empty class="p-6 text-sm text-muted-foreground">
        No projects discovered yet.
      </p>

      <div v-else class="divide-y">
        <section v-for="project in projects.projects" :key="project.id" data-project-row class="p-4">
          <button class="flex w-full items-center justify-between gap-4 text-left" @click="toggleProject(project.id)">
            <span class="min-w-0">
              <span class="block truncate font-display text-sm font-extrabold">{{ project.uproject_name }}</span>
              <span class="mt-1 block text-xs text-muted-foreground">
                {{ project.location_count }} location{{ project.location_count === 1 ? "" : "s" }}
              </span>
            </span>
            <UecmIcon :name="expanded.has(project.id) ? 'chevron-down' : 'chevron-right'" />
          </button>

          <div v-if="expanded.has(project.id)" class="mt-3 overflow-hidden rounded-md border">
            <table class="w-full text-left text-sm">
              <thead class="bg-muted text-xs uppercase text-muted-foreground">
                <tr>
                  <th class="px-3 py-2">Machine</th>
                  <th class="px-3 py-2">Root</th>
                  <th class="px-3 py-2">Status</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="loc in projects.locations[project.id] ?? []" :key="loc.id ?? `${loc.machine_id}-${loc.abs_path}`" data-project-location-row class="border-t">
                  <td class="px-3 py-2">{{ machineNameById.get(loc.machine_id) ?? `#${loc.machine_id}` }}</td>
                  <td class="px-3 py-2 font-mono text-xs">{{ loc.abs_path }}</td>
                  <td class="px-3 py-2">
                    <UecmStatusBadge tone="info" :label="loc.discovery_status" size="sm" />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>
      </div>
      <p v-if="projects.error" class="p-4 text-sm text-status-critical">{{ projects.error.message }}</p>
    </main>

    <ProjectDiscoveryWizard
      :open="showDiscovery"
      :machines="machines.machines"
      :credentials="credentials.credentials"
      @close="showDiscovery = false"
      @discovered="projects.load()"
    />
    <ProjectMatchingModal
      :open="showMatching"
      :projects="projects.projects"
      :machines="machines.machines"
      @close="showMatching = false"
      @saved="projects.load()"
    />
  </div>
</template>
