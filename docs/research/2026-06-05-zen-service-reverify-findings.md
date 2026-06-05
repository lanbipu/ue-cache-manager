# Zen service-install fixes — lanPC E2E 复验 findings（machine 13, 2026-06-05 第二轮）

> 复验对象：fix commit `7ee134c`（bugs 1–4），按 `docs/superpowers/plans/2026-06-05-zen-service-fixes-e2e-reverify.md` 在 lanPC 本机（WSL interop → Windows exe + WinRM 到 192.168.10.20）重跑。
> 重新 build 的 `C:\Tools\UECM\uecm-cli.exe` SHA256 = `E5251ABD68FEC133A5EC3FB8A053A732B284DAD4CE30F6C1B37145F335C99C7D`（21:46）；`ps-scripts/zen-service-install.ps1` 部署于 21:39，确认含 `Normalize-ZenExe` / `already_installed` / `repaired`，且 CLI 从 `<exe-dir>/ps-scripts` 解析（powershell.rs:55）→ 跑的就是修复版。
> endpoint 1：8558 / `F:\Epic\DDC\Zen` / asio / shared_upstream → installed_service。UE 5.8 primary `D:\Program Files\Epic Games\UE_5.8`。

## 判定（二轮，修完缺陷 A/B 后）：**五项关键断言全过**

- **第一轮**（部署 `7ee134c` 原样复验）：Bug 4 架构修复生效、Bug 3 + 断言③④ 过，但 Bug 4 把 exe 换到含空格 intree 路径、并让服务真启动后，顶出两个 fix commit 没覆盖的下游 ps 缺陷 → 断言② FAIL、断言① CLI start 冷启动偶发 FAIL。
- **修复**（lanPC，TDD）：缺陷 A（zen-service-install.ps1 分词器，新增 `Resolve-ServiceExe` + `Build-PatchedImagePath` 纯函数 + 单测）、缺陷 B（zen-up.ps1 一行 `-WarningAction SilentlyContinue`）。
- **第二轮**（部署修复后复验）：断言② already_installed + repaired **都过**，① start 返回干净 JSON。**五项全过。**

下表「实际」列已更新为**第二轮**结果；第一轮的原始 FAIL 输出保留在「新缺陷」节作为根因证据。

## 四个关键断言 + Bug 3

| 断言 | 期望 | 实际 | 判定 |
|---|---|---|---|
| **①** intree ImagePath + start RUNNING 常驻 | token0=intree `…UE_5.8…\zenserver.exe` 含 `--port 8558 --http asio`；start→RUNNING 且保持 | ImagePath = intree `D:\Program Files\Epic Games\UE_5.8\Engine\Binaries\Win64\zenserver.exe …--port 8558 --http asio`（**非** AppData）✓；服务 start→**RUNNING 且常驻**（PID 2404/15228/19444/26712），8558 listen ✓。修缺陷 B 后 `zen service start` 返回干净 JSON `{ok:true,status:Running}`（含 repair 后 stop→start 一次）。 | ✅ PASS（修 B 后） |
| **②** repaired:true + 幂等 already_installed | 干净重装→`already_installed:true`；port-drift→`repaired:true` | **第一轮**：`different ZenExePath / DataDir. Refusing`（根因缺陷 A）。**第二轮（修 A 后）**：幂等重装→`already_installed:true`（unquoted 与 repair 后 quoted ImagePath 都过）；造 unquoted port-drift 重装→`repaired:true`，`new_path_name="D:\…\zenserver.exe" --data-dir "F:\Epic\DDC\Zen" --port 8558 --http asio`，提示 stop+start 生效。 | ✅ PASS（修 A 后） |
| **③** 服务在跑时 sponsor-down refused | refused:true, is_installed_service:true, listener_pid=服务PID | `zen sponsor-down --dry-run`（服务跑、占 8558）→ `zen-sponsor-down.ps1` 输出 envelope `{ok:false, refused:true, is_installed_service:true}`，CLI 渲染为 error：**"port 8558 is served by the installed 'ZenServer' service (pid 2404), not an editor sponsor; use `zen service stop`"**。pid 2404 = 服务进程，提示正确。 | ✅ PASS |
| **④** 端口稳定 8558 | stop→start 仍 8558 | `zen service stop --yes`→Stopped、8558 FREE；再 start→Running、8558 listen（PID 19444）；`zen probe --machine 13` → `effective_port:8558, reachable:true, build_version:5.8.8-…`。 | ✅ PASS |
| **Bug 3** `--service-user LocalSystem` | ok（不再 1639/rollback） | `zen service install --service-user LocalSystem --yes` → `ok:true, sc_exit_code:0, service_account_applied:true`；`sc qc` → `SERVICE_START_NAME : LocalSystem`。 | ✅ PASS |

