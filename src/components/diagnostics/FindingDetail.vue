<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import UecmCodeBlock from "@/components/primitives/UecmCodeBlock.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import Button from "@/components/ui/Button.vue";
import { INI_RULES } from "@/lib/iniRules";
import type { IniFinding } from "@/services/tauri";

const { t } = useI18n();
const props = defineProps<{ finding: IniFinding | null; busy: boolean }>();
const emit = defineEmits<{ apply: [f: IniFinding]; skip: [f: IniFinding] }>();

const rule = computed(() => props.finding ? (INI_RULES[props.finding.rule_id] ?? null) : null);
</script>

<template>
  <div v-if="!finding" data-finding-empty class="grid h-full place-items-center text-sm text-muted-foreground">
    {{ t("findingDetail.selectHint") }}
  </div>
  <div v-else data-finding-detail class="flex h-full flex-col overflow-y-auto">
    <header class="flex items-center gap-3 border-b bg-card/30 px-6 py-4">
      <UecmStatusBadge :tone="finding.severity" :label="finding.severity" size="md" />
      <div>
        <h2 class="font-display text-lg font-extrabold">{{ rule?.label ?? finding.rule_id }}</h2>
        <p class="font-mono text-xs text-muted-foreground">{{ finding.file_path }} · L{{ finding.line_number ?? '?' }}</p>
      </div>
    </header>
    <div class="grid gap-3 px-6 py-4 md:grid-cols-3">
      <div class="rounded-md border bg-card p-3">
        <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("findingDetail.whatsWrong") }}</p>
        <p class="mt-1 text-sm">{{ rule?.description ?? finding.rationale }}</p>
      </div>
      <div class="rounded-md border bg-card p-3">
        <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("findingDetail.whyItMatters") }}</p>
        <p class="mt-1 text-sm">{{ rule?.rationale ?? finding.rationale }}</p>
      </div>
      <div class="rounded-md border bg-card p-3">
        <p class="font-mono text-[11px] font-bold uppercase tracking-wide text-muted-foreground">{{ t("findingDetail.userFacingSymptom") }}</p>
        <p class="mt-1 text-sm">{{ finding.symptom }}</p>
      </div>
    </div>
    <div class="grid gap-3 px-6 pb-4 md:grid-cols-2">
      <UecmCodeBlock :code="finding.snippet_before" tone="critical" :caption="t('findingDetail.detectedCaption')"
                     :start-line="(finding.line_number ?? 1) - 1" :highlight-line="finding.line_number ?? 0" />
      <UecmCodeBlock v-if="finding.snippet_after" :code="finding.snippet_after" tone="healthy" :caption="t('findingDetail.suggestedFixCaption')"
                     :start-line="(finding.line_number ?? 1) - 1" :highlight-line="finding.line_number ?? 0" />
    </div>
    <footer class="mt-auto flex items-center gap-2 border-t bg-card/30 px-6 py-3">
      <Button data-apply-btn :disabled="finding.recommended_action === 'manual' || finding.fixed_at != null || busy"
              @click="emit('apply', finding)">
        {{ busy ? t("findingDetail.applying") : t("findingDetail.applySuggestion") }}
      </Button>
      <Button variant="outline" disabled>{{ t("findingDetail.customEdit") }}</Button>
      <Button variant="outline" disabled>{{ t("findingDetail.openFile") }}</Button>
      <Button data-skip-btn variant="ghost" :disabled="finding.skipped_at != null"
              @click="emit('skip', finding)">{{ t("findingDetail.skip") }}</Button>
      <p v-if="finding.fixed_at" class="ml-auto text-xs text-status-healthy">{{ t("findingDetail.appliedAt", { time: finding.fixed_at }) }}</p>
      <p v-else-if="finding.skipped_at" class="ml-auto text-xs text-muted-foreground">{{ t("findingDetail.skipped") }}</p>
    </footer>
  </div>
</template>
