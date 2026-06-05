# UE Cache Manager CLI 运行流程

**文档目的**：通过 CLI 把 UECM 各项功能完整跑一遍并记录结果，供后续 Skill 开发、UI 界面功能验证、部署工作参考。

**执行环境**：lanPC（`C:\Tools\UECM\uecm-cli.exe`），部分命令（标注 `跨平台` 的）可在 mac/Linux 执行。

**记录约定**：每个命令执行后在 `结果` 行填写：`✅ 正常` / `❌ 报错：<信息>` / `⏭ 跳过：<原因>`。

---

## 一、机器管理（`machine`）

> 目标：确认机器库存、SSH 连通状态、UE/GPU 信息完整。
>
> **执行顺序**：先扫描网络发现主机 → 将新主机加入库存 → 列出库存确认结果。

### 1.1 网络扫描发现新机器（跨平台）

```powershell
uecm-cli.exe machine scan 192.168.10.0/24 --timeout-ms 1000 --output json
```

- 结果：
- 记录：发现的活跃 IP（`smb_open`/`rpc_open` 为 true 的）

---

### 1.2 将发现的主机加入库存（如有新机器）

```powershell
# 示例：添加尚未在库存中的 IP
uecm-cli.exe machine add --ip 192.168.10.X --hostname <name>
```

- 结果：

---

### 1.3 列出所有机器（确认库存）（跨平台）

```powershell
uecm-cli.exe machine list --output json
```

- 结果：
- 记录：机器 id 列表（后续命令依赖）

---

### 1.3 刷新机器（SSH 探测 + UE/GPU 检测）

> 需要 SSH 连通（`uecm-svc` key auth）

```powershell
# lanPC (id=13)
uecm-cli.exe machine refresh 13 --output json

# Razer (id=12，需先 SSH 纳管)
uecm-cli.exe machine refresh 12 --output json
```

- 结果（lanPC）：
- 结果（Razer）：

---

### 1.4 查看机器详情

```powershell
uecm-cli.exe machine detail 13 --output json
uecm-cli.exe machine detail 12 --output json
```

- 结果（lanPC）：
- 结果（Razer）：

---

### 1.5 深度扫描（UE/GPU + INI + Health 一体化）

```powershell
uecm-cli.exe machine deep-scan --machine-ids 13,12 --output json
```

- 结果：

---

## 二、SSH 传输 / 节点纳管（`ssh`）

> 目标：确认所有节点 SSH 可达，Razer 如不可达则重新纳管。

### 2.1 探测 SSH 连通性

```powershell
uecm-cli.exe ssh probe 192.168.10.20 --output json
uecm-cli.exe ssh probe 192.168.10.173 --output json
```

- 结果（lanPC 192.168.10.20）：
- 结果（Razer 192.168.10.173）：

---

### 2.2 打包 Bootstrap 安装包（Razer 未纳管时）

```powershell
uecm-cli.exe ssh package-bootstrap --out "C:\Tools\UECM\bootstrap-bundle" --output json
```

- 结果：
- 后续动作：将 bundle 复制到 Razer，双击 `UECM-Bootstrap.cmd`，再重跑 2.1

---

## 三、凭据管理（`cred` / `secret`）

> 目标：确认 `uecm-svc` 凭据别名已正确录入。

### 3.1 列出已注册凭据别名

```powershell
uecm-cli.exe cred list --output json
```

- 结果：

---

### 3.2 保存标准运维凭据（如不存在）

```powershell
uecm-cli.exe cred save --alias render-svc --user uecm-svc --pass-stdin --kind winrm
# stdin 输入：UecmRender@2026
```

- 结果：

---

### 3.3 SecretStore 验证

```powershell
uecm-cli.exe secret list
uecm-cli.exe secret get render-svc
```

- 结果：

---

## 四、远程环境变量（`env`）

> 目标：确认各节点 `UE-LocalDataCachePath` / `UE-SharedDataCachePath` 已设置。

### 4.1 读取环境变量

