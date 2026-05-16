# Product

## Register

product

## Users

UECM 的主要使用者是 VP/XR 渲染集群里的 **TD（Technical Director）/ IT 运维**。他们：

- 同时负责 5–20 台 Windows render node 的 UE 工程冷启动 + 运行时性能
- 熟悉 PowerShell、WinRM、注册表、UE 引擎内部机制（DDC、PSO、Shader pipeline）
- 直接对接拍摄现场，单次操作失败 = 直播 / 实拍 stuck
- 常在拍摄前的紧张时间窗里使用 UECM，一边看屏幕一边和导演 / DP 沟通

工作场景：在 LED 屏直播 / 虚拟制片棚里，操作机往往是一台 27" 显示器的 workstation，环境光偏暗、操作员同时要响应现场通讯。屏幕上要"一眼能看到不对的地方"，而不是"得自己拼出整体状态"。

## Product Purpose

UECM 把 UE 工程在多机集群上的"零编译启动 + 零卡顿运行"配置集中化、自动化、可视化。

具体覆盖三层：

- **DDC（Derived Data Cache）**：消除 Shader 重编译，让所有机器开工程时直接读共享缓存
- **PSO Precaching**：UE 5.1+ 加载关卡时主动预创建 GPU pipeline state，消除场景切换卡顿
- **PSO Cache 文件**：兜底 Precaching 漏掉的边缘组合

成功标准：操作员能在一个 UI 里看清集群健康度、定位失效配置、一键修复并验证生效；新机器接入的人力 cost 从 30 分钟降到 5 分钟以内。

## Brand Personality

calm · trustworthy · production-safe

UECM 要让操作员相信"这个工具不会替我做出我没看见的决定"。不喊话、不抢戏、不堆 metric。每次破坏性操作（改注册表、改凭据、改 INI）都让用户先看到影响范围和 backup 路径，再确认。

视觉语言上，向 macOS System Settings + Vercel / Linear 学习：宽松留白、清晰分组、原生质感、克制的颜色。状态色（healthy / warning / critical）是"信息"而不是"告警"；出现得克制，出现时含义明确。

## Anti-references

UECM 明确不能变成下列任何一类：

- **典型企业 IT 工具的密集表格**：Windows Server Manager / VMware vCenter / vSphere 那种"信息全堆出来等你自己扫"的 Bootstrap-shaped 界面。UECM 是给现代 TD 用的，不是给 2010 年的 sysadmin 用的。
- **Observability dashboard 套路**：Grafana / Datadog 那种黑底 + neon 配色 + 十个不同色 metric 卡的"NOC 控制室"风。UECM 不是监控平台，不需要把所有数字摆出来。
- **游戏化 / 消费级硬件 UI**：Razer Synapse / NVIDIA GeForce Experience 那种番茄色 CTA + 大圆角 + 渐变魔手风。UECM 不是发烧友工具。
- **通用 SaaS 模板**：Notion / ClickUp 那种柔和渐变 hero 卡 + emoji icon + "三个 hero metric"。这是产品向的工具，不是营销页。

## Design Principles

1. **Native confidence, not dashboard theater**：不做 metric 剧场。每个屏幕只回答一个清晰的问题（"这台机器活着吗？" "这个 INI 配置对吗？" "集群健康吗？"）。
2. **Operations carry weight**：改注册表 / 改凭据 / 改 INI 是有重量的动作。UI 要让破坏性操作"自带摩擦"，先看到 diff、看到影响的机器列表、看到 backup 路径。
3. **Status is information, not alarm**：状态色（healthy / warning / critical）克制使用，单条信息只用一种颜色，避免"七彩斑马"。颜色之外用图标 + 文字双通道。
4. **Practiced operator, not first-time visitor**：假设用户已经知道 DDC / PSO 是什么，但 UECM 内部约定（Mode A/B 共享、SYSTEM cred、cmdkey 注入）需要解释清楚，而且要解释在每次决定的入口处，不藏在文档里。
5. **Show the cluster, not just the screen**：UECM 的核心价值是"看 N 台机器的一致性"。任何视图都应该让"全集群"和"单台"两个层次都好读，并且让两者之间的跳转是顺滑的。

## Accessibility & Inclusion

- **WCAG AA** 为默认目标：颜色对比、焦点可见、键盘可达。
- **i18n**：中文 / 英文双语，由 vue-i18n 驱动；所有用户可见字符串走 i18n 资源文件，不硬编码。中文排版遵循全局 CJK 规则（sans-serif 优先、不加负 letter-spacing、放宽行高）。
- **Theme**：light + dark 双主题，由 OKLCH token 驱动，已有完整 token 系统（参 `src/styles/tokens.css`）。新组件必须双主题验证后再合入。
- **Status color 必须有图标 / 文字双通道**，避免对色盲用户单凭颜色判断 healthy / critical。
