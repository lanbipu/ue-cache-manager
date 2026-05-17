<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useMachinesStore } from "@/stores/machines";
import { useHealthCheckStore } from "@/stores/healthCheck";
import Button from "@/components/ui/Button.vue";
import UecmIcon from "@/components/primitives/UecmIcon.vue";
import UecmStatusDot from "@/components/primitives/UecmStatusDot.vue";
import UecmStateBlock from "@/components/primitives/UecmStateBlock.vue";
import MachineDetailTabs from "@/components/machines/MachineDetailTabs.vue";
import CommandPalette from "@/components/CommandPalette.vue";
import DiscoveryWizard from "@/components/modals/DiscoveryWizard.vue";
import CredentialDialog from "@/components/modals/CredentialDialog.vue";
import EnvVarConfigModal from "@/components/modals/EnvVarConfigModal.vue";
import IniEditModal from "@/components/modals/IniEditModal.vue";
import ShareCreateWizard from "@/components/modals/ShareCreateWizard.vue";
import BatchEnvVarModal from "@/components/modals/BatchEnvVarModal.vue";
import BatchIniEditModal from "@/components/modals/BatchIniEditModal.vue";

type StatusTone = "healthy" | "warning" | "critical" | "offline" | "unknown";

const { t } = useI18n();
const store = useMachinesStore();
const health = useHealthCheckStore();
const route = useRoute();
const router = useRouter();

const showDiscovery = ref(false);
const showCredentials = ref(false);
const showEnvVar = ref(false);
const showIniEdit = ref(false);
const showShareWizard = ref(false);
const showBatchEnv = ref(false);
const showBatchIni = ref(false);

const checkedIds = ref<Set<number>>(new Set());
const checkedArray = computed(() => Array.from(checkedIds.value));

function toggleCheck(id: number | null) {
  if (id == null) return;
  const next = new Set(checkedIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  checkedIds.value = next;
}

function clearSelection() {
  checkedIds.value = new Set();
}

const expandedId = computed<number | null>(() => {
  const raw = route.query.host;
  if (raw == null || Array.isArray(raw)) return null;
  const n = Number(raw);
  return Number.isFinite(n) ? n : null;
});

const expandedMachine = computed(() =>
  expandedId.value == null ? null : store.machines.find((m) => m.id === expandedId.value) ?? null,
);

const activeTab = computed(() => {
  const raw = route.query.tab;
  if (raw == null || Array.isArray(raw)) return "overview";
  return raw;
});

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && expandedId.value !== null) {
    closeDetail();
  }
}

// --- Detail outline (Figma v7 path) ---------------------------------------
// Article outline is a single SVG path: rounded rectangle + triangle bump
// pointing at the selected card. Geometry parameters come from the Figma
// design (file Kx953RiISgI99lIbWH7cPE, node 35:2).
const BUMP_HEIGHT = 22;                     // gap above article rect for triangle
const CORNER_R = 16;                        // rounded corner radius
const KAPPA = 0.5523 * CORNER_R;            // cubic-bezier kappa (~8.84)
const TRIANGLE_HALF_WIDTH = 35;             // base opening half-width
const DETAIL_PANEL_MAX_WIDTH = 1000;        // detail panel max width (px); ~0.6 of full frame

const gridRef = ref<HTMLUListElement | null>(null);
const detailLiRef = ref<HTMLLIElement | null>(null);
const detailContentEl = ref<HTMLElement | null>(null);
const arrowLeft = ref<number | null>(null);
const panelLeft = ref(0);
const articleSize = reactive({ w: 0, h: 0 });

function setDetailLi(el: unknown) {
  detailLiRef.value = (el as HTMLLIElement | null) ?? null;
}

function setDetailContentEl(el: unknown) {
  detailContentEl.value = (el as HTMLElement | null) ?? null;
}

function updateArticleSize() {
  const el = detailContentEl.value;
  if (el) {
    articleSize.w = el.offsetWidth;
    articleSize.h = el.offsetHeight;
  } else {
    articleSize.w = 0;
    articleSize.h = 0;
  }
}