```powershell
uecm-cli.exe env get --host 192.168.10.20 --name UE-LocalDataCachePath --output json
uecm-cli.exe env get --host 192.168.10.20 --name UE-SharedDataCachePath --output json
```

- 结果：

---

### 4.2 写入环境变量（如需修正）

```powershell
uecm-cli.exe env set --host 192.168.10.20 --name UE-LocalDataCachePath --value "D:\UE-DDC-Local" --yes --output json
```

- 结果：

---

## 五、INI 配置管理（`ini`）

> 目标：扫描集群 INI 配置，定位并修复已知问题。

### 5.1 集群 INI 扫描

```powershell
uecm-cli.exe ini scan --machine-ids 13,12 --output json
```

- 结果：
- 记录：scan_run_id =

---

### 5.2 查看扫描结论

```powershell
uecm-cli.exe ini runs --limit 5
uecm-cli.exe ini findings <scan_run_id> --output json
```

- 结果：

---

### 5.3 查看 DDC/PSO/Zen 配置快照

```powershell
uecm-cli.exe ini config <scan_run_id> --output json
uecm-cli.exe ini config <scan_run_id> --domain zen --output json
```

- 结果：

---

### 5.4 自动修复 finding（可选）

```powershell
# 先 dry-run 确认修复内容
uecm-cli.exe ini apply <finding_id> --dry-run
# 确认无误后执行
uecm-cli.exe ini apply <finding_id> --yes
```

- 结果：

---

### 5.5 BackendGraph 扫描

```powershell
uecm-cli.exe ini backend-graph scan --host 192.168.10.20 --file-path "C:\...\DefaultEngine.ini" --output json
```

- 结果：

---

## 六、SMB Share 管理（`share`）

> 目标：确认 DDC Shared 路径 SMB 共享已创建并可访问。

### 6.1 列出已注册 Share

```powershell
uecm-cli.exe share list --output json
```

- 结果：

---

### 6.2 创建 SMB Share（如需）

```powershell
# Mode A：开放 Guest+Everyone
uecm-cli.exe share create --mode a --host 192.168.10.20 --share DDC-Shared --local-path "F:\Epic\DDC\Shared" --yes --output json
```

- 结果：

---

## 七、项目管理（`project`）

> 目标：确认项目库存正确，所有节点的 project_locations 已配置。

### 7.1 列出所有项目

```powershell
uecm-cli.exe project list --output json
```

- 结果：
- 记录：project_id 列表

---

### 7.2 查看项目位置（各机器路径）

```powershell
uecm-cli.exe project locations <project_id> --output json
```

- 结果：

---

### 7.3 远程发现 .uproject 文件

```powershell
uecm-cli.exe project discover --machine-id 13 --roots "E:\Projects,D:\Projects" --output json
```

- 结果：

---

### 7.4 为 Razer 添加项目位置（zen enable 前置条件）

```powershell
uecm-cli.exe project set-location \
  --project-id <project_id> \
  --machine-id 12 \
  --abs-path "C:\Projects\<ProjectName>" \
  --uproject-path "<ProjectName>.uproject"
```

- 结果：

---

## 八、集群健康检查（`health`）

> 目标：获取完整健康报告，确认 L1/L2/L3 全部通过。

### 8.1 运行全量健康探测

```powershell
uecm-cli.exe health run --machine-ids 13,12 --output json
```

- 结果：
- 记录：scan_run_id =

---

### 8.2 查看健康结果

```powershell
uecm-cli.exe health results <scan_run_id> --output json
```

- 结果：

---

### 8.3 跨机器一致性检查

```powershell
uecm-cli.exe health consistency-check --hosts 192.168.10.20,192.168.10.173 --output json
```

- 结果：

---

### 8.4 DDC 文件统计

```powershell
uecm-cli.exe health file-stats \
  --host 192.168.10.20 \
  --local-path "D:\UE-DDC-Local" \
  --shared-path "F:\Epic\DDC\Shared" \
  --output json
```

- 结果：

---

## 九、GPU 矩阵（`gpu`）

```powershell
uecm-cli.exe gpu matrix --output json
```

- 结果：

---

