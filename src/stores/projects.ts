import { defineStore } from "pinia";
import { computed, ref } from "vue";
import {
  tauriApi,
  type DiscoveryResult,
  type ProjectLocation,
  type ProjectSummary,
  type UecmError,
} from "@/services/tauri";

export const useProjectsStore = defineStore("projects", () => {
  const projects = ref<ProjectSummary[]>([]);
  const locations = ref<Record<number, ProjectLocation[]>>({});
  const isLoading = ref(false);
  const error = ref<UecmError | null>(null);

  async function load() {
    isLoading.value = true;
    error.value = null;
    try {
      projects.value = await tauriApi.listProjects();
    } catch (e) {
      error.value = e as UecmError;
    } finally {
      isLoading.value = false;
    }
  }

  async function loadLocations(projectId: number) {
    error.value = null;
    try {
      locations.value = {
        ...locations.value,
        [projectId]: await tauriApi.listProjectLocations(projectId),
      };
    } catch (e) {
      error.value = e as UecmError;
    }
  }

  async function discover(
    machineId: number,
    searchRoots: string[],
    credentialAlias: string | null,
  ): Promise<DiscoveryResult[]> {
    error.value = null;
    const results = await tauriApi.discoverProjects(machineId, searchRoots, credentialAlias);
    await load();
    return results;
  }

  async function setLocation(
    projectId: number,
    machineId: number,
    absPath: string,
    uprojectPath: string,
    manual: boolean,
  ) {
    error.value = null;
    await tauriApi.setProjectLocation(projectId, machineId, absPath, uprojectPath, manual);
    await loadLocations(projectId);
    await load();
  }

  async function removeProject(projectId: number) {
    error.value = null;
    await tauriApi.deleteProject(projectId);
    const next = { ...locations.value };
    delete next[projectId];
    locations.value = next;
    await load();
  }

  async function removeLocation(projectId: number, locationId: number) {
    error.value = null;
    await tauriApi.deleteProjectLocation(locationId);
    await loadLocations(projectId);
    await load();
  }

  async function createManual(uprojectName: string, displayName: string | null) {
    error.value = null;
    const id = await tauriApi.createProjectManual(uprojectName, displayName);
    await load();
    return id;
  }

  const projectsById = computed(() => {
    const map = new Map<number, ProjectSummary>();
    for (const project of projects.value) {
      map.set(project.id, project);
    }
    return map;
  });

  return {
    projects,
    locations,
    isLoading,
    error,
    load,
    loadLocations,
    discover,
    setLocation,
    removeProject,
    removeLocation,
    createManual,
    projectsById,
  };
});
