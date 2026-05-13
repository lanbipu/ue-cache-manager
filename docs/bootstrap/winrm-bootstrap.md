# UECM WinRM Bootstrap

## 结论

如果目标 Windows 机器没有任何远程管理入口，UECM 主控机无法凭空执行命令。需要一次性 bootstrap。完成后，日常 UE/GPU refresh、环境变量、INI、DDC、PSO 等操作应回到 UECM 主控机远程执行。

## 适用场景

- 目标机没有 `SSH`。
- 目标机没有启用 `WinRM`。
- 目标机无法通过 `SMB ADMIN$` / `RPC` / `Service Control Manager` 被远程初始化。
- 没有可用的 `GPO` / `Intune` / `SCCM` / `RMM` 下发通道。
- 需要通过 U 盘、共享盘、镜像模板或人工一次性执行脚本完成首次纳管。

## 脚本

源脚本：

```text
ps-scripts/enable-winrm.ps1
```

远程自动 bootstrap 脚本：

```text
ps-scripts/bootstrap-winrm-remote.ps1
```

USB 包生成脚本：

```text
ps-scripts/package-winrm-bootstrap.ps1
```

生成 USB 包：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\ps-scripts\package-winrm-bootstrap.ps1 -OutputDirectory E:\UECM-WinRM-Bootstrap
```

目标机本地执行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1
```

## 默认开启的权限和系统设置

`enable-winrm.ps1` 默认会做这些修改：

- 启动 `WinRM` service。
- 设置 `WinRM` startup type 为 `Automatic`。
- 执行 `Enable-PSRemoting -Force -SkipNetworkProfileCheck`。
- 执行 `winrm quickconfig -q`。
- 启用 Windows Firewall 里的 `Windows Remote Management` rule group。
- 如果当前 active network profile 是 `Public`，改成 `Private`。
- 验证 `Test-WSMan localhost`。

默认不会做这些事：

- 不启用 `SSH`。
- 不安装 UECM persistent agent。
- 不启用 `Basic` authentication。
- 不启用 `CredSSP`。
- 不设置 `AllowUnencrypted`。
- 不启用 WinRM HTTPS `5986`。
- 不修改 global `ExecutionPolicy`。
- 不创建或修改 Windows 用户账号。

## 可选开关

### 限制主控机 IP

只允许 UECM 主控机访问 WinRM：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -AllowedRemoteAddress 192.168.10.20
```

可传多个地址：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -AllowedRemoteAddress 192.168.10.20,192.168.10.21
```

### Workgroup local admin

如果不是域环境，而是用目标机本地 Administrator 账号做远程管理，可能需要：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -EnableLocalAccountRemoteAdmin
```

这个开关会设置：

```text
HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System\LocalAccountTokenFilterPolicy = 1
```

含义：允许本地 Administrators 组账号在远程管理场景获得完整 admin token。域账号环境通常不需要这个开关。

### 跳过 network profile 修改

如果现场网络策略不允许脚本把 `Public` 改成 `Private`：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -NetworkCategory Skip
```

## 部署方式

### UECM 自动 bootstrap

如果目标机的 `ADMIN$` / `RPC` / `Service Control Manager` 管理入口可用，UECM 可以从「机器详情」页直接运行 first-contact bootstrap：

1. 在 UECM 中保存一个 `WinRM` 类型的管理员凭据。
2. 选择离线或未纳管机器。
3. 在 first-contact bootstrap 区域选择该管理员凭据。
4. 如果现场是 workgroup + 目标机本地 Administrator 账号，勾选 `Workgroup local admin`。
5. 点击「运行 bootstrap」。
6. UECM 会通过 bundled `PsExec64.exe` 上传并执行 `enable-winrm.ps1`。
7. UECM 会把目标 host/IP 追加到主控机 `WSMan:\localhost\Client\TrustedHosts`，确保 workgroup/IP 场景的后续 `WinRM` 调用可用。
8. 成功后机器状态应变为 online，后续 refresh/UE/GPU/INI/DDC/PSO 均走 `WinRM`。

如果自动 bootstrap 失败，UECM 会显示同一份本地脚本内容，作为 USB / 镜像 / GPO fallback。

### USB

1. 在构建机或主控机上生成 package。
2. 把 `UECM-WinRM-Bootstrap` 目录复制到 U 盘。
3. 在每台目标机上用 Administrator 打开 PowerShell。
4. 运行 `UECM-Bootstrap-WinRM.ps1`。
5. 回到 UECM 主控机执行 scan/refresh。

### 共享盘

如果目标机能访问共享盘，但主控机不能远程管理目标机，可以把 package 放到共享盘，由现场人员在目标机本地执行一次。

### Golden image

如果机器通过统一镜像部署，把 `enable-winrm.ps1` 合入镜像初始化流程。这样新机器开机后天然可被 UECM scan/refresh。

### GPO

域环境推荐用 `GPO` startup script 下发 `enable-winrm.ps1`。这比逐台 U 盘执行更适合批量 render node。

### Intune / SCCM / RMM

已有企业设备管理系统时，把 `enable-winrm.ps1` 作为一次性 remediation script 或 package 下发。

## 验证

目标机本地验证：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\UECM-Bootstrap-WinRM.ps1 -CheckOnly
```

主控机验证：

```powershell
Test-WSMan 192.168.10.173
```

如果是 workgroup/IP 访问，主控机还需要配置 `TrustedHosts`：

```powershell
Set-Item WSMan:\localhost\Client\TrustedHosts -Value "192.168.10.173" -Force
Test-WSMan 192.168.10.173
```

UECM 验证：

- `Machines` 页面重新 `Scan`。
- 选择目标机点击 `Refresh`。
- 预期能显示 UE install 和 GPU 信息。

## 失败判断

如果执行后主控机仍然无法 `Test-WSMan <target>`：

- 检查目标机和主控机是否在同一网络可达。
- 检查目标机 firewall 是否被第三方安全软件覆盖。
- 检查是否用了错误的 local/domain credential。
- workgroup local admin 场景检查是否已使用 `-EnableLocalAccountRemoteAdmin`。
- 如果启用了 `-AllowedRemoteAddress`，确认主控机 IP 没写错。
