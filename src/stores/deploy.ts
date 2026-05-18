import { defineStore } from "pinia";
import { ref } from "vue";
import {
  previewPlan,
  runPlan,
  subscribe,
  type DeployEvent,
  type DeployPlan,
  type DeployStep,
} from "@/lib/deployApi";

interface HostStatus {
  state: "pending" | "running" | "ok" | "error";
  message: string | null;
}
interface StepProgress {
  step: DeployStep;
  hosts: Record<string, HostStatus>;
  ok_count: number;
  fail_count: number;
}

export const useDeployStore = defineStore("deploy", () => {
  const steps = ref<DeployStep[]>([]);
  const progress = ref<Record<string, StepProgress>>({});
  const running = ref(false);
  const completed = ref(false);
  const finalOk = ref<boolean | null>(null);
  const summary = ref("");
  let unlisten: (() => void) | null = null;

  async function preview(plan: DeployPlan) {
    steps.value = await previewPlan(plan);
    progress.value = {};
    completed.value = false;
    finalOk.value = null;
    summary.value = "";
    for (const s of steps.value) {
      progress.value[s] = { step: s, hosts: {}, ok_count: 0, fail_count: 0 };
    }
  }

  async function run(plan: DeployPlan, credentialAlias: string | null, stopOnFailure: boolean) {
    await preview(plan);
    running.value = true;
    unlisten = await subscribe(onEvent);
    try {
      await runPlan(plan, credentialAlias, stopOnFailure);
    } finally {
      running.value = false;
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    }
  }

  function onEvent(e: DeployEvent) {
    if (e.kind === "step_started") {
      const p = progress.value[e.step];
      if (!p) return;
      for (const h of e.hosts) {
        p.hosts[h] = { state: "running", message: null };
      }
    } else if (e.kind === "step_host_ok") {
      const p = progress.value[e.step];
      if (!p) return;
      p.hosts[e.host] = { state: "ok", message: e.message };
    } else if (e.kind === "step_host_error") {
      const p = progress.value[e.step];
      if (!p) return;
      p.hosts[e.host] = { state: "error", message: e.error };
    } else if (e.kind === "step_completed") {
      const p = progress.value[e.step];
      if (!p) return;
      p.ok_count = e.ok_count;
      p.fail_count = e.fail_count;
    } else if (e.kind === "plan_completed") {
      completed.value = true;
      finalOk.value = e.ok;
      summary.value = e.summary;
    }
  }

  return { steps, progress, running, completed, finalOk, summary, preview, run };
});
