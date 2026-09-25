# Windows 系统快捷入口

## 调研结论

竞品截图展示了系统配置、系统信息、设置页和管理控制台混合搜索，以及“动态生成”标识。截图无法确认其内部算法。本实现采用可维护的功能目录，加上本机能力检测，生成当前可用入口。

微软公开的调用方式：

- [启动 Windows 设置](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings-app)：通过 `ms-settings:` URI 打开指定页面；文档明确说明可用性随 Windows 版本、SKU 和硬件而变化。
- [执行控制面板项](https://learn.microsoft.com/en-us/windows/win32/shell/executing-control-panel-items)：通过 `control.exe`、CPL 或规范名称打开控制面板项。
- [MMC 命令行](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/mmc)：使用 `mmc.exe <console.msc>` 打开管理控制台。
- [SettingsPageVisibility 策略](https://learn.microsoft.com/en-us/windows/client-management/mdm/policy-csp-settings#pagevisibilitylist)：可以通过 `hide:` 和 `showonly:` 限制设置页显示。

## 已实现

目录包含 40 个入口定义，实际数量由本机能力决定。保留原有 8 个入口的 ID，已有固定项与启动历史继续有效。

- 管理工具：系统配置、系统信息、计算机管理、事件查看器、服务、磁盘管理、任务计划、组件服务、本地组策略、磁盘清理、驱动器优化、资源监视器、颜色管理。
- 设置页：显示、声音、系统颜色、激活、恢复、Windows 更新、系统版本、存储、网络状态、代理、VPN、打印机、鼠标、日期时间、默认应用、通知、登录、启动应用，以及 Windows 10 共享体验。
- 应用启动和索引刷新时重新检测；不在每次按键搜索时扫描系统。
- EXE、CPL 和 MSC 检查本机文件；MSC 同时检查 MMC 宿主，并使用带引号的完整控制台路径。未安装的可选组件不会显示。
- 设置页检查系统构建号、`ms-settings` 协议注册、HKLM/HKCU `SettingsPageVisibility`。较新页面按最低构建号筛选；共享体验保守限定到 Windows 10。
- 中文、拼音、首字母、英文名称搜索复用现有搜索索引；首页默认展示 12 项，可展开全部，支持固定与使用历史排序。
- 临时不可用的固定系统项隐藏，但保留固定记录，恢复可用后重新出现。

## 调用与边界

发现过程只读取系统信息。启动时只打开页面或工具，不自动执行清理、恢复、更新、磁盘修改等操作，也不会把搜索文字拼接为命令。设置 URI 必须与目录中的 ID、路径和来源完全匹配，且不允许附加参数。

这是“已知功能目录 + 能力筛选”，并非枚举 Windows 中所有隐藏设置页。文件存在和 URI 注册不能保证页面在所有企业策略、精简系统、权限或 SKU 下都能打开；最终权限检查仍由 Windows 完成。没有自动提权，也没有在验证中执行清理或恢复等操作。

后续扩展应为硬件专属入口增加相应能力检测，再加入目录，避免在没有 Wi-Fi、触控板等设备的机器上展示无效入口。

测试覆盖缺失组件、控制台依赖、系统版本、协议缺失、页面策略、启动参数校验、固定项暂时失效与恢复，以及本机只读发现。