## 十、DDC Pak 工作流（`ddc`）

> 目标：生成 DDC Pak → 验证 → 分发到渲染节点。

### 10.1 生成 DDC Pak

```powershell
uecm-cli.exe ddc generate \
  --project-id <project_id> \
  --source-machine 13 \
  --backend auto \
  --output json
```

- 结果：

---

### 10.2 验证 Pak 文件

```powershell
uecm-cli.exe ddc verify \
  --project-id <project_id> \
  --source-machine 13 \
  --output json
```

- 结果：

---

### 10.3 分发 Pak 到 Razer

```powershell
uecm-cli.exe ddc distribute \
  --project-id <project_id> \
  --source-machine 13 \
  --targets 12 \
  --yes \
  --output json
```

- 结果：

---

## 十一、PSO 缓存工作流（`pso`）

> 目标：验证 PSO CVars → 收集 PSO → 分发。

### 11.1 验证 PSO CVars

```powershell
uecm-cli.exe pso verify --project-id <project_id> --output json
```

- 结果：

---

### 11.2 收集 PSO（长任务）

```powershell
uecm-cli.exe pso collect \
  --project-id <project_id> \
  --source-machine 13 \
  --resolution 1920x1080 \
  --max-minutes 10 \
  --output json
```

- 结果：

---

### 11.3 分发 PSO 到 Razer

```powershell
uecm-cli.exe pso distribute \
  --project-id <project_id> \
  --source-machine 13 \
  --targets 12 \
  --yes \
  --output json
```

- 结果：

---

## 十二、DDC 日志验证（`log`）

```powershell
uecm-cli.exe log verify-startup \
  --host 192.168.10.20 \
  --editor-exe "C:\Epic\UE_5.X\Engine\Binaries\Win64\UnrealEditor.exe" \
  --project "C:\Projects\<Name>\<Name>.uproject" \
  --timeout 180 \
  --output json
```

- 结果：

---

## 十三、Zen DDC 集成（`zen`）

> 目标：完整走完 lanPC shared_upstream + Razer local 的建立流程。

### 13.1 查看现有端点

```powershell
uecm-cli.exe zen list-endpoints --output json
uecm-cli.exe zen status --all --output json
```

- 结果：
- 记录：当前端点 id

---

### 13.2 注册 lanPC 为 shared_upstream

```powershell
uecm-cli.exe zen register \
  --machine 13 \
  --role shared_upstream \
  --data-dir "F:\Epic\DDC\Zen" \
  --declared-port 8558 \
  --lifecycle installed_service \
  --httpserverclass asio \
  --output json
```

- 结果：
- 记录：endpoint_id =

---

### 13.3 推送 zen.lua 配置（先 dry-run）

```powershell
# 预览
uecm-cli.exe zen lua-preview --endpoint-id <ep_id>

# 推送（需指定 dest-path，T2.9 前必须手动提供）
uecm-cli.exe zen apply-config \
  --endpoint-id <ep_id> \
  --dest-path "C:\Users\uecm-svc\AppData\Local\UnrealEngine\Common\Zen\Install\zen.lua" \
  --dry-run

uecm-cli.exe zen apply-config \
  --endpoint-id <ep_id> \
  --dest-path "C:\Users\uecm-svc\AppData\Local\UnrealEngine\Common\Zen\Install\zen.lua" \
  --yes
```

- 结果（dry-run）：
- 结果（实际推送）：

---

### 13.4 安装 ZenServer Windows Service（lanPC）

```powershell
# dry-run
uecm-cli.exe zen service install --endpoint-id <ep_id> --dry-run

# 安装
uecm-cli.exe zen service install --endpoint-id <ep_id> --yes
```

- 结果：

---

### 13.5 启动服务 + 验证状态

```powershell
uecm-cli.exe zen service start --endpoint-id <ep_id>
uecm-cli.exe zen service status --endpoint-id <ep_id> --output json
uecm-cli.exe zen probe --machine 13 --output json
uecm-cli.exe zen cache-stats --endpoint-id <ep_id> --output json
```

