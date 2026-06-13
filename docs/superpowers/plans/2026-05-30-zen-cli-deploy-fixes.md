# Zen Server CLI 部署链修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 uecm-cli 的 zen-server `installed_service` 部署链脱离 Claude Code 也能在典型节点上跑出正确结果（修 F1–F6）。

**Architecture:** Rust 纯逻辑（intree fallback 选择、传 Port/HttpServerClass、detect fail-fast 判定、sponsor-down 派发）在 mac 上 TDD；PowerShell sidecar（ImagePath patch / 已存在服务 drift 修复 / sponsor 身份守卫 / 跨用户发现）+ 真实 SCM/注册表/端口行为在 lanPC 用 runbook 实测。CLI(`cli/domain_zen.rs`) 与 GUI 后端(`commands/zen.rs`) 是两套独立代码，F1/F2/F3 在两边各改一遍（不抽共享 Rust helper，贴合现有重复风格）。

**Tech Stack:** Rust（Tauri 2 / rusqlite / serde_json / clap）、PowerShell 7 sidecar（stdin JSON → envelope JSON）、SSH 传输（uecm-svc key）。

**对应 spec：** `docs/superpowers/specs/2026-05-30-zen-cli-deploy-fixes-design.md`（含 Codex review rev1 + 写计划 rev2）。

**平台限制（CLAUDE.md）：** 除 `machine scan/list` 外所有 zen 命令 Windows-only。mac 只跑 `cd src-tauri && cargo test --lib` + `cargo build`；PS/端到端在 lanPC（`C:\Tools\UECM\uecm-cli.exe`，源码仓 `E:\AIWorkspace\vp\ue-cache-manager`，build 后复制）。machine 13 = lanPC = 192.168.10.20。

---

## File Structure

**修改（Rust）**
- `src-tauri/src/cli/domain_zen.rs` — CLI 域：F1（install/uninstall 的 zen.exe 解析 ×2）、F2（install 传 Port/HttpServerClass）、F3（detect_binary 接 helper）、F4（新 `sponsor_down` fn + dispatch）、F6（apply-config dest 推导）。
- `src-tauri/src/commands/zen.rs` — GUI 后端：F1（×2）、F2（install 传 Port/HttpServerClass）、F3（zen_detect_binary 接 helper）。
- `src-tauri/src/core/zen/binary.rs` — F3：新增纯 helper `detect_yielded_nothing` + 单测。
- `src-tauri/src/cli/args.rs` — F4：`ZenAction::SponsorDown` 变体。

**修改（PowerShell）**
- `ps-scripts/zen-service-install.ps1` — F2：`Patch-ImagePath` helper + 全新安装 patch + 已存在服务 port/http drift 三分支修复。
- `ps-scripts/zen-detect-binary.ps1` — F5：跨 user profile 枚举 install-dir binary。

**新建**
- `ps-scripts/zen-sponsor-down.ps1` — F4：身份守卫 + `zen.exe down --port`。

**测试落点（mac）**
- `src-tauri/src/core/zen/binary.rs` `#[cfg(test)]` — F3 helper 单测、F1 fallback 机制单测（经 `machine_ue_installs` 数据层）。
- `src-tauri/src/cli/args.rs` 既有 clap 测试模块（或 domain_zen 测试）— F4 子命令解析单测。

---

## 落地顺序与分组

A 组（关键路径，跑通 lanPC E2E 后即可合 main）：Task 1–10（F1、F2、F3、F4）。
B 组（增强，可后续追加）：Task 11（F5）、Task 12（F6）。
收尾：Task 13（lanPC E2E runbook + 全量回归）。

---

## Task 1: F1 — fallback 选择机制单测（数据层，先立回归基线）

**Files:**
- Test: `src-tauri/src/core/zen/binary.rs`（`#[cfg(test)] mod tests` 内，复用既有 `db_with_machine` / `seed_ue_install` 夹具）

> 由于 F1 决定不抽共享函数（4 处内联 `.or_else`），这里测的是 fallback 真正依赖的机制：`machine_ue_installs::list_for_machine` 是 `ORDER BY version DESC`，`find_map(|i| i.zen_cli_intree_path)` 取到**最高版本**的 intree zen.exe。

- [ ] **Step 1: 写失败测试**

在 binary.rs 测试模块末尾加（`seed_ue_install` 已存在并支持播种 intree path；若它不写 intree_path，则在测试里直接 `machine_ue_installs::upsert` 造两行）：

```rust
#[test]
fn intree_fallback_picks_highest_version_zen_cli() {
    use crate::data::machine_ue_installs::{self, UeInstall};
    let (db, machine_id) = db_with_machine();

    // 两行：5.2 与 5.8，各带 intree zen.exe。无 machine_zen_install。
    for (ver, path) in [
        ("5.2", r"D:\Epic\UE_5.2\Engine\Binaries\Win64\zen.exe"),
        ("5.8", r"D:\Epic\UE_5.8\Engine\Binaries\Win64\zen.exe"),
    ] {
        machine_ue_installs::upsert(
            &db,
            &UeInstall {
                id: None,
                machine_id,
                version: ver.into(),
                install_path: format!(r"D:\Epic\UE_{ver}"),
                is_primary: false,
                zen_cli_intree_path: Some(path.into()),
                zen_cli_intree_version: Some(format!("{ver}.0")),
                zen_cli_intree_sha256: Some("deadbeef".into()),
                zenserver_intree_path: None,
                zenserver_intree_version: None,
                zenserver_intree_sha256: None,
            },
        )
        .unwrap();
    }

    // 这就是 4 处 install/uninstall 内联使用的 fallback 表达式。
    let picked = machine_ue_installs::list_for_machine(&db, machine_id)
        .unwrap()
        .into_iter()
        .find_map(|i| i.zen_cli_intree_path);

    assert_eq!(
        picked.as_deref(),
        Some(r"D:\Epic\UE_5.8\Engine\Binaries\Win64\zen.exe"),
        "fallback must pick the highest-version intree zen.exe (version DESC)"
    );
}

#[test]
fn intree_fallback_none_when_no_intree_rows() {
    let (db, machine_id) = db_with_machine();
    let picked = crate::data::machine_ue_installs::list_for_machine(&db, machine_id)
        .unwrap()
        .into_iter()
        .find_map(|i| i.zen_cli_intree_path);
    assert!(picked.is_none(), "no intree rows → fallback yields None → caller keeps 'has no zen.exe'");
}
```

