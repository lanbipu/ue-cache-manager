<script setup lang="ts">
import { ref, nextTick } from "vue";

const props = defineProps<{ value: string }>();
const emit = defineEmits<{ (e: "save", v: string): void }>();

const editing = ref(false);
const draft = ref(props.value);
const inputEl = ref<HTMLInputElement | null>(null);

async function startEdit() {
  draft.value = props.value;
  editing.value = true;
  await nextTick();
  inputEl.value?.focus();
  inputEl.value?.select();
}

function cancel() {
  // Restore the draft to props.value before exiting edit mode so the
  // browser's blur->commit chain (which fires after Escape unfocuses the
  // input) runs but emits nothing — `trimmed === props.value` short-circuits.
  draft.value = props.value;
  editing.value = false;
}

function commit() {
  const trimmed = draft.value.trim();
  if (trimmed && trimmed !== props.value) {
    emit("save", trimmed);
  }
  editing.value = false;
}
</script>

<template>
  <span
    v-if="!editing"
    data-hostname-display
    class="cursor-pointer hover:underline"
    @click="startEdit"
  >
    {{ value }}
  </span>
  <input
    v-else
    ref="inputEl"
    v-model="draft"
    data-hostname-input
    class="text-xl font-semibold border rounded px-2 py-0.5"
    @keyup.enter="commit"
    @keyup.escape="cancel"
    @blur="commit"
  />
</template>
