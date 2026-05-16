# Figma → UECM Design System Rules

这些规则在所有「把 Figma 设计实现成代码」的任务中必须遵守。
项目栈：Vue 3 + TypeScript + Tauri 2 + Vite + Tailwind + `reka-ui` + CVA。

---

## 1. Required Figma Flow (顺序不能跳)

每次从 Figma 拿设计前，按这个顺序调用 MCP 工具：

1. **`get_design_context`** — 拉目标 node 的结构化表示（这是主输入，不能跳过）。
2. **响应太大或被截断** → 先 `get_metadata` 拿高层 node map，再用具体 node id 重新 `get_design_context`。
3. **`get_screenshot`** — 拿一张参考图，用于实现完了之后做视觉比对。
4. 只有同时拿到 `get_design_context` + `get_screenshot` 之后，才能开始下载资源 / 写代码。
5. 实现完成后必须对照截图做 1:1 校对（视觉 + 交互行为）。

变量 / token 导出用 `get_variable_defs`，组件库探查用 `search_design_system` / `get_libraries`。

URL 解析：`figma.com/design/:fileKey/:fileName?node-id=:nodeId` → `nodeId` 里的 `-` 要换成 `:`。

---

## 2. 代码翻译规则

Figma MCP 默认输出 **React + Tailwind**，但本项目是 **Vue 3 SFC**。把输出当成「设计意图的表达」而不是最终代码：

- 一律翻成 `<script setup lang="ts">` 的 Vue 3 SFC。
- React 的 `className` → Vue 的 `class` / `:class`。
- React 的 `useState` / `useEffect` → Vue 的 `ref` / `computed` / `watch` / `onMounted`。
- 表单 / 弹层 / Menu 等带交互的部件，优先用 `reka-ui` 的 primitive 包装（项目已经依赖 `reka-ui ^2.9.6`）。
- 共享逻辑放 `src/composables/`，命名 `useXxx.ts`，参考 `useColorMode`。

---

## 3. 组件放在哪儿 (强制)

| 层 | 路径 | 用途 | 命名 |
|---|---|---|---|
| **Primitives** | `src/components/primitives/` | UECM 业务原子件（KPI、状态徽章、进度条、KV 行…） | **必须 `Uecm` 前缀**，PascalCase（如 `UecmKpiTile.vue`） |
| **UI base** | `src/components/ui/` | shadcn 风格通用件（按钮、输入框、Dropdown…） | PascalCase，无前缀（如 `Button.vue`） |
| **Modals / Wizards** | `src/components/modals/` | 所有模态 / 向导 | PascalCase + `Modal` / `Wizard` 后缀 |
| **Shell** | `src/components/shell/` | 应用骨架（`AppShell`、`UecmSidebar`、`UecmTopBar`、`UecmLogPanel`） | — |
| **Feature 域** | `src/components/{batch,ddcpak,machines,diagnostics,pso}/` | 业务功能组件按域分文件夹 | PascalCase |
| **Views (页面)** | `src/views/` | 路由级页面，对应路由 entry | PascalCase（如 `Machines.vue`） |

**重要**：

- 新原子件必须导出到 `src/components/primitives/index.ts`（barrel）。
- 新 `ui/` 组件如果有多个相关 part（如 `dropdown-menu/` 里多个文件），放子文件夹 + `index.ts` barrel。
- **永远先 grep / 浏览 `primitives/` 和 `ui/` 看有没有现成的**再决定要不要新建。重复的视觉模式（KPI 卡、状态徽章、进度条、KV 列表、code block）几乎都有现成的，直接复用。

---

## 4. Design Tokens (绝对不能硬编码)

所有 token 定义在 `src/styles/tokens.css`，存的是**裸 OKLCH 分量**（`L C H`，不带 `oklch(...)`），由 `tailwind.config.js` 用 `oklch(var(--xxx) / <alpha-value>)` 包装，所以 Tailwind alpha 修饰符（`bg-primary/90`、`bg-muted/40`）都能用。

### 颜色（用 Tailwind class，**不要写 hex / rgb / 裸 oklch**）

| Tailwind class | 用途 |
|---|---|
| `bg-background` / `text-foreground` | 页面底色 + 主文字 |
| `bg-card` / `text-card-foreground` | 卡片 |
| `bg-popover` / `text-popover-foreground` | 弹层 / Dropdown 内容 |
| `bg-primary` / `text-primary-foreground` | 主操作色 |
| `bg-secondary` / `text-secondary-foreground` | 次操作色 |
| `bg-muted` / `text-muted-foreground` | 弱化区块 / 次要文字 |
| `bg-accent` / `text-accent-foreground` | hover / 高亮 |
| `bg-destructive` / `text-destructive-foreground` | 危险操作 |
| `border` / `border-input` / `ring-ring` | 边框 / input 边框 / focus ring |
| `bg-status-{healthy,warning,critical,info,offline,unknown}` | 状态色（带 `online` / `warn` 别名） |
| `bg-sidebar*` | 侧边栏专用 |
| `bg-surface{,-raised,-subtle,-inverse}` | 表面层语义别名 |

**Sidebar 必须用 `sidebar*` 系列，不要混用普通颜色**——侧边栏在 dark 模式有独立调色。

### 字体

- `font-sans`（系统栈）—— body / 普通正文。
- `font-display`（Manrope variable）—— 大标题、KPI 数字、按钮（`Button.vue` 已经用了）。
- `font-mono` —— 代码块、shell 输出、等宽场景。
- 中文 / CJK 内容**永远不要用 `font-mono`**，Manrope / mono 没有 CJK 字符会触发逐字回退，行高字宽全乱。

