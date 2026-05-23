# UECM SSH 传输重构 · Phase A2（余下只读域）实施计划

> **For agentic workers:** 用 superpowers:executing-plans 逐 task 执行。提交用 `git -c commit.gpgsign=false`（1Password 签名锁会拦）。每域收尾在 lanPC 真节点验证（mac 无真节点）。
>
> 前置：A0/A1 已完成验证，A2 的 `SshExecutor::from_config()` + discovery 域已切（见 `2026-05-23-ssh-transport-phase-a2-...` 提交 + memory `ssh-a0-validated`）。spec §5/§11-A2。

**Goal:** 把余下 8 个只读诊断域的 self-remoting sidecar 重构成节点纯脚本，调用方从 `powershell::run_json`（本机自带远程）切到 `ssh::run_json`（经 SSH 在节点跑 `-File`）。

**Architecture:** 每个 sidecar 现在是「`param($HostName,$Username,$Password)` + `$script = {…节点逻辑…}` + `Invoke-Command -ComputerName $HostName -ScriptBlock $script` + JSON emit」。统一去掉远程包装，让内层块直接在节点本地跑、入参经 stdin JSON 收。调用方 `fn(host, creds)` → `fn(exec: &dyn RemoteExecutor, host)`，creds 消失（SSH key 认证）。SSH/WinRM 并存（A5 才删 winrm）。

**Tech Stack:** PowerShell 5.1（节点脚本）、Rust（core + cli + commands）、系统 ssh、lanPC 真节点。

---

## 迁移配方（Migration Recipe）—— 所有域共用，先读懂这一节

### A. 节点纯脚本改写（两种情况）

**情况 1：内层块无参数**（`ArgumentList=0`，如 command-line-scan / consistency / renderstream）。

改写规则：删 `param($HostName,...)` 整块、删 `Build-CredentialOrNull`、删 `Invoke-Command` 包装；把 `$script = { … }` 的**花括号内内容**原样提到顶层（去一层缩进）直接跑；保留 prologue + 末尾 JSON emit + try/catch。

完整样板（`scan-command-line-args.ps1` 改写后）：

```powershell
# Scans shortcuts / bat / services for -LocalDataCachePath= / -SharedDataCachePath=.
# Node-pure: runs locally on the target (shipped + executed via SSH -File).
# Output: JSON { ok, findings: [...] }
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null
$ErrorActionPreference = 'Stop'
try {
    function MatchArgs($cmd) {
        $out = @{}
        $patterns = @{
            local  = '-LocalDataCachePath=("[^"]+"|[^\s]+)'
            shared = '-SharedDataCachePath=("[^"]+"|[^\s]+)'
        }
        foreach ($k in $patterns.Keys) {
            $m = [regex]::Match($cmd, $patterns[$k], 'IgnoreCase')
            if ($m.Success) { $out[$k] = ($m.Groups[1].Value).Trim('"') }
        }
        $out
    }
    $findings = New-Object System.Collections.Generic.List[object]
    # ... (内层块其余逻辑原样，去一层缩进) ...
    @{ ok = $true; findings = @($findings) } | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; message = $_.Exception.Message; findings = @() } | ConvertTo-Json -Compress
    exit 1
}
```

**情况 2：内层块有参数**（`ArgumentList=N`，如 read-ini-file / health / project / ddc-file-stats / ue-log）。

额外规则：内层 `$script` 的 `param($A,$B)` → prologue 读 stdin JSON 绑定；用 `$p.A` / `$p.B` 取值。

完整样板（`read-ini-file.ps1` 改写后，原内层 `param($FilePath)`）：