收尾：`cache-stats` providers = `["dashboard","http","prj","sessions","ws","z$"]`（含 `z$`）✓；`zen status --machine 13` → `lifecycle_mode:installed_service, ok:true`，latest_probe reachable ✓；`health run --machine-ids 13` → `healthy:12 / warning:4 / critical:3 / skipped:5`（多数 healthy，但有 3 项 critical，未细查）。

## 新缺陷（均为 Bug 4 修复后暴露，fix commit `7ee134c` 未覆盖；本轮已在 lanPC 修复 + 重部署 + 复验过）

### 缺陷 A（阻断断言②，**critical**）✅ 已修 — 幂等/drift 分词器吃不下"未加引号的含空格 exe 路径"

`ps-scripts/zen-service-install.ps1:295-313` 的 token 分词器只对**引号包裹**或**无空格**的 token0 正确。但 zen 注册服务时把 ImagePath 写成**不带引号**，而 Bug 4 修复把 exe 换到 intree 的 `D:\Program Files\Epic Games\UE_5.8\…`（**含空格**）。实证（在真实 `Win32_Service.PathName` 上模拟）：

```
CIM_PathName=[D:\Program Files\…\zenserver.exe --data-dir F:\Epic\DDC\Zen --port 8558 --http asio]
Starts_with_quote=False
token0=[D:\Program]            token_count=9
existingExe_normalized=[d:\zenserver.exe]            ← Normalize-ZenExe("D:\Program")→dir "D:\"→ d:\zenserver.exe
expectedExe_normalized=[d:\program files\epic games\ue_5.8\engine\binaries\win64\zenserver.exe]
exeMatches=False
```

→ line 451 `($existingExe -ne $expectedExe)` 恒真 → "different ZenExePath"。上一轮 exe 是 AppData 路径（无空格）所以分词没炸（那时 FAIL 是 zen.exe↔zenserver.exe 命名，已被 `Normalize-ZenExe` 修掉）；Bug 4 把 exe 移到含空格路径后，**分词成了新短板**。

连带影响：repair 分支（line 414）要 `$exeMatches -and $dirMatches -and $userMatches` 才触发，`exeMatches=False` → **repair 在本机原生 ImagePath 上同样不可达**（断言②的 repaired:true 因此也拿不到）。

**已修**：新增纯函数 `Resolve-ServiceExe`（贪婪重组到第一个 `.exe` 边界，处理 quoted / 未引号含空格 / 无空格三种形态）替换 token0 抽取；幂等比较 `$existingExe` 改用之。第二处同 bug 在 repair 重建 `$exePart`（`$curBin.TrimStart('"').Split('"')[0]` 同样假设带引号 → 第二轮 repair 触发后 `GetFullPath` 报"路径格式不支持"）—— 抽 `Build-PatchedImagePath` 纯函数复用 `Resolve-ServiceExe`。两函数加 `__tests__\zen-service-install.tests.ps1` 单测（unquoted-spaces / quoted / no-space / dir-with-.exe / bare / repair-rebuild 共 7 例，TDD 红→绿）。复验：幂等 `already_installed:true`、port-drift `repaired:true`。