function updateArrowPosition() {
  const li = detailLiRef.value;
  const grid = gridRef.value;
  const id = expandedId.value;
  if (!li || !grid || id == null) {
    arrowLeft.value = null;
    panelLeft.value = 0;
    return;
  }
  const card = grid.querySelector<HTMLElement>(
    `[data-machine-card][data-machine-id="${id}"]`,
  );
  if (!card) {
    arrowLeft.value = null;
    panelLeft.value = 0;
    return;
  }
  const cardRect = card.getBoundingClientRect();
  const liRect = li.getBoundingClientRect();
  const cardCenterX = cardRect.left + cardRect.width / 2 - liRect.left;
  const panelWidth = Math.min(DETAIL_PANEL_MAX_WIDTH, liRect.width);
  // Center panel on selected card, clamp to li bounds
  const rawLeft = cardCenterX - panelWidth / 2;
  panelLeft.value = Math.max(0, Math.min(liRect.width - panelWidth, rawLeft));
  // arrowLeft is cx relative to article's own coord system
  arrowLeft.value = cardCenterX - panelLeft.value;
}

const detailOutlinePath = computed(() => {
  const w = articleSize.w;
  const h = articleSize.h;
  if (w === 0) return "";
  const top = BUMP_HEIGHT;
  const bottom = BUMP_HEIGHT + h;
  const rawCx = arrowLeft.value ?? w / 2;
  // Clamp so triangle stays clear of the rounded corners
  const cx = Math.max(
    CORNER_R + TRIANGLE_HALF_WIDTH,
    Math.min(w - CORNER_R - TRIANGLE_HALF_WIDTH, rawCx),
  );
  return [
    `M ${CORNER_R} ${top}`,
    `L ${cx - 35} ${top}`,
    `C ${cx - 29.22} ${top} ${cx - 26.44} ${top - 1.99} ${cx - 20.24} ${top - 5.95}`,
    `L ${cx - 8} ${top - 14.46}`,
    `C ${cx - 2} ${top - 18.42} ${cx + 2} ${top - 18.42} ${cx + 8} ${top - 14.46}`,
    `L ${cx + 20.24} ${top - 5.95}`,
    `C ${cx + 26.44} ${top - 1.99} ${cx + 29.22} ${top} ${cx + 35} ${top}`,
    `L ${w - CORNER_R} ${top}`,
    `C ${w - CORNER_R + KAPPA} ${top} ${w} ${top + CORNER_R - KAPPA} ${w} ${top + CORNER_R}`,
    `L ${w} ${bottom - CORNER_R}`,
    `C ${w} ${bottom - CORNER_R + KAPPA} ${w - CORNER_R + KAPPA} ${bottom} ${w - CORNER_R} ${bottom}`,
    `L ${CORNER_R} ${bottom}`,
    `C ${CORNER_R - KAPPA} ${bottom} 0 ${bottom - CORNER_R + KAPPA} 0 ${bottom - CORNER_R}`,
    `L 0 ${top + CORNER_R}`,
    `C 0 ${top + CORNER_R - KAPPA} ${CORNER_R - KAPPA} ${top} ${CORNER_R} ${top}`,
    "Z",
  ].join(" ");
});

let resizeObserver: ResizeObserver | null = null;
let articleResizeObserver: ResizeObserver | null = null;

onMounted(() => {
  store.loadMachines();
  if (typeof window !== "undefined") {
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", updateArrowPosition);
    window.addEventListener("resize", updateArticleSize);
  }
  if (typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => updateArrowPosition());
  }
});

watch(
  expandedId,
  (id) => {
    if (id == null) {
      store.clearSelection();
    } else {
      store.selectMachine(id);
    }
  },
  { immediate: true },
);