```powershell
# Reads an INI file's sections + keys. Node-pure (SSH -File). Args via stdin JSON.
# stdin: { "FilePath": "<path>" }   Output: JSON { ok, found, sections, message }
[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; chcp 65001 | Out-Null
$ErrorActionPreference = 'Stop'
try {
    $p = [Console]::In.ReadToEnd() | ConvertFrom-Json
    $FilePath = $p.FilePath
    if (-not (Test-Path $FilePath)) {
        @{ ok = $true; found = $false; sections = @(); message = "" } | ConvertTo-Json -Compress -Depth 6
        return
    }
    $lines = Get-Content -Path $FilePath -Encoding UTF8
    $sections = New-Object System.Collections.ArrayList
    # ... (内层块其余逻辑原样，去一层缩进；把 return @{found;sections} 改成下面的 emit) ...
    @{ ok = $true; found = $true; sections = @($sections); message = "" } | ConvertTo-Json -Compress -Depth 6
}
catch {
    @{ ok = $false; found = $false; sections = @(); message = $_.Exception.Message } | ConvertTo-Json -Compress
    exit 1
}
```

**JSON key = 内层 `param()` 名**：改某域脚本时，读它内层 `$script` 的 `param(...)` 拿确切参数名，stdin JSON 用同名 key，节点脚本里 `$p.<名>` 取。

### B. 调用方改写（Rust）

`powershell::run_json(script_path, &args)` → `ssh::run_json(exec, host, &NodeScript{ name, args, ssh_user: None })`：

- 函数签名加 `exec: &dyn crate::core::ssh::RemoteExecutor` 作首参；删 `creds: Option<(&str,&str)>` 参数。
- `NodeScript.args`：无参域用 `serde_json::json!({})`；带参域用 `serde_json::json!({ "<内层param名>": <值> })`。
- 保留 `loopback::is_loopback_target(host)` 本地分支（若该 fn 原本有），逻辑不变。
- 错误经 discovery 同款 onboarding 提示更友好（可选；见 discovery.rs `with_onboarding_hint`，按需复用）。

### C. 调用方的上游

每个上游调用点（cli/commands）：构造 `let exec = crate::core::ssh::SshExecutor::from_config()?;` 一次，把 `&exec` 传进去；删原来的 cred 分支 / `--user/--pass/--cred-alias` 传参（CLI flag 本身留到 A5 清，这里只是不再传给 detect/probe 类）。

### D. 每域收尾（验证）

1. fake-executor 单测（参考 discovery.rs：`FakeExec` 返回预置 JSON → 断言解析）。
2. `cargo test --lib core::<domain>::` 通过 + `cargo test --lib` 全绿（零回归）。
3. **lanPC 真节点验证**：scp 改写后的脚本到 `C:\ProgramData\UECM\ps-scripts\`，加 `#[ignore]` 集成测试（env 驱动，参考 discovery `it_detect_against_real_node`）跑通。

---

## 任务（按简单→复杂排序）

> 无 stdin 参数的 3 个域最简单（情况 1），先做。每个 task 的步骤 = 配方 A/B/C/D 套用到该域。

### Task 1：command-line-scan 域（无参，配方情况 1）

**Files:** `ps-scripts/scan-command-line-args.ps1`、`src-tauri/src/core/command_line_scanner.rs`、调用方 `src-tauri/src/cli/domain_health.rs:81`

