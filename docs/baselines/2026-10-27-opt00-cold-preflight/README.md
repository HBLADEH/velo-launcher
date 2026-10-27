# OPT-00 重启后冷启动条件复核

公开说明：本页关联的公开结果已脱敏；原始日志与源码快照仅本地保留，见[公开范围与校验口径](../README.md)。

2026-10-27（北京时间）本次系统已在 **19:25:07** 重启，但安装版 Velo 在 **19:26:10** 以 `--background` 运行。检查时仍在运行，PID 30492。该次系统启动已不满足“此前尚未运行 Velo”的冷启动条件，**未产生冷启动计时样本**。

具体依据见[只读预检](boot-preflight.json)：

- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `Velo` 值仍指向安装版及 `--background`，StartupApproved 状态为启用。`Win32_StartupCommand` 也确认了同一入口。
- 用户 `config.json` 的 `launch_at_startup=false` 与实际注册表入口不一致，因此仅检查应用配置不足以保证下次不会自动运行。
- 冻结候选程序的字节数 / 修改时间、五份测试输入的字节数 / 修改时间均与重启前清单一致；未读取候选程序内容或测试输入内容，也未启动探针。
- 现有采集器 `Arm -Kind ColdStartup` 拒绝执行：`A Velo instance is already running.`。未将停止安装版后的重开冒充冷启动。

本次只保存了启动项备份，尚未更改注册表、应用配置或运行中的安装版。之前未预检实际登录启动项，造成此次重启无法用于冷启动测量；后续必须在重启前确认该入口不会自动运行。

## 已准备的可恢复处理

[管理脚本](../../../scripts/manage-latency-startup.ps1)默认执行只读 `Status`，源码快照见归档（本地原件：`manage-latency-startup-source.ps1.txt`，不入库）。已经解析检查并实际执行 `Backup` / `Status`，二者均确认 `RegistryChanged=false`，见[处理前状态](startup-status-before.json)。

备份保存在忽略目录：`.reasonix/opt00-latency-20261027-1240/startup-backup-20261027.json`。备份包含当前用户身份、准确的注册表键、值类型和原始字符串，不会覆盖已有备份。

只有用户明确同意临时例外后，才执行：

```powershell
./scripts/manage-latency-startup.ps1 -Action Disable `
  -BackupPath .reasonix/opt00-latency-20261027-1240/startup-backup-20261027.json
```

该操作仅暂时移除当前用户 `Run` 键中准确匹配的 `Velo` 字符串值，不停止当前实例、不修改 `config.json`、不修改其他启动项，也不自动重启。原值或用户身份与备份不符时拒绝操作。

测试结束或取消后恢复原值：

```powershell
./scripts/manage-latency-startup.ps1 -Action Restore `
  -BackupPath .reasonix/opt00-latency-20261027-1240/startup-backup-20261027.json
```

恢复时如发现该启动项已被用户改成其他值，脚本拒绝覆盖。此步骤尚未实际执行，不能宣称停用 / 恢复已验证。用户也可自行禁用该登录启动项，再由只读预检确认后安排下一次重启。

此前的[热呼出与暖启动证据](../2026-10-27-opt00-latency/README.md)及 `SHA256SUMS-phase1.txt` 保持不变。真正冷启动仍为 0 个有效样本，OPT-00 继续待验证。
