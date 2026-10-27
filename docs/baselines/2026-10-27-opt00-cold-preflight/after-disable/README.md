# 用户授权后的冷启动准备

公开说明：本页关联的公开结果已脱敏；原始日志与源码快照仅本地保留，见[公开范围与校验口径](../../README.md)。

用户已明确授权“允许临时停用，测完恢复”。本阶段已临时停用实际登录入口，并保留当前安装版实例；没有自动重启电脑，也没有修改应用配置。下一次冷启动仍需用户自行保存工作并手动重启。

最终状态见[重启准备检查](ready-for-reboot.json)：系统注册表服务确认 `Run\Velo` 不存在，`Win32_StartupCommand` 中其余 Velo 入口为 0，PID 30492 的安装版仍在运行。用户输入文件指纹未被准备过程改写。冻结程序与测试输入已重新固化，准备时刻为北京时间 2026-10-27 21:56:06；下一次系统启动必须晚于该时刻。

## 实际系统入口与本地视图

最初的本地 HKCU API 删除后，本地视图显示不存在，但系统 `StdRegProv` 与登录启动枚举仍返回原值。`disable-result.json` / `status-after-disable.json` 是该本地视图的中间结果，不能作为实际停用证明。

管理脚本已改为通过系统 `StdRegProv`，在当前用户 SID 下操作固定的 `Run\Velo` 值，仍要求原始字符串、值类型及备份身份准确匹配。采用 `EnumValues` 判断名称是否存在，避免本机 `GetStringValue` 对不存在的值返回 1 时误报写入失败。最终依据为[系统删除结果](disable-system-provider.json)和[系统状态](status-system-provider.json)。

已实际恢复原始字符串并核验，再次临时停用；见[恢复往返检查](restore-roundtrip-check.json)。恢复时也会还原此前本地 API 视图中的删除。仅处理同一个已授权的值，不修改 StartupApproved 或其他启动项。

## 下次重启后采集

重启后先不要打开安装版或测试版 Velo，回到对话回复“已重启”。确认新启动条件后执行一次首次启动采集：

```powershell
./scripts/collect-gui-startups.ps1 -Cold -Count 1 `
  -WorkDirectory .reasonix/opt00-latency-20261027-1240 `
  -OutputDirectory docs/baselines/<本次全新冷启动目录> `
  -StartupBackupPath .reasonix/opt00-latency-20261027-1240/startup-backup-20261027.json
```

须先创建全新的输出目录。布防后由用户从 `launch/Velo timing.lnk` 启动一次；保持窗口前台，不重复打开，不点击搜索框补救初始焦点失败，也不按 Enter 启动候选。无法直接输入如实保留为失败。

采集器完成 / 报错后都通过嵌套 `finally` 调用恢复步骤。恢复回归模拟了“采集失败且普通清理也失败”，仍确认恢复步骤执行，见回归日志（本地原件：`cleanup-regression.txt`，不入库）；测试没有操作真实注册表。真实管理脚本的原值恢复已单独完成往返验证。源码快照见系统启动管理（本地原件：`manage-latency-startup-system-source.ps1.txt`，不入库）和冷启动采集（本地原件：`collect-gui-startups-source.ps1.txt`，不入库）。

本次开始准备时，安装版 PID 30492 正在运行，因此当前私有采集清单已记录其程序路径及 `--background` 参数。冷启动观察结束后会恢复这个后台运行状态；这与此前阶段 `Restore=null` 的历史清单分开记录。

每次冷启动尝试结束后即恢复原启动项。下一次独立重启采集前，需再次临时停用并核验，已有用户授权无需重复询问。每次系统启动只允许一次冷尝试；同次重开均属暖启动。

如果在采集前取消或暂缓，可立即恢复原值：

```powershell
./scripts/manage-latency-startup.ps1 -Action Restore `
  -BackupPath .reasonix/opt00-latency-20261027-1240/startup-backup-20261027.json
```

私有工作目录的 `cold-cycle.json` 当前保存 `RestoreRequired=true` / `State=AwaitingNewBoot`，供中断后恢复处理。此次准备尚未产生冷启动计时样本；先前 63 份计时证据保持不变。
