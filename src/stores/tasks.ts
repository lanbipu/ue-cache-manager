import { defineStore } from "pinia";
import { computed, ref } from "vue";

export type TaskStatus = "progress" | "healthy" | "warning" | "critical";

export interface TaskRecord {
  id: string;
  label: string;
  progress: number;
  status: TaskStatus;
}

export const useTasksStore = defineStore("tasks", () => {
  const tasks = ref<TaskRecord[]>([]);
  const activeTask = computed(() => tasks.value.find((task) => task.status === "progress") ?? null);

  function upsertTask(task: TaskRecord) {
    const index = tasks.value.findIndex((item) => item.id === task.id);
    if (index === -1) tasks.value.push(task);
    else tasks.value[index] = task;
  }

  function completeTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (task) {
      task.status = "healthy";
      task.progress = 100;
    }
  }

  return { tasks, activeTask, upsertTask, completeTask };
});