watch(
  () => store.machines,
  (list) => {
    const validIds = new Set(
      list.map((m) => m.id).filter((id): id is number => id != null),
    );
    let changed = false;
    const next = new Set<number>();
    for (const id of checkedIds.value) {
      if (validIds.has(id)) next.add(id);
      else changed = true;
    }
    if (changed) checkedIds.value = next;
  },
  { deep: true },
);

watch(
  [
    expandedId,
    gridRef,
    detailLiRef,
    // join IDs so rename-induced reordering (same length, different positions)
    // still triggers arrow recalculation
    () => store.machines.map((m) => m.id ?? m.ip).join(","),
  ],
  async () => {
    await nextTick();
    if (resizeObserver) {
      resizeObserver.disconnect();
      if (gridRef.value) resizeObserver.observe(gridRef.value);
    }
    updateArrowPosition();
  },
);

watch(detailContentEl, async (el) => {
  articleResizeObserver?.disconnect();
  if (el && typeof ResizeObserver !== "undefined") {
    articleResizeObserver = new ResizeObserver(updateArticleSize);
    articleResizeObserver.observe(el);
    await nextTick();
    updateArticleSize();
  } else {
    articleSize.w = 0;
    articleSize.h = 0;
  }
});

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("keydown", onKey);
    window.removeEventListener("resize", updateArrowPosition);
    window.removeEventListener("resize", updateArticleSize);
  }
  resizeObserver?.disconnect();
  articleResizeObserver?.disconnect();
});

const online = computed(() =>
  store.machines.filter((m) => m.status.toLowerCase() === "online").length,
);
const critical = computed(() =>
  store.machines.filter((m) => ["critical", "error", "failed"].includes(m.status.toLowerCase())).length,
);
const warning = computed(() =>
  store.machines.filter((m) => ["warning", "warn", "degraded"].includes(m.status.toLowerCase())).length,
);
const healthScore = computed(() => {
  const total = health.summary.total;
  if (total === 0) return null;
  const raw =
    ((health.summary.healthy - health.summary.critical * 0.75 - health.summary.warning * 0.35) / total) *
    100;
  return Math.max(0, Math.round(raw));
});

function statusTone(status: string): StatusTone {
  const s = status.toLowerCase();
  if (s === "online" || s === "healthy") return "healthy";
  if (["critical", "error", "failed"].includes(s)) return "critical";
  if (["warning", "warn", "degraded"].includes(s)) return "warning";
  if (s === "offline") return "offline";
  return "unknown";
}

function onExpand(id: number | null) {
  if (id == null) return;
  if (id === expandedId.value) {
    closeDetail();
    return;
  }
  router.push({
    path: "/machines",
    query: { ...route.query, host: String(id), tab: activeTab.value },
  });
}

function closeDetail() {
  const next = { ...route.query };
  delete next.host;
  delete next.tab;
  router.push({ path: "/machines", query: next });
}
</script>

