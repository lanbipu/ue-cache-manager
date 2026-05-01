<script setup lang="ts">
import { RouterLink } from "vue-router";

interface NavItem {
  to: string;
  label: string;
}

const items: NavItem[] = [
  { to: "/", label: "Dashboard" },
  { to: "/machines", label: "Machines" },
  { to: "/projects", label: "Projects" },
  { to: "/ddc-pak", label: "DDC Pak" },
  { to: "/pso-cache", label: "PSO Cache" },
  { to: "/ini-scanner", label: "INI Scanner" },
  { to: "/health-check", label: "Health Check" },
];
</script>

<template>
  <nav class="flex flex-col w-48 border-r p-2 gap-1">
    <RouterLink
      v-for="item in items"
      :key="item.to"
      :to="item.to"
      data-nav-item
      class="px-3 py-2 rounded text-sm hover:bg-gray-100"
      active-class="bg-gray-200 font-medium"
    >
      {{ item.label }}
    </RouterLink>
  </nav>
</template>
