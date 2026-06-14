# 排错与限制

约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`。

## 退出码 → 怎么办

| Code | 名称 | 常见原因 / 处理 |
|---|---|---|
| 0 | ok | 成功 |
| 1 | operation_failed | 业务运行时失败。读 JSON `error.message`；远程操作多半是目标机状态/权限问题 |
| 2 | invalid_input | 参数无效，**或破坏性命令缺 `--yes`**（这是安全门，补 `--yes` 或先 `--dry-run`）|
| 3 | environment_error | DB/配置/IO。检查 `system db-path` 是否对、DB 是否 migrate 过 |
| 4 | powershell_failed | 远程 PowerShell 失败。常见：凭据错、WinRM 受限 token UAC 拒、目标机不可达 |
| 64 | usage_error | 命令行语法错。多半是位置参数/flag 用反了（见下） |

## 最常见的低级错（位置参数 vs flag）

| 错 | 对 |
|---|---|
| `machine add 192.168.10.20` | `machine add --ip 192.168.10.20` |
| `system echo --message hi` | `system echo hi`（位置参数） |
| `system check` | 没这命令；自检用 `system echo` + `system db-path` |
| `local-cache provision ...` | `local-cache create ...`（旧文档笔误） |
| `share create --mode a --share X --local-path Y`（漏 host） | 必带 `--host <服务器>` |
| `ddc generate --project-id 65`（漏 source） | 必带 `--source-machine <id>` |

## 提权 SSH 通道（WinRM 做不了的操作）

WinRM 会话即使用 Administrators 组账号（uecm-svc），UAC 也会过滤 token，导致以下操作被拒：
- 写/删 **Machine-scope** 环境变量
- Stop / Uninstall **Windows 服务**

多数已被 UECM 命令内部封装（`zen clean-env`、`zen service stop/uninstall` 会走提权通道）。
但少数场景（如重装已存在的 zen 服务，需先停掉 UE Hub 装的 `ZenServer`）仍需手动经提权 SSH：

```bash
UecmKey='C:\Users\lanPC\AppData\Roaming\com.lanbipu.uecm\uecm_ed25519'
# 注意：用 Windows ssh.exe + uecm_ed25519 key 连 uecm-svc（不是 WSL 的 ssh，也不是密码认证）
ssh -i "$UecmKey" uecm-svc@<IP> "powershell Stop-Service ZenServer -Force"
```
- key 路径：`C:\Users\lanPC\AppData\Roaming\com.lanbipu.uecm\uecm_ed25519`
- 从 lanPC WSL 里要用 **Windows 的 ssh.exe**（`/mnt/c/Windows/System32/OpenSSH/ssh.exe`）+ 该 key 连 uecm-svc；WSL 的 ssh 连不上目标 22。

### 权限矩阵
| 操作 | WinRM(uecm-svc) | 提权 SSH(uecm-svc) |
|---|---|---|
| 读文件/注册表 | ✅ | ✅ |
| 写 Machine-scope 环境变量 | ❌ UAC 过滤 | ✅ |
| Stop/Uninstall 服务 | ❌ UAC 过滤 | ✅ |
| 创建 SMB 共享 | ✅（在 Admins 组） | ✅ |
| 写 Program Files 下 INI | ✅（在 Admins 组） | ✅ |

## 平台 / 路径限制

- **Windows-only**：除 `machine scan` / `machine list` 外所有命令只能在 Windows 跑。当前会话在 lanPC WSL，
  经 interop 调 `.exe` 算 Windows，OK。纯 Linux/mac 调会直接报 Windows-only。
- **路径一律 Windows 格式**：`E:\...`、`\\HOST\Share`、`D:\Program Files\...`。WSL/Unix 路径（`/tmp/...`）会
  "找不到文件"（F-035）。给 CLI 写临时文件（deploy plan 等）写到 `C:\Temp\` 或仓库 `E:\...`。
- **JSON 里的路径**：反斜杠转义为 `\\`，UNC 为 `\\\\SERVER\\Share`。

## "看起来像 bug 其实不是"

- `health run` `zen_reachable` = critical 但服务在跑 → probe 数据过 5 分钟窗口（F-043）。**先 `zen probe` 再 `health run`**。
- Zen 模式下 `env_shared/env_vars` = na（不是 critical）→ 正常，DESIGN-1 故意降级。
- `ddc generate/verify --backend auto` 返回 `skipped:true` → 正常，Zen 原生缓存不需要 pak（F-027/F-028）。
- `zen urlacl add` 返回 `already_exists:true` → 正常，同 SID 不算冲突（F-020）。
- `ini apply` summary 的 `backup_path` 在 tuple 改路径下是操作描述不是 .bak 路径（F-041）——别据它找备份。
- `pso collect`/`log verify-startup` 对重型 VP 项目（nDisplay/ControlRig/Python）超时 → editor shutdown hang，UE 项目侧问题，核心功能实际工作（F-044）。
- `health file-stats` shared 端 not found → SMB 共享环境层没真生效，命令本身没错（F-042）。

## 部署 / 维护注意

- **PS 脚本必须与 binary 同步**：解析顺序 `UECM_PS_DIR` > `<exe目录>/ps-scripts` > `repo/ps-scripts`。
  每次更新 `uecm-cli.exe` 必须同步复制 `ps-scripts/*.ps1` 到 `C:\Tools\UECM\ps-scripts\`，否则修过的脚本不生效。
- **改 `.cmd`（bootstrap）**：必须 CRLF + 纯 ASCII，中文只能放 README.txt，否则 Windows 双击报错且账号不建。
- **binary 可能比源码旧/新**：核对命令以重新 build 出的 `--help` / `system schema` 为准，不以源码 args.rs 想当然。
- 自检三件套：`system version` / `system db-path` / `system echo ping`。
