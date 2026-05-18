<script setup lang="ts">
import { useI18n } from "vue-i18n";

interface Inconsistency {
  kind: string;
  found?: Record<string, string[]>;
  hosts?: string[];
}

defineProps<{
  inconsistencies: Inconsistency[];
}>();

const { t } = useI18n();

function entriesOf(inc: Inconsistency): Array<[string, string[]]> {
  if (inc.found) return Object.entries(inc.found);
  if (inc.hosts) return [["(missing UE install)", inc.hosts]];
  return [];
}
</script>

<template>
  <section
    v-if="inconsistencies.length === 0"
    class="rounded-md border border-status-healthy/40 bg-status-healthy/10 p-3 text-status-healthy text-sm"
  >
    {{ t("consistency.allMatch") }}
  </section>
  <section v-else class="space-y-3">
    <article
      v-for="(inc, i) in inconsistencies"
      :key="i"
      class="rounded-md border border-status-warning/40 bg-card p-3 text-sm"
    >
      <h4 class="font-display text-base mb-2">{{ t(`consistency.kind.${inc.kind}`) }}</h4>
      <ul class="space-y-1">
        <li v-for="([value, hosts]) in entriesOf(inc)" :key="value">
          <span class="font-mono">{{ value }}</span> — {{ hosts.join(", ") }}
        </li>
      </ul>
    </article>
  </section>
</template>
