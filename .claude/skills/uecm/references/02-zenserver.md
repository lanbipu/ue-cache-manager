# 流程 02 — ZenServer 共享 DDC 部署

约定 `BIN=/mnt/c/Tools/UECM/uecm-cli.exe`，远程命令带 `--cred-alias render-svc --output json --no-input`。

## 架构约束（先理解，否则会装错地方）

UE DDC 分三层：**ZenLocal**（工作站 localhost:8558，UE Editor AutoLaunch 自管）/ **ZenShared**
（LAN 共享服务器，UECM 的 `UECMZenServer` 服务管）/ Cloud。

- **ZenServer 服务必须装在独立服务器上**（不跑 UE Editor 的机器）。与工作站共机会争抢 8558 端口，
  UE 启动弹错误窗（F-036/F-039）。`zen service install` 在有 `ue_runtime_user` 的机器（=工作站信号）上
  会发 advisory 警告但不硬失败——看到这个警告说明你可能装错机器了。
- 工作站只做客户端 `zen enable`，**不装 zen 服务**，也**不写 `AutoLaunch=false`**（旧方案已回滚，UE 的 ZenLocal 保持正常）。
- 端口内嵌在 Host URI 里（`http://server:8558`）；UE 的 Zen store **没有独立 `Port=` 字段**，别想着单独传 port。

| 层 | 位置 | 管理者 | 端口 |
|---|---|---|---|
| ZenLocal | 工作站 localhost | UE Editor AutoLaunch | 8558 |
| ZenShared | 独立服务器 LAN IP | UECM `UECMZenServer` 服务 | 8558 |

---

## A. 服务端部署（在独立 Zen 服务器上）

先确认目标机已纳管（流程 01：add + refresh）。然后：

```bash
# 1) 注册 endpoint（角色 shared_upstream = 集群主；data-dir 按真实磁盘改，别用默认 D:\UECM\ZenData 除非合适）
"$BIN" zen register --machine <server_id> --role shared_upstream --declared-port 8558 --data-dir 'E:\ZenData'

# 2) 探测 zen 二进制（apply-config 的前置！自动选最高 UE 版本 in-tree zenserver.exe）
"$BIN" zen detect-binary --machine <server_id> --cred-alias render-svc

# 3) 渲染并写 zen.lua 到目标机（附 SHA256 校验）。先预览 lua 再写
"$BIN" zen lua-preview --endpoint-id <eid>                                  # 读：看将写什么
"$BIN" zen apply-config --endpoint-id <eid> --cred-alias render-svc --dry-run
"$BIN" zen apply-config --endpoint-id <eid> --cred-alias render-svc --yes

# 4) URL ACL（http.sys 端口保留）。principal 用 LocalService
"$BIN" zen urlacl add --endpoint-id <eid> --principal "NT AUTHORITY\LocalService" --cred-alias render-svc --dry-run
"$BIN" zen urlacl add --endpoint-id <eid> --principal "NT AUTHORITY\LocalService" --cred-alias render-svc --yes
# 已存在且同 SID 会返回 already_exists:true，不报冲突（F-020 修复后，大小写变体不误报）

# 5) 装服务（UECMZenServer）
"$BIN" zen service install --endpoint-id <eid> --cred-alias render-svc --dry-run
"$BIN" zen service install --endpoint-id <eid> --cred-alias render-svc --yes

# 6) 启服务
"$BIN" zen service start --endpoint-id <eid> --cred-alias render-svc

# 7) 探针验证可达
"$BIN" zen probe --machine <server_id> --cred-alias render-svc
"$BIN" zen status --machine <server_id> --output json        # reachable 应为 true
```

- endpoint id（`<eid>`）：`zen register` 返回，或 `zen list-endpoints` 查。
- `zen detect-binary` 不跑，`apply-config` 会报 "cannot derive --dest-path: no install-dir zen.exe recorded"（F-024）。

---

## B. 客户端配置（在每台工作站上）

```bash
# 前置：工作站必须先设 ue_runtime_user（zen enable --global 用它定位 UserEngine.ini）
"$BIN" machine set-ue-user --machine <ws_id> --ue-user <Windows用户名>

# 开 ZenShared upstream（--global 写 UserEngine.ini 全局生效；或 --project-id 只改某项目）
"$BIN" zen enable --upstream-endpoint-id <eid> --global --machines <ws_id> --cred-alias render-svc --dry-run
"$BIN" zen enable --upstream-endpoint-id <eid> --global --machines <ws_id> --cred-alias render-svc --yes
```

