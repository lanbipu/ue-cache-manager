========================================
 UECM WinRM Bootstrap 一键部署工具
========================================

这是什么？
--------
让一台全新装好的 Windows 渲染节点，"一次双击" 就完成 UECM
（UE Cache Manager）远程接管所需的全部系统配置。跑完之后，
operator 主机就可以通过局域网远程管理这台机器的 UE 缓存、
环境变量、INI 配置、SMB 共享等。

使用步骤
--------
0.（可选但强烈建议）想让 UECM 之后能远程接管这台机器，先用记事本
   打开 UECM-Bootstrap.cmd，找到顶部 UECM_LOCAL_ADMIN_PASSWORD=
   这一行，在等号后填一个强密码（账号名默认 uecm-svc，可改）。
   不填 = 只开 WinRM/SMB/WMI，不创建账号（之后没有可远程登录的账号）。
1. 把整个文件夹（U 盘 / 网盘 / 共享文件夹皆可）拷到目标机器。
2. 双击 UECM-Bootstrap.cmd。
3. 系统弹出 UAC 提示（"是否允许此应用对你的设备进行更改"），
   点击 "是"。
4. 看到窗口出现醒目的 "[ OK ] UECM bootstrap SUCCEEDED" 提示后即可关闭窗口。

整个过程通常 30 秒以内。

跑完之后机器会发生什么变化？
--------------------------
- WinRM 服务启动并设为开机自启，监听 5985 端口
- Windows 防火墙放行 WinRM 远程管理规则
- 网络配置从 Public 切换为 Private
- 启用本地管理员账号远程认证（workgroup 必备）
- 文件共享服务（LanmanServer）启动 + 防火墙放行 TCP 445
- WMI 服务确认运行（用于 operator 远程查询 GPU / UE 版本）
- PowerShell 远程执行策略调为 RemoteSigned
- 启用 Windows 长路径支持（UE 工程必备）
- 电源计划切换为 "高性能"
- 若填了密码：创建（或重置）一个本地管理员账号（账号名见 .cmd 里的
  UECM_LOCAL_ADMIN，默认 uecm-svc）并加入 Administrators 组——这
  就是 UECM 远程登录用的那把"钥匙"

跑完之后不会改的：
- 不会装任何软件
- 不会动你现有的用户账号 / 密码（只有你主动在 .cmd 里填了密码时，才会
  创建 / 重置 .cmd 里指定的那一个本地管理员账号，别的账号一概不碰）
- 不会重启系统
- 不会动 Defender / 反病毒 / EDR
- 不会改 RDP / 域账号 / 已有共享

operator 端首次连接前还需要做一步（重要）
------------------------------------------
当 operator 机器和目标机不在同一个域，而是用 IP 或 workgroup
连接时，operator 自己的 PowerShell 客户端默认不信任目标 IP，
会拒绝带凭据的远程调用。你需要在 operator 机器上以管理员身份
执行一次：

    Set-Item WSMan:\localhost\Client\TrustedHosts `
        -Value '192.168.10.50,192.168.10.51' -Force

把 192.168.10.50,192.168.10.51 替换成你所有目标机的 IP（逗号分隔），
或者直接用通配符 '192.168.10.*' 覆盖整个网段。

只需要做一次。之后 operator 上的 UECM CLI（uecm-cli machine
refresh / share create / 等）就能正常使用带凭据的 WinRM 调用。

连接时用哪个账号？就是你在 .cmd 里填的那一组（账号名见 .cmd 顶部
UECM_LOCAL_ADMIN 那行，默认 uecm-svc；密码就是你设的那个）。在
UECM 里把它存成一个凭据别名，
这台机器的所有远程操作都用它。MSA（Microsoft 账户）和不知道
密码的内置 Administrator 都没法用于 WinRM 远程认证，所以才要
专门建这个本地管理员账号。

如果不做 TrustedHosts 这一步，常见的错误信息是：
    "The WinRM client cannot process the request. Default
    authentication may be used with an IP address under the
    following conditions: ..."

故障排查
--------
[Q] 双击没反应 / UAC 直接关闭：
    右键点击 UECM-Bootstrap.cmd → "以管理员身份运行"。

[Q] 看到红色错误 "Administrator privileges are required"：
    当前 PowerShell 窗口不是管理员权限，按上一条重新走。

[Q] 看到 "Test-WSMan localhost still failed"：
    Windows Defender / 第三方 EDR 可能拦了 WinRM 配置。
    暂时关闭 EDR 重试，并联系运维确认放行规则。

[Q] 跑完之后 operator 端 uecm-cli machine refresh 报
    "WinRM client cannot process the request"：
    operator 端没加 TrustedHosts。看上面"operator 端首次连接前
    还需要做一步"那节。

[Q] 跑完之后 operator 端 uecm-cli machine refresh 还是
    "WinRM offline"：
    检查 operator 机和目标机是否在同一网段、防火墙是否限制
    了来源 IP。可用 -AllowedRemoteAddress 参数显式放开。

[Q] 想只检查不修改：
    用管理员 PowerShell 跑：
       .\UECM-Bootstrap-WinRM.ps1 -CheckOnly
    会输出当前状态 JSON，不写任何东西。

[Q] 已经跑过一次，能再跑吗？
    可以。脚本是幂等的，重复跑不会产生副作用。
