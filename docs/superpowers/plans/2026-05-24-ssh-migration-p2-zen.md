# SSH 迁移 P2 — zen 域迁 SSH(12 脚本 node-pure + plumbing) 实现计划

> REQUIRED SUB-SKILL: superpowers:executing-plans(inline)。对应 spec P2。提交 `git -c commit.gpgsign=false`。
> 前置:P0(lanPC 重 onboard)+ P1 完成。改 zen 前已读 `docs/zen-integration.md` + plan7 deferral 文档(via trace agent)。

**Goal:** 12 个传输用 `zen-*.ps1` 从 `param()`(经 WinRM `build_param_script`+`run_remote` 整体下发)改成 node-pure(`[Console]::In.ReadToEnd()|ConvertFrom-Json` + stdin JSON);Rust 侧 ~10 个 `run_remote` 调用站点 + `commands/zen.rs` 换 `ssh::run_json(NodeScript)`;删 `build_param_script`/`run_remote` + 其单测;长任务加 SSH keepalive;3 个孤儿脚本处置。不删 winrm.rs(P5)。

**Architecture:** 套 A2/A3 配方 + 5 条 PS5.1 真机坑。zen 脚本已被 SSH auto-staging 推到节点(非 enable-*),故 `-File` 可跑。命令签名(CLI + Tauri)对外不变。

---

## 通用配方(每个 zen 脚本)
1. 删 `param(...)` 块,改 prologue:
```powershell
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null
$ErrorActionPreference = 'Stop'   # 注意 5 坑:内层若依赖 Continue 则别全局 Stop
try {
    $p = [Console]::In.ReadToEnd() | ConvertFrom-Json
    $ServiceName = $p.ServiceName  # ... 按下表绑定;null-guard mandatory 字段
    # ... 原内层逻辑(去掉对 param 的依赖)...
    @{ ok = $true; ... } | ConvertTo-Json -Compress -Depth N
} catch { @{ ok=$false; message="$($_.Exception.Message)" } | ConvertTo-Json -Compress; exit 1 }
```
2. Rust 调用站点:`build_param_script(name, &[("K","V")]) + run_remote(body, creds, am)` → `ssh::run_json::<T>(&exec, host, &NodeScript{ name, args: serde_json::json!({"K":v}), ssh_user: None })`,`exec = SshExecutor::from_config()?`。operator creds/auth_method 忽略(P4 清)。
3. 每脚本 lanPC 真机抽验(`uecm-cli ssh probe` 已证 from_config 通;zen 命令真机跑 or `ssh uecm-svc@lanpc -File` 喂 stdin)。

## 每脚本 param → stdin JSON 映射
| 脚本 | stdin 字段(mandatory*) | 备注 |
|---|---|---|
| zen-detect-binary | (无) | 最简单,只加 prologue;args=`{}` |
| zen-down | ServiceName | 默认 ZenServer |
| zen-up | ServiceName | |
| zen-service-status | ServiceName | |
| zen-env-cleanup | Name*, Scopes(string[]) | Scopes 默认 ["machine","user"] |
| zen-urlacl-list | PortFilter | 可空 |
| zen-urlacl-add | UrlPrefix*, UserAccount* | |
| zen-urlacl-remove | UrlPrefix* | |
| zen-write-lua-config | LuaText*, DestPath* | LuaText 多行字符串 |
| zen-service-uninstall | ZenExePath*, ServiceName | 长任务(sc/zen) |
| zen-verify-rules | UeRoot*, UprojectPath*, TimeoutSeconds(300), ExpectedHost, ExpectedPort, ExpectedNamespace | **长任务**(编辑器≤300s)→ keepalive |
| zen-service-install | ZenExePath*, ServiceName, DataDir*, ServiceUser, **ServicePassword(secret)**, +更多 | **长任务 + secret**;ServicePassword 经 stdin(不上命令行/日志);非内置账号才需 |

## 子阶段(每子阶段收尾:cargo test + build + 真机抽验 + codex review)
- **P2a 只读/简单**:detect-binary、service-status、urlacl-list、down、up(状态/启停查询类)。先做 detect-binary 跑通整条 SSH 路径模式。
- **P2b mutating**:env-cleanup、urlacl-add、urlacl-remove、write-lua-config、service-uninstall。
- **P2c 复杂**:verify-rules(长任务 keepalive)、service-install(长任务 + secret)。
- **P2d plumbing 收尾**:删 `build_param_script`/`run_remote` + 其 2 单测;`commands/zen.rs:429` 的 winrm::invoke 换 ssh::run_json;grep 确认 zen 无 winrm::invoke。

## 长任务 keepalive(P2c 前置)
`core/ssh.rs build_ssh_args`:加 `-o ServerAliveInterval=30 -o ServerAliveCountMax=10`(允许长任务空闲不被 NAT/idle 断;connect 仍 ConnectTimeout=10)。更新 build_ssh_args 单测断言含 keepalive。

## 孤儿脚本(P2d)
`zen-probe-cache-stats.ps1`/`zen-probe-health.ps1`/`zen-read-lockfile.ps1`:0 Rust 引用,功能在 `core/zen/{probe,cache_stats}.rs` 走 reqwest HTTP。**删除**(确认 grep 全仓 0 引用后);若想留作 operator 手动调试则加注释标注「manual-debug, not wired」。

## 验收(对照 spec P2)
- 12 脚本 node-pure(`grep -L 'Console.*ReadToEnd' ps-scripts/zen-*.ps1` 仅剩孤儿/已删)。
- domain_zen + core/zen/verify + commands/zen 无 `winrm::invoke`(grep)。
- `build_param_script`/`run_remote` 删除。
- cargo test --lib 全绿;build green;codex 无 blocker。
- lanPC:`uecm-cli zen <子命令>` 抽验关键流(detect/status/up/down 至少)。

## 注意(5 坑 + zen 特有)
- `ConvertFrom-Json` 出的对象,string[](Scopes)字段保持数组;`$p.Scopes` 已是数组。
- mandatory 字段 null-guard:`if([string]::IsNullOrWhiteSpace($X)){throw "..."}`。
- ServicePassword:仅经 stdin,catch 里别把它拼进 message。
- verify-rules/service-install 长任务:keepalive + 真机长跑验证(单测覆盖不到)。
- 改 zen 前 grep「Codex round」看历史坑。