- [ ] **Step 2: 跑测试确认失败/通过基线**

Run: `cd src-tauri && cargo test --lib intree_fallback_ -- --nocapture`
Expected: 编译通过；两测 PASS（这是对既有数据层行为的回归锁定。若 `seed_ue_install`/`UeInstall` 字段名不符导致编译错，按编译器提示对齐字段——`UeInstall` 字段见 `data/machine_ue_installs.rs:8-30`）。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/core/zen/binary.rs
git commit -m "test(zen): lock intree-fallback selection (version DESC, highest wins)"
```

---

## Task 2: F1 — CLI 域 install/uninstall 加 intree fallback

**Files:**
- Modify: `src-tauri/src/cli/domain_zen.rs:1258-1267`（install）、`1430-1440`（uninstall）

- [ ] **Step 1: 改 install 的 zen.exe 解析（domain_zen.rs:1258-1267）**

把：

```rust
    let zen_exe = install
        .as_ref()
        .and_then(|m| m.zen_cli_path.clone())
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={} has no zen.exe (zen_cli) recorded — run \
                 `uecm-cli zen detect-binary --machine {}` first",
                ep.machine_id, ep.machine_id,
            ))
        })?;
```

改为：

```rust
    let zen_exe = install
        .as_ref()
        .and_then(|m| m.zen_cli_path.clone())
        // F1: no install-dir copy recorded (common: detect-binary couldn't see
        // it cross-user) → fall back to the highest-version intree zen.exe
        // from machine_ue_installs (list_for_machine is ORDER BY version DESC).
        .or_else(|| {
            crate::data::machine_ue_installs::list_for_machine(&db, ep.machine_id)
                .ok()
                .and_then(|v| v.into_iter().find_map(|i| i.zen_cli_intree_path))
        })
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={} has no zen.exe (zen_cli) recorded — run \
                 `uecm-cli zen detect-binary --machine {}` first",
                ep.machine_id, ep.machine_id,
            ))
        })?;
