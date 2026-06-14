# 流程 01 — 机器纳管 + 两级 DDC 缓存

约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`，远程命令统一带 `--cred-alias <别名> --output json --no-input`。
渲染节点统一运维凭据见 CLAUDE.md（`uecm-svc` / `UecmRender@2026`）。

---

## A. 机器纳管

### A0. 准备凭据别名（一次性）

```bash
# 用 --pass-stdin 避免密码进 shell 历史
printf '%s' 'UecmRender@2026' | "$BIN" cred save --alias render-svc --user uecm-svc --pass-stdin
"$BIN" cred list                    # 确认存进去了
```

### A1. 扫描网段（可选，纯探测）

```bash
"$BIN" machine scan 192.168.10.0/24 --output json
```
- 只做 TCP 端口探测（SSH/SMB），**不写库**。结果只在本次输出。
- `scan` 发现候选 → 下一步 `machine add` 才真正录入。这两步是独立的（F-003）。

### A2. 新机器先 Bootstrap（新机未开 SSH 时的唯一现场步骤）

新机器在 bootstrap 前 SSH 未开，UECM 连不上，必须人工介入一次：

```bash
"$BIN" ssh package-bootstrap --out 'C:\Temp\UECM-Bootstrap'    # [写] 生成引导包
```
- 把生成的包**人工传到新机器**（U 盘/共享）→ 在新机**双击 `UECM-Bootstrap.cmd`**（开 SSH + 建 `uecm-svc` 账号入 Administrators 组）。
- ⚠️ 告诉用户：这步不会自动发生，`package-bootstrap` 只生成包不远程执行（F-017）。已通 SSH 的机器可跳过。

### A3. 录入 + 刷新 + 设 UE 账号

```bash
"$BIN" machine add --ip 192.168.10.21 --output json          # [写] 返回 id，记下它
"$BIN" machine refresh <id> --cred-alias render-svc          # [写] SSH 探测主机名/UE安装/GPU 写回 DB
"$BIN" machine detail <id> --output json                     # 读：核对 UE 安装、物理 GPU（虚拟适配器已过滤）
"$BIN" machine set-ue-user --machine <id> --ue-user <Windows用户名>   # [写] zen enable --global 的前置，纳管时就设好
```
- ⚠️ `machine add` 的 IP 是 `--ip` flag，不是位置参数（F-004）。
- `machine refresh` 只填 UE 安装/GPU，**不填 Zen 路径**（Zen 路径要 `zen detect-binary`，见流程 02）（F-006）。
- 想一步到位可用 `machine deep-scan --machine-ids <id> --cred-alias render-svc`（= refresh + ini scan + health）。

---

## B. 两级 DDC 缓存配置

UE DDC 两级缓存：**本地缓存**（本机磁盘）+ **共享缓存**（SMB 网络目录）。
读：先查本地→未命中查共享→共享命中回填本地→都未命中重编译并同时写两级。
> 注：若该集群走 **ZenServer 共享 DDC**，共享层用 Zen 而非 SMB——那走流程 02，不要同时配 SMB 共享缓存。本节是传统 SMB 两级缓存路径。

### B1. 本地缓存（顺序不能反：先建目录，再写变量）

```bash
# 1) 先建目录（env set 不会自动建目录）
"$BIN" local-cache create --host 192.168.10.21 --path 'D:\UE-DDC-Local' --cred-alias render-svc --dry-run   # 预览
"$BIN" local-cache create --host 192.168.10.21 --path 'D:\UE-DDC-Local' --cred-alias render-svc --yes
# 2) 再写环境变量指向它
"$BIN" env set --host 192.168.10.21 --name UE-LocalDataCachePath --value 'D:\UE-DDC-Local' --cred-alias render-svc --dry-run
"$BIN" env set --host 192.168.10.21 --name UE-LocalDataCachePath --value 'D:\UE-DDC-Local' --cred-alias render-svc --yes
```
- ⚠️ 顺序：`local-cache create`（建目录）→ `env set`（写变量）。反了变量会指向不存在的目录（F-009）。
- ⚠️ 命令是 `local-cache create` 不是 `provision`。

### B2. 共享缓存服务器（建 SMB 共享）

```bash
"$BIN" share create --mode a --host 192.168.10.20 --share DDC-Shared --local-path 'F:\Epic\DDC\Shared' --cred-alias render-svc --dry-run
"$BIN" share create --mode a --host 192.168.10.20 --share DDC-Shared --local-path 'F:\Epic\DDC\Shared' --cred-alias render-svc --yes
"$BIN" share list --output json     # 读回 UNC 路径，如 \\LANPC\DDC-Shared
```
- `--host` 是承载共享的服务器（别漏）。mode a=open / mode b=dedicated ddc-svc。
- 记下返回的 UNC 路径，下一步要用。

### B3. 写共享缓存变量（两个变量都要设！）

```bash
# 在每台要用共享缓存的机器上设 UE-SharedDataCachePath = 共享 UNC
"$BIN" env set --hosts 192.168.10.21,192.168.10.22 --name UE-SharedDataCachePath --value '\\LANPC\DDC-Shared' --cred-alias render-svc --dry-run
"$BIN" env set --hosts 192.168.10.21,192.168.10.22 --name UE-SharedDataCachePath --value '\\LANPC\DDC-Shared' --cred-alias render-svc --yes
```
- ⚠️ DDC 完整配置需要**两个**环境变量：`UE-LocalDataCachePath`（B1）+ `UE-SharedDataCachePath`（本步）。漏第二个 health 会报 env_shared critical（F-015）。
- 多机用 `--hosts a,b,c`（逗号分隔），单机用 `--host`。
- ⚠️ Machine-scope 变量写入在 WinRM 受限 token 下可能被 UAC 拒——见 `troubleshooting.md` 提权通道。

### B4. 让 INI 的 Shared.Path 指向这个 UNC（若 ini scan 报 R021）

`share create` 得到的 UNC（如 `\\LANPC\DDC-Shared`）就是 ini scan 里 R021（Shared.Path 非 UNC）的修复值。
R021 是 `manual` 类 finding（CLI 不知道该填什么），用 `ini set` 人工填（流程 04 详述）：

```bash
"$BIN" ini set --host 192.168.10.21 --file '...\BaseEngine.ini' --section DerivedDataBackendGraph --key Shared.Path --value '\\LANPC\DDC-Shared' --cred-alias render-svc --yes
```

---

## C. 复验

```bash
"$BIN" zen probe --machine <ids> --cred-alias render-svc      # health 前先刷探针（即使没用 zen 也无害）
"$BIN" health run --machine-ids <ids> --cred-alias render-svc --output json
"$BIN" health runs --limit 1                                  # 拿最新 run_id
"$BIN" health results <run_id> --output json                 # 逐行看，env_local/env_shared 应 healthy
```

---

## 速查：纳管一台新机 + 配两级缓存（端到端）

```
cred save --alias render-svc --user uecm-svc --pass-stdin
ssh package-bootstrap --out C:\Temp\UECM-Bootstrap   # 新机才需要；人工传送+双击
machine add --ip <IP>                                 # → id
machine refresh <id> --cred-alias render-svc
machine set-ue-user --machine <id> --ue-user <用户名>
local-cache create --host <IP> --path D:\UE-DDC-Local --cred-alias render-svc --yes
env set --host <IP> --name UE-LocalDataCachePath --value D:\UE-DDC-Local --cred-alias render-svc --yes
share create --mode a --host <共享服务器IP> --share DDC-Shared --local-path F:\Epic\DDC\Shared --cred-alias render-svc --yes
env set --host <IP> --name UE-SharedDataCachePath --value \\SERVER\DDC-Shared --cred-alias render-svc --yes
zen probe --machine <id> --cred-alias render-svc
health run --machine-ids <id> --cred-alias render-svc
```
（每个 `--yes` 前先用 `--dry-run` 给用户看预览。）
