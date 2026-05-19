<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{
  report: {
    host: string;
    local_path?: string | null;
    local_writable?: boolean | null;
    shared_path?: string | null;
    shared_writable?: boolean | null;
    shared_deactivated_reason?: string | null;
    move_collision_count: number;
    maintenance: Array<{ layer: string; file_count: number; total_bytes: number }>;
    paks_opened: string[];
    truncated: boolean;
  };
}>();

const { t } = useI18n();

function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GiB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(1)} MiB`;
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KiB`;
  return `${n} B`;
}
</script>

<template>
  <div class="rounded-md border border-border bg-card p-4 text-card-foreground">
    <h3 class="font-display text-lg mb-2">{{ t("logVerify.title", { host: report.host }) }}</h3>
    <dl class="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
      <dt class="text-muted-foreground">{{ t("logVerify.localPath") }}</dt>
      <dd>
        {{ report.local_path ?? "—" }}
        <span v-if="report.local_writable === false" class="ml-2 text-status-warning">
          {{ t("logVerify.readOnly") }}
        </span>
      </dd>

      <dt class="text-muted-foreground">{{ t("logVerify.sharedPath") }}</dt>
      <dd>
        {{ report.shared_path ?? "—" }}
        <span v-if="report.shared_writable === false" class="ml-2 text-status-warning">
          {{ t("logVerify.readOnly") }}
        </span>
      </dd>

      <template v-if="report.shared_deactivated_reason">
        <dt class="text-status-critical">{{ t("logVerify.deactivated") }}</dt>
        <dd class="text-status-critical">{{ report.shared_deactivated_reason }}</dd>
      </template>

      <dt class="text-muted-foreground">{{ t("logVerify.moveCollisions") }}</dt>
      <dd :class="report.move_collision_count > 0 ? 'text-status-warning' : ''">
        {{ report.move_collision_count }}
      </dd>
    </dl>

    <div v-if="report.maintenance.length" class="mt-3">
      <h4 class="text-sm font-semibold mb-1">{{ t("logVerify.maintenance") }}</h4>
      <table class="w-full text-sm">
        <thead>
          <tr class="text-muted-foreground">
            <th class="text-left">{{ t("logVerify.layer") }}</th>
            <th class="text-right">{{ t("logVerify.files") }}</th>
            <th class="text-right">{{ t("logVerify.size") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="m in report.maintenance" :key="m.layer">
            <td>{{ m.layer }}</td>
            <td class="text-right">{{ m.file_count.toLocaleString() }}</td>
            <td class="text-right">{{ formatBytes(m.total_bytes) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <p v-if="report.truncated" class="mt-2 text-xs text-status-warning">
      {{ t("logVerify.truncated") }}
    </p>
  </div>
</template>
