<script setup lang="ts">
import { useI18n } from "vue-i18n";
import UecmIcon from "./UecmIcon.vue";
import Button from "@/components/ui/Button.vue";
import { useLocale } from "@/composables/useLocale";
import type { Locale } from "@/locales";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";

const { t } = useI18n();
const { locale, setLocale } = useLocale();

const options: { value: Locale; label: string }[] = [
  { value: "en", label: "English" },
  { value: "zh", label: "中文" },
];
</script>

<template>
  <DropdownMenu>
    <DropdownMenuTrigger as-child>
      <Button variant="ghost" size="icon-sm" :aria-label="t('language.label')" data-language-toggle>
        <UecmIcon name="languages" />
      </Button>
    </DropdownMenuTrigger>
    <DropdownMenuContent align="end">
      <DropdownMenuItem
        v-for="option in options"
        :key="option.value"
        :data-active="locale === option.value ? 'true' : 'false'"
        @select="setLocale(option.value)"
      >
        <UecmIcon :name="locale === option.value ? 'check' : 'circle'" />
        {{ option.label }}
      </DropdownMenuItem>
    </DropdownMenuContent>
  </DropdownMenu>
</template>
