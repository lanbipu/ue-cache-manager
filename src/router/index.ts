import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import Dashboard from "@/views/Dashboard.vue";
import Machines from "@/views/Machines.vue";
import Projects from "@/views/Projects.vue";
import DDCPak from "@/views/DDCPak.vue";
import PSOCache from "@/views/PSOCache.vue";
import INIScanner from "@/views/INIScanner.vue";
import HealthCheck from "@/views/HealthCheck.vue";
import Shares from "@/views/Shares.vue";

export const routes: RouteRecordRaw[] = [
  { path: "/", name: "dashboard", component: Dashboard },
  { path: "/machines", name: "machines", component: Machines },
  { path: "/shares", name: "shares", component: Shares },
  { path: "/projects", name: "projects", component: Projects },
  { path: "/ddc-pak", name: "ddc-pak", component: DDCPak },
  { path: "/pso-cache", name: "pso-cache", component: PSOCache },
  { path: "/ini-scanner", name: "ini-scanner", component: INIScanner },
  { path: "/health-check", name: "health-check", component: HealthCheck },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
