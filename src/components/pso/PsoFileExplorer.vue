<script setup lang="ts">
import { computed, ref, watch } from "vue";
import UecmHorizontalSplit from "@/components/primitives/UecmHorizontalSplit.vue";
import type { PsoCacheFile } from "@/services/tauri";

const props = defineProps<{
  files: PsoCacheFile[];
  machineLabel: (id: number) => string;
}>();

const emit = defineEmits<{
  (e: "distribute", file: PsoCacheFile): void;
}>();

const selectedId = ref<number | null>(null);
const selected = computed(() =>
  props.files.find((file) => file.id != null && file.id === selectedId.value) ?? null,
);

watch(
  () => props.files,
  (files) => {
    selectedId.value = files[0]?.id ?? null;
  },
  { immediate: true },
);

function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
</script>

<template>
  <div data-pso-file-explorer class="h-96 overflow-hidden rounded-lg border bg-card">
    <UecmHorizontalSplit :left-weight="2" :right-weight="3">
      <template #left>
        <ul v-if="files.length > 0">
          <li
            v-for="file in files"
            :key="file.id ?? file.file_name"
            data-pso-file-row
            class="cursor-pointer border-b px-3 py-2 text-xs"
            :class="file.id === selectedId ? 'bg-muted' : 'hover:bg-muted/40'"
            @click="selectedId = file.id"
          >
            <p class="truncate font-mono font-bold">{{ file.file_name }}</p>
            <p class="truncate text-muted-foreground">
              {{ formatSize(file.size_bytes) }} / {{ machineLabel(file.source_machine_id) }}
            </p>
          </li>
        </ul>
        <p v-else data-pso-file-empty class="p-4 text-sm text-muted-foreground">
          No collected PSO files yet for this project.
        </p>
      </template>

      <template #right>
        <div v-if="selected" class="space-y-3 p-4 text-sm">
          <div>
            <p class="font-mono text-xs font-bold text-muted-foreground">Path</p>
            <p class="break-all font-mono text-xs">{{ selected.file_path }}</p>
          </div>
          <div class="grid gap-3 md:grid-cols-2">
            <div>
              <p class="font-mono text-xs font-bold text-muted-foreground">Size</p>
              <p>{{ formatSize(selected.size_bytes) }}</p>
            </div>
            <div>
              <p class="font-mono text-xs font-bold text-muted-foreground">UE</p>
              <p>{{ selected.ue_version ?? "-" }}</p>
            </div>
            <div>
              <p class="font-mono text-xs font-bold text-muted-foreground">Source</p>
              <p>{{ machineLabel(selected.source_machine_id) }}</p>
            </div>
            <div>
              <p class="font-mono text-xs font-bold text-muted-foreground">Collected</p>
              <p>{{ selected.collected_at ?? "-" }}</p>
            </div>
          </div>
          <div>
            <p class="font-mono text-xs font-bold text-muted-foreground">GPU signature</p>
            <p class="break-all font-mono text-xs">{{ selected.gpu_signature }}</p>
          </div>
          <button
            data-pso-file-distribute-btn
            class="w-full rounded-md bg-primary px-3 py-2 text-sm font-bold text-primary-foreground hover:bg-primary/90"
            @click="emit('distribute', selected)"
          >
            Distribute
          </button>
        </div>
        <p v-else class="p-4 text-sm text-muted-foreground">
          Select a file to inspect it.
        </p>
      </template>
    </UecmHorizontalSplit>
  </div>
</template>