### 缺陷 B（断言① 冷启动偶发，**medium**）✅ 已修 — `Start-Service` 本地化 warning 泄进 stdout 破 JSON

`ps-scripts/zen-up.ps1:65` `Start-Service -Name $ServiceName -ErrorAction Stop` 没加 `-WarningAction SilentlyContinue`。服务进 START_PENDING、Start-Service 等待时会往 **Warning 流**打本地化 "Waiting for service … to start"（zh-CN = "警告: 正在等待服务…启动…"），invoke wrapper 把它并进 stdout，顶在 JSON 前 → `parse_envelope` 报 `zen-up.ps1 returned non-JSON output: expected value at line 1 column 1; raw: 警告: 正在等待服务…`。

**timing-dependent**：仅在服务启动慢到触发 warning 时复现——本轮**首次全新冷装那一次**中招（zen 初始化 `F:\Epic\DDC\Zen` 较慢），之后 warm 启动均干净返回 `{ok:true,status:Running}`。脚本 line 75-86 本就有 post-start 复查判真失败，**抑制 warning 流安全**。

**已修**：`zen-up.ps1:65` 加 `-WarningAction SilentlyContinue`。脚本第 75-86 行的 post-start `WaitForStatus` + 复查仍是真·成功判据，抑制 warning 流安全。复验：repair 后 stop→start 返回干净 `{ok:true,status:Running}`。

## 其他观察（非回归，备注）

- `zen service uninstall` 对**正在 RUNNING** 的服务返回 `zen service uninstall failed (exit 1)`（zen.exe 自身要求先停）；`zen service stop --yes` 后 uninstall 即 `ok:true,zen_exit_code:0`。operator 流程需"先 stop 再 uninstall"。脚本只回传 exit code、未回传 zen.exe stdout，排障时看不到原因。
- `zen service stop` / `install` / `uninstall` / `sponsor-down` / `probe` / `cache-stats` / `status` 全部 JSON 正常，仅 `service start` 有缺陷 B。

## push 状态

- 复验发现 **`origin/main` 已在 `171753e`**——`7ee134c`(fix) + `debda37`/`e3c0622`(backflow) **早已 push 到 origin**（本地 main / origin/main / bundle tip 三者一致），虽然 fix message 写"unpushed until a lanPC E2E re-run confirms"。即原始修复已在 origin，但带缺陷 A/B。
- 本轮 A/B 修复是 origin 之上的**新增 fix-forward**：`ps-scripts/zen-service-install.ps1`（`Resolve-ServiceExe` + `Build-PatchedImagePath`）、`ps-scripts/zen-up.ps1`（WarningAction）、`ps-scripts/__tests__/zen-service-install.tests.ps1`（新）、本 findings。lanPC 无 tty / GCM 直 push 会失败 → 走 `git bundle` 搬回 mac push（见 plan 末「push」节）。

## 结论

- **五项关键断言（①②③④ + Bug 3）二轮全过**，push 判据成立。原始 `7ee134c` 的 Bug 4 架构修复 + Bug 3 是真实进展；缺陷 A/B 已在本轮 TDD 修掉并复验。
- 重新 build 的 CLI（含 Bug 4 Rust 修复）：SHA256 `E5251ABD68FEC133A5EC3FB8A053A732B284DAD4CE30F6C1B37145F335C99C7D`。
- 节点当前态：ZenServer 装好（默认 LocalService）、RUNNING、占 8558、installed_service / ok:true（留作正常运行态）；ImagePath 经 repair 后为带引号形态。
- 其他观察（非回归）：`zen service uninstall` 对 RUNNING 服务需先 `stop`（zen.exe 自身 exit 1，脚本未回传其 stdout，排障看不到原因）。