- [ ] **Step 1:** 按配方 A 情况 1 把 `scan-command-line-args.ps1` 改成节点纯（样板已在配方 A 给全，直接用）。
- [ ] **Step 2:** `command_line_scanner.rs`：`scan(host, creds)` → `scan(exec: &dyn RemoteExecutor, host)`；body 改 `ssh::run_json(exec, host, &NodeScript{ name:"scan-command-line-args.ps1", args: serde_json::json!({}), ssh_user: None })`；`ok==false` 分支保留。删 `#[cfg(not(windows))] returns_powershell_error_off_windows` 测试，换成 FakeExec 解析测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ssh::{NodeScript, ProbeResult, RemoteExecutor, ScriptOutput};
    struct FakeExec(String);
    impl RemoteExecutor for FakeExec {
        fn run(&self, _h: &str, _s: &NodeScript) -> UecmResult<ScriptOutput> {
            Ok(ScriptOutput { stdout: self.0.clone(), stderr: String::new(), exit_code: 0 })
        }
        fn probe(&self, _h: &str, _u: Option<&str>) -> UecmResult<ProbeResult> { unreachable!() }
    }
    #[test]
    fn scan_parses_findings() {
        let exec = FakeExec(r#"{"ok":true,"findings":[{"source":"service","path":"x","matches":{"local":"D:\\DDC"}}]}"#.to_string());
        let hits = scan(&exec, "RENDER-01").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source, "service");
    }
}
```
- [ ] **Step 3:** 改上游 `cli/domain_health.rs:81`：构造 `let exec = crate::core::ssh::SshExecutor::from_config()?;`，`scan(&exec, host)`，删 cred 传参。
- [ ] **Step 4:** `cargo test --lib core::command_line_scanner::` + `cargo test --lib` 全绿。
- [ ] **Step 5:** lanPC：scp 脚本 + `#[ignore]` 集成跑通。
- [ ] **Step 6:** `git -c commit.gpgsign=false commit -m "feat(command-line-scan): migrate to SSH node-pure script"`

### Task 2：consistency 域（无参，配方情况 1）

**Files:** `ps-scripts/consistency-snapshot.ps1`、`src-tauri/src/core/consistency_check.rs`、调用方 `cli/domain_health.rs:66`、`commands/consistency.rs:28`

- [ ] **Step 1:** 按配方 A 情况 1 改写 `consistency-snapshot.ps1`（内层 `$script` 提顶层，emit `@{ ok=$true; ... } | ConvertTo-Json`）。
- [ ] **Step 2:** `consistency_check::snapshot(host, creds)` → `snapshot(exec: &dyn RemoteExecutor, host)`，body 走 `ssh::run_json(exec, host, &NodeScript{ name:"consistency-snapshot.ps1", args: serde_json::json!({}), ssh_user: None })`；加 FakeExec 解析测试（仿 Task1 Step2，断言 `HostSnapshot` 字段）。
- [ ] **Step 3:** 改两处上游（`cli/domain_health.rs:66`、`commands/consistency.rs:28`）：各构造 exec、传 `&exec`、删 cred。
- [ ] **Step 4:** `cargo test --lib core::consistency_check::` + 全绿。
- [ ] **Step 5:** lanPC 集成验证。
- [ ] **Step 6:** commit `feat(consistency): migrate to SSH node-pure script`。

### Task 3：renderstream 域（无参，配方情况 1）

**Files:** `ps-scripts/probe-renderstream-service.ps1`、`src-tauri/src/core/renderstream_service.rs`、调用方 `cli/domain_health.rs:458`、`commands/health_check.rs:232`

- [ ] **Step 1:** 配方 A 情况 1 改写 `probe-renderstream-service.ps1`。
- [ ] **Step 2:** `renderstream_service::report(host, creds)` → `report(exec, host)`，走 ssh::run_json（`name:"probe-renderstream-service.ps1"`, args `{}`）；FakeExec 测试断言 `RsServiceReport`。
- [ ] **Step 3:** 改两处上游（`cli/domain_health.rs:458`、`commands/health_check.rs:232`）。
- [ ] **Step 4:** 测试全绿。
- [ ] **Step 5:** lanPC 集成验证。
- [ ] **Step 6:** commit `feat(renderstream): migrate to SSH node-pure script`。

### Task 4：ini-read 域（带参，配方情况 2）

**Files:** `ps-scripts/read-ini-file.ps1`、`src-tauri/src/core/ini_scanner.rs:78`、调用方 `cli/domain_ini.rs:294`、`cli/domain_ini.rs:334`、`commands/health_check.rs:356`

