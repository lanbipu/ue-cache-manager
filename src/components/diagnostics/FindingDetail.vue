<script setup lang="ts">
import type { IniFinding } from "@/services/tauri";
import UecmCodeBlock from "@/components/primitives/UecmCodeBlock.vue";
import UecmStatusBadge from "@/components/primitives/UecmStatusBadge.vue";
import Button from "@/components/ui/Button.vue";

defineProps<{ finding: IniFinding | null; busy?: boolean }>();
const emit = defineEmits<{ apply: [finding: IniFinding]; skip: [finding: IniFinding] }>();
</script>

<template>
  <aside data-finding-detail class="overflow-y-auto p-6">
    <p v-if="!finding" class="text-sm text-muted-foreground">Select a finding to inspect it.</p>
    <div v-else class="space-y-4">
      <header class="flex items-center justify-between gap-4">
        <div>
          <h2 class="font-display text-lg font-extrabold">{{ finding.rule_id }}</h2>
          <p class="mt-1 font-mono text-xs text-muted-foreground">{{ finding.section ?? "global" }} · line {{ finding.line_number ?? "-" }}</p>
        </div>
        <UecmStatusBadge :tone="finding.severity" :label="finding.severity" />
      </header>
      <section class="grid gap-3 lg:grid-cols-3">
        <div class="rounded-md border bg-card p-3">
          <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">What</div>
          <p class="mt-1 text-sm">{{ finding.symptom }}</p>
        </div>
        <div class="rounded-md border bg-card p-3">
          <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">Why</div>
          <p class="mt-1 text-sm">{{ finding.rationale }}</p>
        </div>
        <div class="rounded-md border bg-card p-3">
          <div class="font-mono text-[11px] font-bold uppercase text-muted-foreground">Action</div>
          <p class="mt-1 text-sm">{{ finding.recommended_action }}</p>
        </div>
      </section>
      <UecmCodeBlock :before="finding.snippet_before" :after="finding.snippet_after" />
      <div class="flex gap-2">
        <Button data-apply-finding-btn :disabled="busy || finding.recommended_action === 'manual' || finding.fixed_at != null" @click="emit('apply', finding)">
          {{ busy ? "Applying" : "Apply suggestion" }}
        </Button>
        <Button variant="outline" @click="emit('skip', finding)">Skip</Button>
      </div>
    </div>
  </aside>
</template>
