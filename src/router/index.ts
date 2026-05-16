import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import Machines from "@/views/Machines.vue";

export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/machines" },
  { path: "/machines", name: "machines", component: Machines },
  {
    path: "/:pathMatch(.*)*",
    redirect: (to) => ({ path: "/machines", query: to.query }),
  },
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;