- [ ] **Step 1:** 按配方 A 情况 2 改写 `read-ini-file.ps1`（样板已在配方 A 给全：stdin `{FilePath}` → `$p.FilePath`）。
- [ ] **Step 2:** `ini_scanner::read_file(host, target, cred)` → `read_file(exec: &dyn RemoteExecutor, host, target)`：**保留** `loopback::is_loopback_target(host)` 本地分支；远程分支改 `ssh::run_json(exec, host, &NodeScript{ name:"read-ini-file.ps1", args: serde_json::json!({ "FilePath": target.path }), ssh_user: None })`；`ok/found` 处理不变。加 FakeExec 测试（断言 sections 解析）。
- [ ] **Step 3:** 改三处上游（`cli/domain_ini.rs:294`、`:334`、`commands/health_check.rs:356`）：构造 exec、传 `&exec`、删 cred。
- [ ] **Step 4:** `cargo test --lib core::ini_scanner::` + 全绿。
- [ ] **Step 5:** lanPC 集成验证（节点上放一个测试 .ini，读回）。
- [ ] **Step 6:** commit `feat(ini-read): migrate read_file to SSH node-pure script`。

### Task 5：project 域（带参，配方情况 2）

**Files:** `ps-scripts/discover-uprojects.ps1`、`src-tauri/src/core/project_discovery.rs:37`、调用方 `cli/domain_project.rs:79`、`commands/projects.rs:70`

- [ ] **Step 1:** 配方 A 情况 2 改写 `discover-uprojects.ps1`：读内层 `$script` 的 `param(...)`（项目根路径数组），stdin JSON 用同名 key，节点脚本 `$p.<名>` 取。
- [ ] **Step 2:** `project_discovery::run_discovery(...)` 加 `exec` 首参、删 cred，走 ssh::run_json（`name:"discover-uprojects.ps1"`，args 为内层 param 对应的 JSON）；FakeExec 测试断言 `DiscoveryResult`。
- [ ] **Step 3:** 改两处上游（`cli/domain_project.rs:79`、`commands/projects.rs:70`）。
- [ ] **Step 4:** 测试全绿。
- [ ] **Step 5:** lanPC 集成验证。
- [ ] **Step 6:** commit `feat(project): migrate discovery to SSH node-pure script`。

### Task 6：ddc-file-stats 域（带参 ×2，配方情况 2）

**Files:** `ps-scripts/ddc-file-stats.ps1`、`src-tauri/src/core/ddc_file_stats.rs:28`、调用方 `cli/domain_health.rs:91`、`cli/domain_health.rs:113`

- [ ] **Step 1:** 配方 A 情况 2 改写 `ddc-file-stats.ps1`（内层 `param` 2 个 → stdin JSON 2 个 key）。
- [ ] **Step 2:** `ddc_file_stats::run(...)` 加 exec 首参、删 cred，走 ssh::run_json（args 为 2 个 param 对应 JSON）；FakeExec 测试断言 `Stats`。
- [ ] **Step 3:** 改两处上游（`cli/domain_health.rs:91`、`:113`）。
- [ ] **Step 4:** 测试全绿。
- [ ] **Step 5:** lanPC 集成验证。
- [ ] **Step 6:** commit `feat(ddc-file-stats): migrate to SSH node-pure script`。

### Task 7：health 域（带参，配方情况 2，最大脚本 295 行）

**Files:** `ps-scripts/health-probes.ps1`、`src-tauri/src/core/health_probes.rs:18`、调用方 `cli/domain_health.rs:291`、`commands/health_check.rs:135`

