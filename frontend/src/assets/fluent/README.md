# Fluent UI System Icons

Fluent UI System Icons 是 Velo 的主要界面图标库。新增系统入口和通用操作图标时，优先沿用本库，不混用其他图标库、手绘路径或 Unicode 字形。第三方应用自身的品牌图标与 Velo Logo 不受此限制。

- 上游：https://github.com/microsoft/fluentui-system-icons
- 固定来源提交：`8512d0121f6abd6c8c40f0bc4eb502ccd66ce6e7`
- 原始路径：`assets/<Title Case Name>/SVG/ic_fluent_<snake_case_name>_<size>_<variant>.svg`
- 许可：MIT，完整版权声明见同目录 `LICENSE.txt`，同时打包到应用的“设置 → 关于 → 图标与开源许可”。

此目录只收录项目使用的官方 SVG，保留原始文件名与内容；不引入整套包，运行时不依赖 CDN。默认采用 Regular，Filled 仅用于已固定等选中状态。系统入口优先使用 24px 版本，通用控件使用 20px、紧凑按钮使用 16px；Window Console 上游提供 20px 版本。

图标统一在 `src/icons.ts` 注册语义名称，由 `UiIcon.vue` 渲染。CSS mask 使用 SVG 原始轮廓并继承 `currentColor`，不要给图标写死颜色或修改官方线宽。浅色、深色及悬浮状态随主题切换。尺寸通过容器 CSS 设置。

新增图标时，从上述固定提交下载相应文件，在 `icons.ts` 中显式导入；更新来源版本时同步校验与保留许可证。图标仅负责装饰，按钮的可访问名称放在父级按钮的文字或 `aria-label` 上。