### 圆角

`rounded-{sm,DEFAULT,md,lg,xl}` 全部基于 `--radius` 0.5rem 计算，**用 Tailwind class 就好**，不要直接写 `border-radius: 8px`。

### 间距

按 Tailwind 默认 4px scale（`gap-2` = 8px、`p-4` = 16px、`mt-3` = 12px）。Figma 里如果设计稿用 6px / 10px / 14px，就近取 8 / 12 / 16，不要为了完全贴合塞奇数值。

### Dark Mode

`darkMode: "class"`，切换由 `useColorMode` composable 在根 `<html>` 加 `.dark` 类。**所有颜色都用 token，不要写 `dark:` 变体**——token 已经在 `.dark` 里换过一遍了，写 `dark:bg-...` 会双重切换出 bug。状态色 / 状态徽章同理。

---

## 5. CVA + Variants 模式

需要 variant / size 的组件用 `class-variance-authority`（参考 `src/components/ui/Button.vue`）：

```ts
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const fooVariants = cva("base classes here", {
  variants: {
    variant: { default: "...", destructive: "..." },
    size: { default: "h-9 px-4", sm: "h-9 px-3 text-xs" },
  },
  defaultVariants: { variant: "default", size: "default" },
});
```

- class 合并永远用 `cn()` from `@/lib/utils`（基于 `twMerge` + `clsx`），不要自己拼字符串。
- 暴露 `class?: string` prop 给消费者覆盖，`cn(fooVariants(...), props.class)` 保证消费者覆盖能赢。
- 需要透传 ref / 行为时，用 `reka-ui` 的 `Primitive` + `asChild` / `as` props。

---

## 6. Status / Tone 语义

业务里凡是要表达「健康度」「告警」「状态」，**用 `UecmStatusBadge` / `UecmStatusDot` + `tone` prop**，不要直接 `bg-red-500`：

合法 `tone`（见 `src/components/primitives/types.ts`）：
`healthy` / `warning` / `critical` / `info` / `offline` / `unknown` / `progress` / `na`

颜色映射会自动选 `--status-*` token，dark 模式自动切。

---

## 7. 路径别名 & 导入

- `@/*` → `src/*`（已在 `tsconfig.app.json` 和 `vite.config.ts` 配置）。
- **绝对禁止相对路径跨级**（`../../components/...`），一律用 `@/components/...`。
- 同目录用 `./Foo.vue`、`./types` 这种相对路径是允许的。
- import 顺序：Vue / 第三方 → `@/` 内部 → 同目录 `./` → 类型 import 单独成组（`import type { X } from "..."`）。

---

## 8. 国际化 (i18n)

- 任何**面向用户的字面文本**都不能硬编码，必须走 `vue-i18n`：
  ```vue
  <script setup lang="ts">
  import { useI18n } from "vue-i18n";
  const { t } = useI18n();
  </script>
  <template>
    <button>{{ t("common.save") }}</button>
  </template>
  ```
- 翻译键加在 `src/locales/` 里对应的 locale 文件，所有 locale 同步更新（不要只加 zh 不加 en）。
- 按钮 / aria-label / placeholder / 错误信息**全部要 i18n**。

---

## 9. 资源 / 图标

- 图标统一用 `UecmIcon`，传 `name` 字符串（icon 名约定见 `UecmIcon.vue` 内部 mapping）。**不要新装图标库**（不要 `lucide-vue-next`、`@iconify` 等）。
- 如果 Figma 给的 icon 在 `UecmIcon` 里没有，先扩展 `UecmIcon` 的 mapping，再使用。
- **重要**：Figma MCP 返回的 localhost asset URL 直接用，**不要换成 placeholder、不要本地复制**。
- 真的需要落地的静态资源（字体、图片）放 `src/assets/`。

---

## 10. 校验清单（提交前必跑）

- [ ] 没有硬编码颜色 / hex / rgb / 裸 oklch（全部走 Tailwind token class）。
- [ ] 没有写 `dark:` 变体覆盖颜色（token 已经处理）。
- [ ] 没有跨级相对路径 `../`。
- [ ] 所有面向用户的字符串走 `t("...")`。
- [ ] 新原子件已导出到 `primitives/index.ts`。
- [ ] 中文 / CJK 内容没用 `font-mono`。
- [ ] 视觉对照 Figma `get_screenshot` 输出，1:1 校对。
- [ ] `pnpm typecheck` 不报错。
- [ ] `pnpm test` 通过。

---

## 11. 反例 (绝对不要这么写)

```vue
<!-- ❌ 硬编码颜色 -->
<div class="bg-[#3B82F6] text-white"></div>

<!-- ✅ 用 token -->
<div class="bg-primary text-primary-foreground"></div>
```

```vue
<!-- ❌ 双重 dark mode -->
<div class="bg-white dark:bg-zinc-900"></div>

<!-- ✅ token 已处理 dark -->
<div class="bg-background"></div>
```

```vue
<!-- ❌ 跨级相对路径 -->
import Foo from "../../components/ui/Foo.vue";

<!-- ✅ alias -->
import Foo from "@/components/ui/Foo.vue";
```

```vue
<!-- ❌ 硬编码字面 -->
<button>保存</button>

<!-- ✅ i18n -->
<button>{{ t("common.save") }}</button>
```

```vue
<!-- ❌ 自己拼 class 字符串 -->
:class="`p-2 ${active ? 'bg-primary' : ''}`"

<!-- ✅ cn() -->
:class="cn('p-2', active && 'bg-primary')"
```