- ⚠️ `--global` 与 `--project-id` 二选一。`--global` 用 `ue_runtime_user` 算 `%LocalAppData%\Unreal Engine\Engine\Config\UserEngine.ini` 路径（F-021/F-038，路径必须是 Local 不是 Roaming，已修）。
- `zen enable` 会改 INI 写入 `[StorageServers] Shared=(Host="http://<host>:<port>", Namespace="ue.ddc", EnvHostOverride=UE-ZenSharedDataCacheHost, ...)`，并**自动清理** legacy `UE-SharedDataCachePath` env（无需手动，DESIGN-3 已澄清）。
- 独立服务器方案下**不写 `AutoLaunch=false`**——UE 的 ZenLocal 继续正常工作。

### 验证读回

```bash
"$BIN" ini read --host <ws_ip> --file '<UserEngine.ini路径>' --section StorageServers --cred-alias render-svc --output json
# 应看到 Shared=(Host="http://<server>:8558", ...)，端口内嵌在 Host 里
```

---

## C. 多区域路由（可选）

`zen enable` 写的 `EnvHostOverride=UE-ZenSharedDataCacheHost` 允许按机覆盖 Host，让某些工作站指向就近的 Zen 服务器：

```bash
# 裸 host:port 会自动规范化为 http://host:port
"$BIN" zen set-region-host --machines <ids> --host 192.168.20.10:8558 --cred-alias render-svc --yes
"$BIN" env get --host <ip> --name UE-ZenSharedDataCacheHost --cred-alias render-svc --output json   # 读回核对
# 还原到 INI 默认（清区域覆盖）
"$BIN" zen clean-env --machines <ids> --name UE-ZenSharedDataCacheHost --cred-alias render-svc --yes
```

---

## D. 关闭 / 清理

```bash
"$BIN" zen disable --global --machines <ws_id> --cred-alias render-svc --yes      # 移除 INI 里的 ZenShared 条目
"$BIN" zen clean-env --machines <ids> --name UE-SharedDataCachePath --cred-alias render-svc --yes   # 清遗留 SMB DDC 变量（走提权通道）
```

---

## E. 迁 / 重装服务（已存在 zen 服务时的陷阱）

目标机已有 ZenServer 服务（如 UE Hub 装的）时，`zen service install` 会拒绝（exit 4，"already installed"），
且 WinRM 受限 token（UAC 过滤 SCM 写）下 `zen service uninstall` 也会 exit 1。正确顺序（F-019）：

```bash
# 1) 先经提权 SSH 通道停服（WinRM 做不了）
UecmKey='C:\Users\lanPC\AppData\Roaming\com.lanbipu.uecm\uecm_ed25519'
ssh -i "$UecmKey" uecm-svc@<IP> "powershell Stop-Service ZenServer -Force"
# 2) 再让 UECM 卸载（已停的服务可无冲突注销）
"$BIN" zen service uninstall --endpoint-id <eid> --cred-alias render-svc --yes
# 3) 重新安装
"$BIN" zen service install --endpoint-id <eid> --cred-alias render-svc --yes
```
> 提权 SSH 通道细节见 `troubleshooting.md`。注意 UE Hub 的 zen（AppData）与 in-tree zenserver.exe 可能不同步（F-023）——UECM 管的是 in-tree 那个。

---

## F. health 验证（Zen 模式）

```bash
"$BIN" zen probe --machine <ids> --cred-alias render-svc          # 必须先刷探针！
"$BIN" zen cache-stats --endpoint-id <eid>
"$BIN" health run --machine-ids <ids> --cred-alias render-svc --output json
```
- Zen 模式下 `env_shared/env_vars` 会自动降级为 `na`（不再误报 critical，DESIGN-1）。
- `zen_reachable` 若报 critical 但服务实际在跑——多半是 probe 数据过 5 分钟窗口（F-043），重跑 `zen probe` 即恢复。

---

## 速查：完整 ZenServer 部署

```
# 服务端（独立服务器）
zen register --machine <server_id> --role shared_upstream --declared-port 8558 --data-dir E:\ZenData
zen detect-binary --machine <server_id> --cred-alias render-svc
zen apply-config --endpoint-id <eid> --cred-alias render-svc --yes
zen urlacl add --endpoint-id <eid> --principal "NT AUTHORITY\LocalService" --cred-alias render-svc --yes
zen service install --endpoint-id <eid> --cred-alias render-svc --yes
zen service start  --endpoint-id <eid> --cred-alias render-svc
zen probe --machine <server_id> --cred-alias render-svc
# 客户端（每台工作站）
machine set-ue-user --machine <ws_id> --ue-user <用户名>
zen enable --upstream-endpoint-id <eid> --global --machines <ws_id> --cred-alias render-svc --yes
```
（每个 `--yes` 前先 `--dry-run` 预览。）