```

- [ ] **Step 2: 改 uninstall 的 zen.exe 解析（domain_zen.rs:1430-1440）**

把 `.and_then(|m| m.zen_cli_path.clone())` 之后、`.ok_or_else(` 之前，插入同一个 `.or_else(...)` 块（与 Step 1 逐字相同）：

```rust
        .or_else(|| {
            crate::data::machine_ue_installs::list_for_machine(&db, ep.machine_id)
                .ok()
                .and_then(|v| v.into_iter().find_map(|i| i.zen_cli_intree_path))
        })
```

- [ ] **Step 3: 编译**

Run: `cd src-tauri && cargo build --lib`
Expected: 编译通过，无 warning（`crate::data::machine_ue_installs` 是有效路径）。

- [ ] **Step 4: 全量单测无回归**

Run: `cd src-tauri && cargo test --lib`
Expected: 全绿。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/cli/domain_zen.rs
git commit -m "fix(zen-cli): service install/uninstall fall back to intree zen.exe (F1)"
```

---

## Task 3: F1 — GUI 后端 install/uninstall 加同样 fallback

**Files:**
- Modify: `src-tauri/src/commands/zen.rs:1047-1055`（install）、`1161-1169`（uninstall）

- [ ] **Step 1: 改 install（commands/zen.rs:1047-1055）**

在 `.and_then(|m| m.zen_cli_path.clone())` 与 `.ok_or_else(` 之间插入：

```rust
        .or_else(|| {
            crate::data::machine_ue_installs::list_for_machine(&db, ep.machine_id)
                .ok()
                .and_then(|v| v.into_iter().find_map(|i| i.zen_cli_intree_path))
        })
```

- [ ] **Step 2: 改 uninstall（commands/zen.rs:1161-1169）**

同样在 `.and_then(|m| m.zen_cli_path.clone())` 之后插入与 Step 1 逐字相同的 `.or_else(...)` 块。

- [ ] **Step 3: 编译 + 全量单测**

Run: `cd src-tauri && cargo build --lib && cargo test --lib`
Expected: 编译通过、测试全绿。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/commands/zen.rs
git commit -m "fix(zen-gui): service install/uninstall fall back to intree zen.exe (F1 parity)"
```

---

## Task 4: F2 (Rust) — 两条 install 路径传 Port / HttpServerClass

**Files:**
- Modify: `src-tauri/src/cli/domain_zen.rs:1375-1379`（install 的 args 初值）
- Modify: `src-tauri/src/commands/zen.rs:1122-1126`（install 的 args 初值）

> `ep.declared_port: i64`、`ep.httpserverclass: String` 已在 endpoint 行上（见 register）。这是字面量透传，单元价值低 → 用 `cargo build` + lanPC E2E（Task 13 step 6/6b）验证；不写独立单测。

- [ ] **Step 1: 改 CLI 域 args（domain_zen.rs:1375-1379）**

把：

```rust
    let mut args = serde_json::json!({
        "ZenExePath": zen_exe,
        "ServiceName": DEFAULT_SERVICE_NAME,
        "DataDir": ep.data_dir,
    });
```

改为：

```rust
    let mut args = serde_json::json!({
        "ZenExePath": zen_exe,
        "ServiceName": DEFAULT_SERVICE_NAME,
        "DataDir": ep.data_dir,
        // F2: zen's `service install` does NOT persist these into the SCM
        // ImagePath; the sidecar patches the registry so the service starts on
        // the declared port instead of relocating to base+100.
        "Port": ep.declared_port,
        "HttpServerClass": ep.httpserverclass,
    });
```

- [ ] **Step 2: 改 GUI 后端 args（commands/zen.rs:1122-1126）**

同样在 `"DataDir": ep.data_dir,` 后加两行：

```rust
        "Port": ep.declared_port,
        "HttpServerClass": ep.httpserverclass,
```

- [ ] **Step 3: 编译 + 全量单测**

Run: `cd src-tauri && cargo build --lib && cargo test --lib`
Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/cli/domain_zen.rs src-tauri/src/commands/zen.rs
git commit -m "feat(zen): forward Port/HttpServerClass to service-install sidecar (F2)"
```

---

## Task 5: F2 (PS) — Patch-ImagePath helper + 全新安装注入

**Files:**
- Modify: `ps-scripts/zen-service-install.ps1`（param 段 `66-69` 后；helper 段 `110` 后；成功尾部 `528-530` 之间）

> lanPC 验证（Task 13）。本任务只改脚本 + 形态自查。

- [ ] **Step 1: 读 Port / HttpServerClass 参数（在 `zen-service-install.ps1:69` 的 `$ServicePassword = ...` 行后插入）**

```powershell
$Port = if ($p.Port) { [string]$p.Port } else { '' }
$HttpServerClass = if ($p.HttpServerClass) { [string]$p.HttpServerClass } else { '' }
```

- [ ] **Step 2: 加 `Patch-ImagePath` helper（在 `Get-ServiceAccount` 函数结束 `110` 行后插入）**

```powershell
# F2: zen `service install` records only `--data-dir` into the SCM PathName;
# `--port` / `--http` are dropped, so the service relocates off the declared
# port (8558 -> 8658) when the port is briefly in TIME_WAIT. Directly rewrite
# ImagePath to pin the runtime flags. Returns $true when ImagePath now contains
# `--port`. No-op (returns $true) when $Port is empty.
function Patch-ImagePath([string]$Name, [string]$ExePath, [string]$DataDir, [string]$Port, [string]$Http) {
    if ([string]::IsNullOrWhiteSpace($Port)) { return $true }
    $exePart = '"' + $ExePath + '"'
    $newBinpath = "$exePart --data-dir `"$DataDir`" --port $Port"
    if (-not [string]::IsNullOrWhiteSpace($Http)) { $newBinpath += " --http $Http" }
    $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$Name"
    Set-ItemProperty -LiteralPath $regPath -Name 'ImagePath' -Value $newBinpath -ErrorAction Stop
    $after = (Get-ItemProperty -LiteralPath $regPath -Name 'ImagePath' -ErrorAction Stop).ImagePath
    return ($after -match '(^|\s)--port\s')
}
```

- [ ] **Step 3: 全新安装末尾调用 patch（在成功 `$payload = @{ ... }` 之前，即 `zen-service-install.ps1:529` 行 `}` 之后、`530` 行 `$payload` 之前插入）**

```powershell
    # F2: pin --port/--http into ImagePath now that the fresh install + any
    # sc.exe account patch have landed.
    $portPatched = Patch-ImagePath $ServiceName $ZenExePath $normalizedDataDir $Port $HttpServerClass
    if (-not $portPatched) {
        @{
            ok = $false
            message = "ImagePath patch failed: --port did not persist for service '$ServiceName'"
            service_name = $ServiceName
            zen_exit_code = $exitCode
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }
```

并在成功 `$payload` 里加一行可观测字段（在 `message = $combined.Trim()` 后）：

```powershell
        image_path_port_pinned = $portPatched
```

- [ ] **Step 4: 形态自查**

Run: `pwsh -NoProfile -Command "$null = [System.Management.Automation.Language.Parser]::ParseFile('ps-scripts/zen-service-install.ps1',[ref]$null,[ref]$null); 'parse-ok'"`
Expected: 输出 `parse-ok`（仅语法解析；行为验证在 lanPC）。若 mac 无 pwsh，跳过，标记留 lanPC。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/zen-service-install.ps1
git commit -m "feat(zen-ps): pin --port/--http into SCM ImagePath on fresh install (F2)"
```

---

## Task 6: F2 (PS) — 已存在服务的 port/http drift 就地修复

**Files:**
- Modify: `ps-scripts/zen-service-install.ps1:305-378`（已存在服务的判定 + 收尾分支）

> 修 Codex review #1：旧 CLI 装的「有 data-dir 没 port」服务，exe/data/account 全匹配 → 现状 `already_installed` no-op exit 0，patch 到不了。加 port/http 解析 + 三分支。

- [ ] **Step 1: 在 `--data-dir` 解析（`317` 行 `if ($null -ne $existingDir)` 块）之后、`$matchesExpected = ...`（`327` 行）之前，解析现有 ImagePath 的 `--port` / `--http`**

```powershell
            # F2: parse existing --port / --http so a port/http-only drift can
            # be repaired in place instead of slipping through as a no-op.
            $existingPort = $null
            $existingHttp = $null
            for ($j = 0; $j -lt $tokens.Count; $j++) {
                $tk = $tokens[$j].ToString()
                if ($tk -ieq '--port' -and ($j + 1) -lt $tokens.Count) { $existingPort = $tokens[$j + 1].ToString() }
                elseif ($tk -match '^--port=(.*)$') { $existingPort = $Matches[1] }
                elseif ($tk -ieq '--http' -and ($j + 1) -lt $tokens.Count) { $existingHttp = $tokens[$j + 1].ToString() }
                elseif ($tk -match '^--http=(.*)$') { $existingHttp = $Matches[1] }
            }
```

- [ ] **Step 2: 在 `$userMatches = (...)`（`345` 行）之后，计算 port/http 是否匹配**

```powershell
        # F2: expected port/http from the caller (empty $Port means "don't manage port").
        $portHttpMatches = $true
        if (-not [string]::IsNullOrWhiteSpace($Port)) {
            $portHttpMatches = ("$existingPort" -eq "$Port") -and
                               ([string]::IsNullOrWhiteSpace($HttpServerClass) -or ("$existingHttp" -eq "$HttpServerClass"))
        }
```

- [ ] **Step 3: 用三分支替换原 no-op / refuse 块（`347-378` 行整段：`if ($matchesExpected -and $userMatches) { ... } ... exit 0`）**

```powershell
        if ($matchesExpected -and $userMatches -and $portHttpMatches) {
            @{
                ok = $true
                service_name = $ServiceName
                already_installed = $true
                existing_status = "$($existingSvc.Status)"
                existing_path_name = $existingPathName
                existing_service_account = $existingStartName
                message = "service '$ServiceName' already installed with matching config (no-op)"
            } | ConvertTo-Json -Compress -Depth 4
            exit 0
        }

        # F2: exe/data/account match but --port/--http drift (the old-CLI bad
        # state) → repair ImagePath in place rather than forcing an uninstall.
        if ($matchesExpected -and $userMatches -and (-not $portHttpMatches)) {
            $repaired = Patch-ImagePath $ServiceName $ZenExePath $normalizedDataDir $Port $HttpServerClass
            @{
                ok = $repaired
                service_name = $ServiceName
                repaired = $repaired
                existing_status = "$($existingSvc.Status)"
                existing_path_name = $existingPathName
                existing_port = "$existingPort"
                requested_port = "$Port"
                message = if ($repaired) {
                    "service '$ServiceName' ImagePath repaired: pinned --port $Port" +
                    (if ([string]::IsNullOrWhiteSpace($HttpServerClass)) { '' } else { " --http $HttpServerClass" }) +
                    " (run `zen service stop` + `start` to take effect)"
                } else {
                    "ImagePath repair failed: --port did not persist for service '$ServiceName'"
                }
            } | ConvertTo-Json -Compress -Depth 4
            exit 0
        }

        $reason = if (-not $matchesExpected) {
            'different ZenExePath / DataDir'
        } elseif (-not $userMatches) {
            "different service account (existing: '$existingStartName', requested: '$ServiceUser')"
        } else {
            'unknown drift'
        }
        @{
            ok = $false
            message = ("Service '{0}' is already installed (status: {1}) with {2}. " +
                       "Refusing to re-install without --full (Plan 7 §12 red line). " +
                       "Run zen-service-uninstall.ps1 first to change DataDir / zen.exe path / service account.") `
                      -f $ServiceName, $existingSvc.Status, $reason
            existing_service_account = $existingStartName
            service_name = $ServiceName
            existing_status = "$($existingSvc.Status)"
            existing_path_name = $existingPathName
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
```

- [ ] **Step 4: 形态自查**

Run: `pwsh -NoProfile -Command "$null = [System.Management.Automation.Language.Parser]::ParseFile('ps-scripts/zen-service-install.ps1',[ref]$null,[ref]$null); 'parse-ok'"`
Expected: `parse-ok`（行为验证在 lanPC Task 13 step 6b）。

- [ ] **Step 5: 提交**

```bash
git add ps-scripts/zen-service-install.ps1
git commit -m "fix(zen-ps): repair port/http ImagePath drift on existing service (F2, Codex #1)"
```

---

## Task 7: F3 — `detect_yielded_nothing` 纯 helper + 单测

**Files:**
- Modify: `src-tauri/src/core/zen/binary.rs`（`persist` 函数后加 pub helper；测试模块加单测）

- [ ] **Step 1: 写失败测试（binary.rs 测试模块内）**

```rust
#[test]
fn detect_yielded_nothing_true_when_intree_all_skipped_and_no_install() {
    let det = BinaryDetection {
        install: None,
        intree: vec![IntreeBinaries {
            ue_version_major: 5, ue_version_minor: 7,
            ue_install_path: "D:\\UE_5.7".into(),
            zen_cli_path: Some("D:\\UE_5.7\\Engine\\Binaries\\Win64\\zen.exe".into()),
            zen_cli_version: Some("5.7.6".into()), zen_cli_sha256: Some("c0ffee".into()),
            zenserver_path: None, zenserver_version: None, zenserver_sha256: None,
        }],
        warnings: vec![],
    };
    let report = PersistReport { install_record_written: false, intree_records_written: 0, ..Default::default() };
    assert!(detect_yielded_nothing(&det, &report));
}

#[test]
fn detect_yielded_nothing_false_when_install_or_intree_written_or_empty() {
    let intree = vec![IntreeBinaries {
        ue_version_major: 5, ue_version_minor: 7, ue_install_path: "D:\\UE_5.7".into(),
        zen_cli_path: None, zen_cli_version: None, zen_cli_sha256: None,
        zenserver_path: None, zenserver_version: None, zenserver_sha256: None,
    }];
    // install record written → false
    let d1 = BinaryDetection { install: None, intree: intree.clone(), warnings: vec![] };
    let r1 = PersistReport { install_record_written: true, intree_records_written: 0, ..Default::default() };
    assert!(!detect_yielded_nothing(&d1, &r1));
    // some intree written → false
    let r2 = PersistReport { install_record_written: false, intree_records_written: 1, ..Default::default() };
    assert!(!detect_yielded_nothing(&d1, &r2));
    // no intree candidates at all (empty machine) → false
    let d3 = BinaryDetection { install: None, intree: vec![], warnings: vec![] };
    let r3 = PersistReport { install_record_written: false, intree_records_written: 0, ..Default::default() };
    assert!(!detect_yielded_nothing(&d3, &r3));
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib detect_yielded_nothing`
Expected: FAIL（`detect_yielded_nothing` 未定义）。

- [ ] **Step 3: 实现 helper（在 `persist` 函数 `313` 行 `}` 之后插入）**

```rust
/// True when a detect-binary run produced nothing usable: it saw intree
/// candidates but skipped them all (no `machine_ue_installs` row → operator
/// forgot `machine refresh`) AND wrote no install-dir record either. Callers
/// turn this into a per-machine failure so the empty result never looks like
/// success and silently breaks the downstream service install.
pub fn detect_yielded_nothing(detection: &BinaryDetection, report: &PersistReport) -> bool {
    !report.install_record_written
        && !detection.intree.is_empty()
        && report.intree_records_written == 0
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo test --lib detect_yielded_nothing`
Expected: PASS（2 测）。`binary.rs:709` 既有 `persist_skips_intree_when_no_ue_install_row` 仍 PASS（persist 未改）。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/core/zen/binary.rs
git commit -m "feat(zen): add detect_yielded_nothing helper for fail-fast (F3)"
```

---

## Task 8: F3 — 接入两条 detect 处理器

**Files:**
- Modify: `src-tauri/src/cli/domain_zen.rs:548-563`（CLI detect 的 Ok 分支）
- Modify: `src-tauri/src/commands/zen.rs:373-388`（GUI detect 的 Ok 分支）

- [ ] **Step 1: CLI 域（domain_zen.rs）—— 在 Ok(detection) 分支里 persist 之后插入 fail-fast 判定**

把 `548-563` 行的：

```rust
            Ok(detection) => {
                let report = zen_binary::persist(&db, machine_id, &detection)?;
                ok_count += 1;
                let summary = serde_json::json!({
```

改为：

```rust
            Ok(detection) => {
                let report = zen_binary::persist(&db, machine_id, &detection)?;
                // F3: intree candidates seen but all skipped (no machine_ue_installs
                // row) and no install record → fail this machine with a fix hint
                // instead of reporting a hollow success.
                if zen_binary::detect_yielded_nothing(&detection, &report) {
                    failed += 1;
                    ctx.emitter
                        .emit_event(&Event::ItemCompleted {
                            item_id: format!("machine:{}", machine_id),
                            index: idx as i64,
                            ok: false,
                            message: Some(format!(
                                "detect-binary found intree zen.exe but machine_ue_installs is empty \
                                 for machine id={machine_id}; run `uecm-cli machine refresh {machine_id}` first"
                            )),
                        })
                        .ok();
                    continue;
                }
                ok_count += 1;
                let summary = serde_json::json!({
```

（`continue` 跳过本机原本的 Completed emit；`idx` 已在 `for (idx, m)` 中可用。）

- [ ] **Step 2: GUI 后端（commands/zen.rs）—— 在 Ok(detection) 分支 persist 之后改为按判定分流**

把 `373-388` 行的：

```rust
            Ok(detection) => {
                let report = zen_binary::persist(&db, mid, &detection)?;
                ok_count += 1;
                results.push(ZenDetectBinaryMachineResult {
                    machine_id: mid,
                    hostname: m.hostname.clone(),
                    ip: m.ip.clone(),
                    ok: true,
                    install_record_written: report.install_record_written,
                    install_record_cleared: report.install_record_cleared,
                    intree_records_written: report.intree_records_written,
                    baseline_new_rows: report.baseline_new_rows,
                    intree_ref_rows: report.intree_ref_rows,
                    warnings: report.warnings,
                    error_message: None,
                });
            }
```

改为：

```rust
            Ok(detection) => {
                let report = zen_binary::persist(&db, mid, &detection)?;
                // F3: hollow result (intree skipped, no install record) → mark failed.
                if zen_binary::detect_yielded_nothing(&detection, &report) {
                    failed += 1;
                    results.push(ZenDetectBinaryMachineResult {
                        machine_id: mid,
                        hostname: m.hostname.clone(),
                        ip: m.ip.clone(),
                        ok: false,
                        install_record_written: report.install_record_written,
                        install_record_cleared: report.install_record_cleared,
                        intree_records_written: report.intree_records_written,
                        baseline_new_rows: report.baseline_new_rows,
                        intree_ref_rows: report.intree_ref_rows,
                        warnings: report.warnings,
                        error_message: Some(format!(
                            "found intree zen.exe but machine_ue_installs is empty for machine id={mid}; \
                             run machine refresh first"
                        )),
                    });
                } else {
                    ok_count += 1;
                    results.push(ZenDetectBinaryMachineResult {
                        machine_id: mid,
                        hostname: m.hostname.clone(),
                        ip: m.ip.clone(),
                        ok: true,
                        install_record_written: report.install_record_written,
                        install_record_cleared: report.install_record_cleared,
                        intree_records_written: report.intree_records_written,
                        baseline_new_rows: report.baseline_new_rows,
                        intree_ref_rows: report.intree_ref_rows,
                        warnings: report.warnings,
                        error_message: None,
                    });
                }
            }
```

- [ ] **Step 3: 编译 + 全量单测**

Run: `cd src-tauri && cargo build --lib && cargo test --lib`
Expected: 通过（注意 `Event::ItemCompleted` 字段名以编译器为准；若签名不同按提示对齐）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/cli/domain_zen.rs src-tauri/src/commands/zen.rs
git commit -m "fix(zen): detect-binary fails fast when intree skipped + no install record (F3)"
```

---

## Task 9: F4 — `ZenAction::SponsorDown` 子命令 + 解析单测

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`ZenAction` 枚举内，紧跟 `Service { ... }` 之后加变体）
- Test: `src-tauri/src/cli/args.rs` 既有测试模块（clap 解析）

- [ ] **Step 1: 加枚举变体（args.rs，`ZenAction::Service { ... }` 之后）**

```rust
    /// Gracefully shut down an editor sponsor zenserver squatting the
    /// endpoint's declared port (so `service install`/`start` can take it).
    /// Refuses if the port is served by the installed ZenServer service.
    SponsorDown {
        #[arg(long, value_name = "ID")]
        endpoint_id: i64,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        cred: crate::cli::credential_args::CredentialArgs,
    },
```

- [ ] **Step 2: 写解析失败测试（args.rs 测试模块）**

```rust
#[test]
fn parses_zen_sponsor_down() {
    use clap::Parser;
    let cli = crate::cli::args::Cli::try_parse_from([
        "uecm-cli", "zen", "sponsor-down", "--endpoint-id", "1", "--dry-run",
    ])
    .expect("sponsor-down should parse");
    // 断言路由到 ZenAction::SponsorDown（按既有测试里访问 Command/Zen 的方式取 action）。
    let s = format!("{cli:?}");
    assert!(s.contains("SponsorDown"), "expected SponsorDown, got: {s}");
}
```

> 若 `Cli`/`Command` 类型路径或既有测试的解构方式不同，按同模块既有 clap 测试样式对齐（搜 `try_parse_from` 参考）。

- [ ] **Step 3: 加 dispatch + stub，保证可编译（domain_zen.rs `handle()`，`ZenAction::Urlacl` 分支之前）**

> 只加变体会让 `handle()` 非穷尽匹配编译失败。先接一个 stub（Task 10 再填真身），让本任务可编译可提交。

```rust
        ZenAction::SponsorDown { endpoint_id, yes, dry_run, cred } => {
            sponsor_down(ctx, endpoint_id, yes, dry_run, &cred)
        }
```

并在 `service_simple` 之后加 stub：

```rust
fn sponsor_down(
    _ctx: &mut Ctx<'_>,
    _endpoint_id: i64,
    _yes: bool,
    _dry_run: bool,
    _cred: &CredentialArgs,
) -> UecmResult<()> {
    Err(UecmError::InvalidInput("zen sponsor-down not yet implemented".into()))
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cd src-tauri && cargo build --lib && cargo test --lib parses_zen_sponsor_down`
Expected: 编译通过；解析测 PASS。

- [ ] **Step 5: 提交（变体 + dispatch + stub + 测试）**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_zen.rs
git commit -m "feat(zen-cli): add `zen sponsor-down` subcommand + dispatch stub (F4)"
```

---

## Task 10: F4 — `sponsor_down` 真实处理器 + 新 PS 脚本

**Files:**
- Modify: `src-tauri/src/cli/domain_zen.rs`（用真身替换 Task 9 的 `sponsor_down` stub；dispatch 已接好）
- Create: `ps-scripts/zen-sponsor-down.ps1`

- [ ] **Step 1: 用真实实现替换 Task 9 的 `sponsor_down` stub（domain_zen.rs）**

```rust
fn sponsor_down(
    ctx: &mut Ctx<'_>,
    endpoint_id: i64,
    yes: bool,
    dry_run: bool,
    cred: &CredentialArgs,
) -> UecmResult<()> {
    let outcome = destructive::check(yes, dry_run, "zen.sponsor_down")?;
    let db = ctx.require_db()?.clone();
    cred.preflight(&db)?;
    let ep = require_endpoint(&db, endpoint_id)?;
    let machine = require_machine(&db, ep.machine_id)?;

    // Reuse the F1 resolution: install-dir zen.exe, else highest-version intree.
    let install = machine_zen_install::find(&db, ep.machine_id)?;
    let zen_exe = install
        .as_ref()
        .and_then(|m| m.zen_cli_path.clone())
        .or_else(|| {
            crate::data::machine_ue_installs::list_for_machine(&db, ep.machine_id)
                .ok()
                .and_then(|v| v.into_iter().find_map(|i| i.zen_cli_intree_path))
        })
        .ok_or_else(|| {
            UecmError::InvalidInput(format!(
                "machine id={} has no zen.exe (zen_cli) recorded — run \
                 `uecm-cli zen detect-binary --machine {}` first",
                ep.machine_id, ep.machine_id,
            ))
        })?;

    let invocation = format!(
        "zen-sponsor-down.ps1 -ZenExePath {zen_exe} -Port {} -ServiceName {DEFAULT_SERVICE_NAME} -DryRun {}",
        ep.declared_port,
        outcome == Outcome::DryRun
    );
    let op_id = operations::start(&db, "zen.sponsor_down", &[ep.machine_id])?;
    let result = run_node(
        &machine.ip,
        "zen-sponsor-down.ps1",
        serde_json::json!({
            "ZenExePath": zen_exe,
            "Port": ep.declared_port,
            "ServiceName": DEFAULT_SERVICE_NAME,
            "DryRun": outcome == Outcome::DryRun,
        }),
    )
    .and_then(|raw| parse_envelope(&raw, "zen-sponsor-down"));
    finalize_op(&db, op_id, &result, &invocation);
    let response = result?;
    let summary = serde_json::json!({
        "ok": true,
        "endpoint_id": endpoint_id,
        "machine_id": ep.machine_id,
        "host": machine.ip,
        "port": ep.declared_port,
        "dry_run": outcome == Outcome::DryRun,
        "remote": response,
    });
    ctx.emitter.emit_event(&Event::Completed { summary }).ok();
    Ok(())
}
```

> `Outcome` / `destructive::check` 已在本文件用于其他命令（见 `service_uninstall`）。`destructive::check` 在 `--dry-run` 时返回 `Outcome::DryRun`，否则要求 `--yes`。这里 dry-run 仍 `run_node`：脚本在 DryRun 下只探测+报告身份、不执行 down（read-only）。

- [ ] **Step 2: 新建 `ps-scripts/zen-sponsor-down.ps1`**

```powershell
# F4 sidecar - gracefully shut down an editor sponsor zenserver on a port.
#
# Parameters (stdin JSON):
#   -ZenExePath  <string>  zen.exe to run `down --port` with.
#   -Port        <int>     port the sponsor zenserver is squatting.
#   -ServiceName <string>  installed service name to compare against. Default "ZenServer".
#   -DryRun      <bool>    when true, report identity but do NOT shut down.
#
# Identity guard (Codex #2): refuse if the listener PID is the installed
# ZenServer service, or the listener is not a zenserver.exe.
#
# Output envelope: { ok, nothing_attached?, refused?, is_installed_service?,
#                    listener_pid?, listener_path?, would_stop?, message }

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 | Out-Null
$ErrorActionPreference = 'Stop'

try {
    $p = [Console]::In.ReadToEnd() | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($p.ZenExePath)) { @{ ok=$false; message="ZenExePath is required" } | ConvertTo-Json -Compress; exit 0 }
    if ([string]::IsNullOrWhiteSpace($p.Port))       { @{ ok=$false; message="Port is required" } | ConvertTo-Json -Compress; exit 0 }
    $ZenExePath  = $p.ZenExePath
    $Port        = [int]$p.Port
    $ServiceName = if ($p.ServiceName) { $p.ServiceName } else { 'ZenServer' }
    $DryRun      = [bool]$p.DryRun

    # 1. Who is listening on the port?
    $conn = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
            Select-Object -First 1
    if ($null -eq $conn) {
        @{ ok=$true; nothing_attached=$true; message="no listener on port $Port" } | ConvertTo-Json -Compress
        exit 0
    }
    $listenerPid = [int]$conn.OwningProcess
    $listenerPath = $null
    try { $listenerPath = (Get-Process -Id $listenerPid -ErrorAction Stop).Path } catch { }

    # 2. Is that PID the installed ZenServer service? (only Running has a PID)
    $svcPid = $null
    try {
        $cim = Get-CimInstance -ClassName Win32_Service -Filter "Name='$ServiceName'" -ErrorAction Stop
        if ($null -ne $cim -and $cim.ProcessId -gt 0) { $svcPid = [int]$cim.ProcessId }
    } catch { }

    if ($null -ne $svcPid -and $svcPid -eq $listenerPid) {
        @{
            ok=$false; refused=$true; is_installed_service=$true
            listener_pid=$listenerPid; listener_path=$listenerPath
            message="port $Port is served by the installed '$ServiceName' service (pid $listenerPid), not an editor sponsor; use `zen service stop`"
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    # 3. Sanity: the listener should be a zenserver.exe.
    if ($null -ne $listenerPath -and $listenerPath -notmatch 'zenserver\.exe$') {
        @{
            ok=$false; refused=$true; is_installed_service=$false
            listener_pid=$listenerPid; listener_path=$listenerPath
            message="port $Port is held by a non-zenserver process ($listenerPath, pid $listenerPid); refusing to shut down"
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    # 4. dry-run: report identity, do not stop.
    if ($DryRun) {
        @{
            ok=$true; would_stop=$true; is_installed_service=$false
            listener_pid=$listenerPid; listener_path=$listenerPath
            message="[dry-run] would shut down sponsor zenserver pid $listenerPid on port $Port"
        } | ConvertTo-Json -Compress -Depth 4
        exit 0
    }

    # 5. shut it down.
    $out = (& $ZenExePath down --port $Port 2>&1 | Out-String)
    $code = [int]$LASTEXITCODE
    @{
        ok = ($code -eq 0)
        is_installed_service=$false
        listener_pid=$listenerPid; listener_path=$listenerPath
        zen_exit_code=$code
        message=$out.Trim()
    } | ConvertTo-Json -Compress -Depth 4
}
catch {
    @{ ok=$false; message="sponsor-down failed: $($_.Exception.Message)" } | ConvertTo-Json -Compress
    exit 0
}
```

- [ ] **Step 3: 编译 + 解析单测 + PS 形态自查**

Run: `cd src-tauri && cargo build --lib && cargo test --lib parses_zen_sponsor_down`
Expected: 编译通过、解析测 PASS。
Run: `pwsh -NoProfile -Command "$null=[System.Management.Automation.Language.Parser]::ParseFile('ps-scripts/zen-sponsor-down.ps1',[ref]$null,[ref]$null);'parse-ok'"`
Expected: `parse-ok`（守卫行为在 lanPC Task 13 step 7 验证）。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/cli/domain_zen.rs ps-scripts/zen-sponsor-down.ps1
git commit -m "feat(zen): `zen sponsor-down` with installed-service identity guard (F4, Codex #2)"
```

---

## Task 11: F5 (PS, B 组) — detect-binary 跨 user profile 枚举

**Files:**
- Modify: `ps-scripts/zen-detect-binary.ps1:97-98`（install dir 解析）

> lanPC 验证。当前只读 `$env:LOCALAPPDATA`（SSH 登录用户 uecm-svc），对装 zen 的 UE 用户盲。uecm-svc 是本地 admin，可遍历 `C:\Users\*`。

- [ ] **Step 1: 替换 install dir 解析（`zen-detect-binary.ps1:97-98`）**

把：

```powershell
    # --- (1) Install dir under %LOCALAPPDATA% ---------------------------------
    $installDir = Join-Path -Path $env:LOCALAPPDATA -ChildPath 'UnrealEngine\Common\Zen\Install'
```

改为：

```powershell
    # --- (1) Install dir under any user's %LOCALAPPDATA% ----------------------
    # F5: SSH logs in as uecm-svc, whose LOCALAPPDATA never holds the UE-user's
    # zen install. Enumerate every profile (uecm-svc is a local admin) and pick
    # the first that actually has the install binary.
    $installDir = $null
    $selfLocal = Join-Path -Path $env:LOCALAPPDATA -ChildPath 'UnrealEngine\Common\Zen\Install'
    $candidates = @($selfLocal)
    try {
        $candidates += Get-ChildItem 'C:\Users' -Directory -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName 'AppData\Local\UnrealEngine\Common\Zen\Install' }
    } catch { }
    foreach ($c in ($candidates | Select-Object -Unique)) {
        if (Test-Path -LiteralPath (Join-Path $c 'zenserver.exe')) { $installDir = $c; break }
        if (Test-Path -LiteralPath (Join-Path $c 'zen.exe'))       { $installDir = $c; break }
    }
    if ($null -eq $installDir) { $installDir = $selfLocal }  # keep old behavior when nothing found
```

> 后续读取 `$installDir` 下文件的逻辑保持不变（含 `155` 行起的注册表 `InstalledDirectory` 探测——保留作为补充）。

- [ ] **Step 2: PS 形态自查**

Run: `pwsh -NoProfile -Command "$null=[System.Management.Automation.Language.Parser]::ParseFile('ps-scripts/zen-detect-binary.ps1',[ref]$null,[ref]$null);'parse-ok'"`
Expected: `parse-ok`（行为在 lanPC 验：detect-binary 后 `install_record_written:true`）。

- [ ] **Step 3: 提交**

```bash
git add ps-scripts/zen-detect-binary.ps1
git commit -m "fix(zen-ps): detect install-dir zen.exe across all user profiles (F5)"
```

---

## Task 12: F6 (Rust, B 组) — apply-config dest-path 自动推导

**Files:**
- Modify: `src-tauri/src/cli/args.rs`（`ApplyConfig` 的 `dest_path` 改为 `Option<String>`）
- Modify: `src-tauri/src/cli/domain_zen.rs`（`apply_config` handler：dest 缺省时从 install dir 推导）

> 需先确认 `apply_config` handler 现状与 install-dir 来源。实现前 grep：`grep -n "fn apply_config\|dest_path\|ApplyConfig" src-tauri/src/cli/domain_zen.rs`。

- [ ] **Step 1: args.rs 把 `dest_path` 改可选**

把 `ApplyConfig { ... dest_path: String ... }` 中：

```rust
        #[arg(long, value_name = "PATH")]
        dest_path: String,
```

改为：

```rust
        /// Absolute zen.lua destination. Optional: when omitted, derived from
        /// the detected zen install dir on the target (…\Zen\Install\zen.lua).
        #[arg(long, value_name = "PATH")]
        dest_path: Option<String>,
```

并同步 `handle()` 里 `ZenAction::ApplyConfig { ... dest_path ... }` 解构与传参为 `Option<String>`。

- [ ] **Step 2: 写失败测试（domain_zen.rs 测试模块）—— 纯函数 `derive_lua_dest`**

> 只从 **install-dir** zen.exe（`…\Zen\Install\zen.exe`）推 `…\Zen\Install\zen.lua`。intree 的 zen.exe 在 `…\Engine\Binaries\Win64\`，不是 zen.lua 的正确位置，**不参与 dest 推导**。

```rust
#[test]
fn derive_lua_dest_from_install_zen_exe() {
    assert_eq!(
        super::derive_lua_dest(r"C:\Users\me\AppData\Local\UnrealEngine\Common\Zen\Install\zen.exe").as_deref(),
        Some(r"C:\Users\me\AppData\Local\UnrealEngine\Common\Zen\Install\zen.lua")
    );
    assert_eq!(super::derive_lua_dest("zen.exe").as_deref(), Some("zen.lua")); // no parent → cwd-relative ok
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cd src-tauri && cargo test --lib derive_lua_dest`
Expected: FAIL（`derive_lua_dest` 未定义）。

- [ ] **Step 4: 实现 helper + 在 `apply_config` 缺省推导（domain_zen.rs）**

helper（放在 `apply_config` fn 之前）：

```rust
/// Derive the zen.lua destination from an install-dir zen.exe path
/// (`…\Zen\Install\zen.exe` → `…\Zen\Install\zen.lua`).
fn derive_lua_dest(zen_exe: &str) -> Option<String> {
    let p = std::path::Path::new(zen_exe);
    let dir = p.parent();
    match dir {
        Some(d) if !d.as_os_str().is_empty() => {
            Some(format!("{}\\zen.lua", d.to_string_lossy()))
        }
        _ => Some("zen.lua".to_string()),
    }
}
```

`apply_config` handler 在用到 `dest_path` 之前插入（**只用 install-dir，不 fallback intree**）：

```rust
    let dest_path: String = match dest_path {
        Some(p) => p,
        None => {
            // F6: derive from the recorded install-dir zen.exe only. The intree
            // copy lives under Engine\Binaries\Win64 — wrong place for zen.lua —
            // so an intree-only machine still requires an explicit --dest-path.
            let install = machine_zen_install::find(&db, ep.machine_id)?;
            let zen_exe = install
                .as_ref()
                .and_then(|m| m.zen_cli_path.clone())
                .ok_or_else(|| UecmError::InvalidInput(format!(
                    "cannot derive --dest-path: machine id={} has no install-dir zen.exe \
                     recorded; run `zen detect-binary` or pass --dest-path explicitly",
                    ep.machine_id
                )))?;
            derive_lua_dest(&zen_exe).ok_or_else(|| UecmError::InvalidInput(
                "recorded zen.exe path has no usable parent dir".into()))?
        }
    };
```

- [ ] **Step 5: 跑测试确认通过 + 编译**

Run: `cd src-tauri && cargo test --lib derive_lua_dest && cargo build --lib`
Expected: 测试 PASS、编译通过。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/cli/args.rs src-tauri/src/cli/domain_zen.rs
git commit -m "feat(zen-cli): auto-derive apply-config --dest-path from install dir (F6)"
```

---

## Task 13: lanPC E2E runbook + 全量回归

**Files:**
- Create: `docs/superpowers/plans/2026-05-30-zen-cli-deploy-fixes-lanpc-runbook.md`

- [ ] **Step 1: mac 全量回归**

Run: `cd src-tauri && cargo test --lib`
Expected: 全绿（基线 ~997–1014 pass，新增 F1/F3/F4/F6 测试通过）。

- [ ] **Step 2: build release + 部署到 lanPC**

按 memory `deploy_tauri_build`：`pnpm tauri build --no-bundle`（不要直接 `cargo build --release`）；产物 + `ps-scripts/*` 复制到 lanPC `C:\Tools\UECM\`（CLI）与 `C:\ProgramData\UECM\ps-scripts\`（sidecar）。PS 改动需管理员同步（`Start-Process -Verb RunAs`）。

- [ ] **Step 3: 写 lanPC runbook 文档（逐条勾，期望 vs 实际）**

照 spec §4 lanPC runbook 的 1–9（含 6b F2 升级反例、7 F4 守卫反例），把每条命令 + 期望 JSON 字段写成 checklist。关键断言：
- step 3：`zen detect-binary --machine 13` → `install_record_written:true`（**F5** 跨用户生效）+ intree 记录 > 0。
- step 6b：对"有 data-dir 没 port"的旧服务再 install → `repaired:true` 且 `sc qc ZenServer` 的 ImagePath 含 `--port 8558 --http asio`（**F2 #1**）。
- step 7：`ZenServer` 服务在跑时 `zen sponsor-down --dry-run` → `refused:true, is_installed_service:true`（**F4 #2**）。
- step 8：`zen service start` 后 `zen probe` → `effective_port:8558`（不是 8658，**F2**）。

- [ ] **Step 4: 提交 runbook**

```bash
git add docs/superpowers/plans/2026-05-30-zen-cli-deploy-fixes-lanpc-runbook.md
git commit -m "docs(plan): lanPC E2E runbook for zen CLI deploy fixes"
```

---

## 完成判据

- mac：`cargo test --lib` 全绿（含 F1 fallback、F3 helper、F4 解析、F6 dest 推导单测）。
- lanPC：runbook 全部勾过——尤其 F2 升级反例（repaired + ImagePath 含 port）、F4 守卫反例（refuse on service）、端口稳定在 8558。
- A 组（Task 1–10 + runbook 中 A 组项）通过即可合 main；B 组（Task 11–12）可同批或后续追加。