<template>
  <div class="flex h-full flex-col gap-6 overflow-auto p-6">
    <header class="flex flex-wrap items-end justify-between gap-4">
      <div class="min-w-0">
        <p class="text-xs font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {{ t("machines.eyebrow") }}
        </p>
        <h1 class="mt-1 font-display text-3xl font-extrabold">{{ t("machines.title") }}</h1>
        <p class="mt-1 max-w-prose text-sm text-muted-foreground">
          {{ t("machines.description") }}
        </p>
      </div>
      <Button data-discover-btn @click="showDiscovery = true">
        <UecmIcon name="radar" />
        {{ t("machines.scan") }}
      </Button>
    </header>

    <section
      data-cluster-summary
      class="flex flex-wrap items-center gap-x-6 gap-y-2 rounded-lg border bg-card px-4 py-3 text-sm"
    >
      <span class="flex items-center gap-2">
        <UecmStatusDot tone="healthy" />
        <span class="font-medium">
          {{ t("machines.summaryOnline", { online, total: store.machines.length }) }}
        </span>
      </span>
      <span class="flex items-center gap-2">
        <UecmStatusDot v-if="critical > 0" tone="critical" />
        <span :class="critical > 0 ? 'font-medium text-status-critical' : 'text-muted-foreground'">
          {{ t("machines.summaryCritical", { count: critical }) }}
        </span>
      </span>
      <span class="flex items-center gap-2">
        <UecmStatusDot v-if="warning > 0" tone="warning" />
        <span :class="warning > 0 ? 'font-medium text-status-warning' : 'text-muted-foreground'">
          {{ t("machines.summaryWarning", { count: warning }) }}
        </span>
      </span>
      <span class="ml-auto font-mono text-xs text-muted-foreground">
        {{ t("machines.summaryHealthScore", { score: healthScore ?? "—" }) }}
      </span>
    </section>

    <section
      v-if="checkedIds.size > 0"
      data-batch-bar
      class="flex flex-wrap items-center gap-2 rounded-lg border border-primary/30 bg-primary/5 px-4 py-2 text-sm"
    >
      <span class="font-medium">
        {{ t("machines.selectedCount", { count: checkedIds.size }) }}
      </span>
      <Button
        variant="outline"
        size="sm"
        data-batch-env-btn
        @click="showBatchEnv = true"
      >
        <UecmIcon name="terminal" size="14" />
        {{ t("machines.batchEnv") }}
      </Button>
      <Button
        variant="outline"
        size="sm"
        data-batch-ini-btn
        @click="showBatchIni = true"
      >
        <UecmIcon name="file-text" size="14" />
        {{ t("machines.batchIni") }}
      </Button>
      <button
        type="button"
        class="ml-auto text-xs text-muted-foreground hover:text-foreground"
        @click="clearSelection"
      >
        {{ t("machines.clearSelection") }}
      </button>
    </section>

    <section class="min-h-0 flex-1">
      <p
        v-if="store.isLoading && store.machines.length === 0"
        class="text-sm text-muted-foreground"
      >
        {{ t("common.loading") }}
      </p>
      <UecmStateBlock
        v-else-if="store.machines.length === 0"
        variant="empty"
        :title="t('machines.emptyTitle')"
        :message="t('machines.emptyMessage')"
      />
      <ul
        v-else
        ref="gridRef"
        data-machines-grid
        class="grid grid-flow-dense gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
      >
        <template v-for="machine in store.machines" :key="machine.id ?? machine.ip">
          <li
            data-machine-card
            :data-machine-id="machine.id ?? undefined"
            class="group/card relative"
          >
            <input
              v-if="machine.id != null"
              data-machine-check
              type="checkbox"
              :checked="checkedIds.has(machine.id)"
              :aria-label="t('machines.toggleSelect', { hostname: machine.hostname })"
              class="absolute left-3 top-3 z-10 size-4 cursor-pointer rounded border-input opacity-0 transition-opacity focus-visible:opacity-100 group-hover/card:opacity-100"
              :class="checkedIds.has(machine.id) ? 'opacity-100' : ''"
              @click.stop
              @change="toggleCheck(machine.id)"
            />
            <button
              type="button"
              class="flex aspect-square w-full flex-col items-center justify-center gap-3 rounded-xl border bg-card p-4 text-card-foreground transition-all hover:-translate-y-0.5 hover:border-primary/40 hover:bg-accent/30 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              :class="[
                machine.id === expandedId ? 'border-primary ring-2 ring-primary' : '',
                machine.id != null && checkedIds.has(machine.id) ? 'border-primary/60 bg-primary/5' : '',
              ]"
              :aria-label="machine.hostname"
              :aria-expanded="machine.id === expandedId"
              @click="onExpand(machine.id)"
            >
              <span class="relative">
                <UecmIcon
                  name="server"
                  size="64"
                  class="text-muted-foreground transition-colors group-hover/card:text-foreground"
                  :class="machine.id === expandedId ? 'text-foreground' : ''"
                />
                <span class="absolute -bottom-0.5 -right-0.5 rounded-full bg-card p-0.5">
                  <UecmStatusDot :tone="statusTone(machine.status)" />
                </span>
              </span>
              <span class="w-full truncate text-sm font-bold">{{ machine.hostname }}</span>
              <span class="font-mono text-xs text-muted-foreground">{{ machine.ip }}</span>
            </button>
          </li>
          <li
            v-if="machine.id === expandedId"
            :ref="setDetailLi"
            data-machine-detail
            class="relative col-start-1 col-span-full mt-2"
          >
            <svg
              v-if="articleSize.w > 0"
              aria-hidden="true"
              class="pointer-events-none absolute overflow-visible drop-shadow-lg"
              :style="{ top: `-${BUMP_HEIGHT}px`, left: `${panelLeft}px` }"
              :width="articleSize.w"
              :height="articleSize.h + BUMP_HEIGHT"
              :viewBox="`0 0 ${articleSize.w} ${articleSize.h + BUMP_HEIGHT}`"
            >
              <path
                :d="detailOutlinePath"
                class="fill-popover stroke-primary"
                stroke-width="2"
                stroke-linejoin="round"
              />
            </svg>
            <article
              :ref="setDetailContentEl"
              :style="{ marginLeft: `${panelLeft}px`, maxWidth: `${DETAIL_PANEL_MAX_WIDTH}px` }"
              class="relative flex max-h-[70vh] flex-col overflow-hidden pb-[2px] pl-[2px] pr-[2px] text-popover-foreground"
            >
              <header class="flex items-center justify-between gap-4 border-b px-6 py-4">
                <div class="flex min-w-0 items-center gap-3">
                  <span class="relative shrink-0">
                    <UecmIcon name="server" size="32" class="text-muted-foreground" />
                    <span class="absolute -bottom-0.5 -right-0.5 rounded-full bg-popover p-0.5">
                      <UecmStatusDot :tone="statusTone(machine.status)" />
                    </span>
                  </span>
                  <div class="min-w-0">
                    <h2 class="truncate font-display text-xl font-extrabold">
                      {{ machine.hostname }}
                    </h2>
                    <p class="font-mono text-xs text-muted-foreground">{{ machine.ip }}</p>
                  </div>
                </div>
                <button
                  type="button"
                  class="rounded-md p-2 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  :aria-label="t('common.close')"
                  @click="closeDetail"
                >
                  <UecmIcon name="x" size="20" />
                </button>
              </header>
              <MachineDetailTabs
                :machine="machine"
                @open-credential-modal="showCredentials = true"
                @open-env-var-modal="showEnvVar = true"
                @open-ini-edit-modal="showIniEdit = true"
                @open-share-wizard="showShareWizard = true"
                @close="closeDetail"
              />
            </article>
          </li>
        </template>
      </ul>
      <p v-if="store.error" class="mt-3 text-xs text-destructive">{{ store.error.message }}</p>
    </section>

    <CommandPalette
      @request-scan="showDiscovery = true"
      @request-create-share="showShareWizard = true"
      @request-batch-env="showBatchEnv = true"
      @request-batch-ini="showBatchIni = true"
    />

    <DiscoveryWizard :open="showDiscovery" @close="showDiscovery = false" />
    <CredentialDialog :open="showCredentials" @close="showCredentials = false" />
    <EnvVarConfigModal
      :open="showEnvVar"
      :machine-id="expandedId"
      var-name="UE-SharedDataCachePath"
      @close="showEnvVar = false"
    />
    <IniEditModal :open="showIniEdit" :machine-id="expandedId" @close="showIniEdit = false" />
    <ShareCreateWizard :open="showShareWizard" @close="showShareWizard = false" />
    <BatchEnvVarModal
      :open="showBatchEnv"
      :machine-ids="checkedArray"
      @close="showBatchEnv = false"
    />
    <BatchIniEditModal
      :open="showBatchIni"
      :machine-ids="checkedArray"
      @close="showBatchIni = false"
    />
  </div>
</template>
