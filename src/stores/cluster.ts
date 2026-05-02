import { defineStore } from "pinia";
import { computed } from "vue";
import { useMachinesStore } from "@/stores/machines";

function isOnline(status: string) {
  return status.toLowerCase() === "online";
}

function isCritical(status: string) {
  return ["critical", "error", "failed"].includes(status.toLowerCase());
}

function isWarning(status: string) {
  return ["warning", "warn", "degraded"].includes(status.toLowerCase());
}

export const useClusterStore = defineStore("cluster", () => {
  const machinesStore = useMachinesStore();
  const machines = computed(() => machinesStore.machines);
  const total = computed(() => machines.value.length);
  const online = computed(() => machines.value.filter((machine) => isOnline(machine.status)).length);
  const critical = computed(() => machines.value.filter((machine) => isCritical(machine.status)).length);
  const warning = computed(() => machines.value.filter((machine) => isWarning(machine.status)).length);
  const score = computed(() => {
    if (total.value === 0) return 0;
    return Math.max(0, Math.round(((online.value - critical.value * 0.75 - warning.value * 0.35) / total.value) * 100));
  });
  const summary = computed(() => `${online.value}/${total.value} online`);

  return { machines, total, online, critical, warning, score, summary };
});