- 结果（start）：
- 结果（status）：
- 结果（probe）：
- 结果（cache-stats）：

---

### 13.6 注册 Razer 为 local（需先 SSH 纳管）

```powershell
uecm-cli.exe zen register \
  --machine 12 \
  --role local \
  --upstream-endpoint-id <lanPC_ep_id> \
  --data-dir "C:\UECM\ZenData" \
  --declared-port 8558 \
  --lifecycle installed_service \
  --httpserverclass asio \
  --output json
```

- 结果：
- 记录：Razer endpoint_id =

---

### 13.7 Razer zen apply-config + service install + start

```powershell
uecm-cli.exe zen apply-config --endpoint-id <razer_ep_id> \
  --dest-path "C:\Users\uecm-svc\AppData\Local\UnrealEngine\Common\Zen\Install\zen.lua" --yes

uecm-cli.exe zen service install --endpoint-id <razer_ep_id> --yes
uecm-cli.exe zen service start --endpoint-id <razer_ep_id>
uecm-cli.exe zen service status --endpoint-id <razer_ep_id> --output json
```

- 结果：

---

### 13.8 设置 UE 运行用户（仅 `--global` 模式需要）

> 如果用 `zen enable --project-id` 模式可跳过此步。
> `--global` 模式会改写用户级 `UserEngine.ini`，路径依赖这里设置的 Windows 用户名。

```powershell
uecm-cli.exe machine set-ue-user --machine 13 --ue-user lanpc
uecm-cli.exe machine set-ue-user --machine 12 --ue-user lanbp
```

- 结果（lanPC）：
- 结果（Razer）：

---

### 13.9 启用项目 ZenShared（两台机器）

```powershell
uecm-cli.exe zen enable \
  --project-id <project_id> \
  --upstream-endpoint-id <lanPC_ep_id> \
  --machines 13,12 \
  --namespace ue.ddc \
  --dry-run

uecm-cli.exe zen enable \
  --project-id <project_id> \
  --upstream-endpoint-id <lanPC_ep_id> \
  --machines 13,12 \
  --namespace ue.ddc \
  --yes
```

- 结果（dry-run）：
- 结果（实际）：

---

### 13.10 验证 ZenShared INI 规则

```powershell
uecm-cli.exe zen verify-rules \
  --ue-version 5.X \
  --ue-install "C:\Epic\UE_5.X" \
  --output json
```

- 结果：

---

### 13.11 Baseline 管理

```powershell
uecm-cli.exe zen baseline list --output json
```

- 结果：

---

## 十四、一键 DDC 部署计划（`deploy`）

> 目标：验证 deploy ddc 计划文件驱动能力。

```powershell
uecm-cli.exe deploy ddc \
  --plan "docs/superpowers/examples/deploy-ddc-plan.example.json" \
  --dry-run
```

- 结果：

---

## 十五、系统自检（`system` / `manifest`）

```powershell
uecm-cli.exe system version
uecm-cli.exe system db-path
uecm-cli.exe system ps-dir
uecm-cli.exe system schema --output json
uecm-cli.exe system exit-codes
uecm-cli.exe manifest --output json
```

- 结果：

---

## 执行状态总览

| 功能域 | 状态 | 备注 |
|---|---|---|
| machine | ⬜ 待执行 | |
| ssh | ⬜ 待执行 | Razer 需先纳管 |
| cred / secret | ⬜ 待执行 | |
| env | ⬜ 待执行 | |
| ini | ⬜ 待执行 | |
| share | ⬜ 待执行 | |
| project | ⬜ 待执行 | Razer location 需配置 |
| health | ⬜ 待执行 | |
| gpu | ⬜ 待执行 | |
| ddc | ⬜ 待执行 | |
| pso | ⬜ 待执行 | |
| log | ⬜ 待执行 | |
| zen（lanPC）| ⬜ 待执行 | 已完全卸载，待重建 |
| zen（Razer）| ⬜ 待执行 | 依赖 SSH 纳管 |
| deploy | ⬜ 待执行 | |
| system / manifest | ⬜ 待执行 | |

---

*最后更新：2026-06-02*
