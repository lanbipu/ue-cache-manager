# CLAUDE.md

## 远程通道选型（WinRM vs Listener Agent）

UECM 所有功能走 **WinRM pull 模式**（operator 触发的离散配置 / 短任务），不引入常驻 agent；好处是无升级负担、走 Windows 原生认证、动作可审计。仅当 DDC distribute / PSO collect 等长任务体验扛不住、要做 nDisplay-like 实时编排（参考 Switchboard Listener）、或需节点主动上报（UE 崩溃 / OOM / GPU 状态）时，才为对应域单独引入 lightweight Windows Service agent；配置类继续走 WinRM，两条通道并存。

## Plan 7 — Zen daemon integration（已结）

UECM ↔ Zen 全链路集成已合 main（PR #10，2026-05-20）。改 zen 相关代码前先读 `docs/zen-integration.md`（操作员手册）+ `docs/research/plan7-deferral-acceptance-2026-05-20.md`（22 轮 codex review 修过的设计 gap 表）；grep「Codex round N」可定位为什么某段代码长成现在这样，避免重复踩坑。

## Figma → 代码任务

接到任何「实现 Figma 设计」「把这个 Figma 文件做成代码」「按设计稿写组件」类任务时，**先读 `.claude/rules/figma-design-system.md`**——里面定义了：

- Figma MCP 工具调用顺序（`get_design_context` → `get_screenshot` → 实现）
- 组件三层结构（`primitives/Uecm*` / `ui/` / `feature 域`）放在哪儿
- 设计 token 用法（OKLCH 制式，全部走 Tailwind class，不能硬编码颜色）
- CVA + `cn()` 变体模式
- `@/*` 路径别名 / vue-i18n / dark mode 处理 / status tone 语义

不读直接动手大概率会写出违反约定的代码。

<!-- DOCSMITH:KNOWLEDGE:BEGIN -->
**任何新增功能从设计阶段就必须考虑 CLI 暴露**

## Knowledge Base (Managed by Docsmith)

- Knowledge entrypoint: `.claude/knowledge/_INDEX.md`
- Config file: `.claude/knowledge.json`

### Current Sources
- `ue50-docs` (292 files) → `.claude/knowledge/ue50-docs/`
- `ue51-docs` (284 files) → `.claude/knowledge/ue51-docs/`
- `ue52-docs` (333 files) → `.claude/knowledge/ue52-docs/`
- `ue53-docs` (29 files) → `.claude/knowledge/ue53-docs/`
- `ue54-docs` (321 files) → `.claude/knowledge/ue54-docs/`
- `ue55-docs` (324 files) → `.claude/knowledge/ue55-docs/`
- `ue56-docs` (389 files) → `.claude/knowledge/ue56-docs/`
- `ue57-docs` (411 files) → `.claude/knowledge/ue57-docs/`

### Query Protocol
1. Read `.claude/knowledge/_INDEX.md` to route to the relevant source.
2. Open `<source>/_INDEX.md` and shortlist target documents by `topic/summary/keywords`.
3. Read target file TL;DR first, then read full content when needed.
4. Before answering, prioritize evidence from `KnowledgeBase docs`; use external knowledge only when KB coverage is insufficient.
5. In every answer, include:
   - `Knowledge Sources`: exact KB document paths used.
   - `External Inputs`: non-KB knowledge used and why.
   - If no KB match: `No relevant KnowledgeBase docs found`.

### Refresh Command
```bash
.venv/bin/python -m cli --project-links --refresh-index .
```
<!-- DOCSMITH:KNOWLEDGE:END -->
