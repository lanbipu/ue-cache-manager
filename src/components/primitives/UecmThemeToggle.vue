<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import UecmIcon from "./UecmIcon.vue";
import Button from "@/components/ui/Button.vue";
import { useColorMode, type ThemeMode } from "@/composables/useColorMode";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";

const { mode } = useColorMode();
const { t } = useI18n();

const options = computed<{ value: ThemeMode; label: string; icon: string }[]>(() => [
  { value: "dark", label: t("theme.dark"), icon: "moon" },
  { value: "light", label: t("theme.light"), icon: "sun" },
  { value: "system", label: t("theme.system"), icon: "monitor" },
]);
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button variant="ghost" size="icon-sm" :aria-label="t('theme.label')">
        <UecmIcon :name="mode === 'light' ? 'sun' : mode === 'system' ? 'monitor' : 'moon'" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">
      <DropdownMenuItem v-for="option in options" :key="option.value" @select="mode = option.value">
        <UecmIcon :name="option.icon" />
        {{ option.label }}
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