- [ ] **Step 1:** 配方 A 情况 2 改写 `health-probes.ps1`：内层 `$script param(...)` → stdin JSON。脚本大（295 行）但**只动外层包装**——内层逻辑原样提顶层、去一层缩进、参数改 `$p.<名>`、末尾 emit `@{ ok=$true; ... }`。
- [ ] **Step 2:** `health_probes::run(...)` 加 exec 首参、删 cred，走 ssh::run_json（`name:"health-probes.ps1"`，args 为内层 param JSON）；FakeExec 测试断言 `ProbeResult`。
- [ ] **Step 3:** 改两处上游（`cli/domain_health.rs:291`、`commands/health_check.rs:135`）。
- [ ] **Step 4:** 测试全绿。
- [ ] **Step 5:** lanPC 集成验证（health 项较多，确认完整 295 行脚本经 `-File` 跑通且 JSON 完整——这也再次实证大脚本无命令行长度问题）。
- [ ] **Step 6:** commit `feat(health): migrate health-probes to SSH node-pure script`。

### Task 8：ue-log 域（带参 ×3 + 第二脚本 tail，配方情况 2）

**Files:** `ps-scripts/parse-ue-log.ps1`、`ps-scripts/tail-ue-log.ps1`、`src-tauri/src/core/ue_log_verify.rs:79`、调用方 `cli/domain_health.rs:110`、`cli/domain_log.rs:13`、`commands/log_verify.rs:32`

- [ ] **Step 1:** 配方 A 情况 2 改写 `parse-ue-log.ps1`（内层 param 3 个 → stdin 3 key）+ `tail-ue-log.ps1`（param 1 个）。
- [ ] **Step 2:** `ue_log_verify::run_for_host(...)` 加 exec 首参、删 cred，走 ssh::run_json（按 parse-ue-log 内层 param 给 args）；若 run_for_host 也用 tail-ue-log，同样切。FakeExec 测试断言 `VerifyReport`。
- [ ] **Step 3:** 改三处上游（`cli/domain_health.rs:110`、`cli/domain_log.rs:13`、`commands/log_verify.rs:32`）。
- [ ] **Step 4:** 测试全绿。
- [ ] **Step 5:** lanPC 集成验证。
- [ ] **Step 6:** commit `feat(ue-log): migrate parse/tail-ue-log to SSH node-pure script`。

### Task 9：A2 只读域收口

- [ ] **Step 1:** `cargo test --lib` 全绿；`cargo build` 通过；grep 确认这 8 个域的 core 模块不再 `use crate::core::powershell` 的 run_json（除 loopback 本地分支可能仍需本机执行——保留）。
- [ ] **Step 2:** 确认 `winrm.rs` 仍在（A5 才删），无 domain 调用方残留指向 winrm::invoke（除尚未迁移的 zen 等非只读域）。
- [ ] **Step 3:** lanPC 跑一次完整 `machine refresh` + `health run`（在 lanPC build 的 uecm-cli 上）确认端到端。
- [ ] **Step 4:** commit 收口（如有）。

---

## Self-Review

- **Spec §11-A2 覆盖**：machine/discovery（已在前序提交）+ 本计划 health(T7) / ini-read(T4) / project(T5) / command-line-scan(T1) / consistency(T2) / ddc-file-stats(T6) / renderstream(T3) / ue-log(T8) 全覆盖 ✅。
- **占位符扫描**：配方 A 给了两种情况的**完整**样板（scan-command-line-args 全 + read-ini-file 全）；各 task 给确切文件 + 行号 + 函数签名变更 + 上游清单。带参域的 args JSON key = 该 sidecar 内层 `param()` 名（执行时读该文件确定，配方 B 已说明规则）——这是依赖现有文件的真实步骤，非 vague 占位。
- **类型一致**：各域统一 `fn(exec: &dyn RemoteExecutor, host, ...)` + `ssh::run_json(exec, host, &NodeScript{name,args,ssh_user:None})` + FakeExec(返回 ScriptOutput) 测试模式；与已落的 discovery 域、ssh.rs 的 RemoteExecutor/NodeScript/ScriptOutput 签名一致。
- **范围**：8 个域同构、独立可验证、逐域提交；SSH/WinRM 并存不破坏未迁移域；loopback 本地分支保留。每域 lanPC 真节点验收。
