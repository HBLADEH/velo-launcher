# RS-02 固定资源

`logo.png` 原样提取自 `build/windows/icon.ico` 内现成的 128 × 128 PNG 图层，
不重绘或重采样。顶部显示 36px，在 200% 缩放时为 72px；使用现有小尺寸资源，
避免为此解码 1254 × 1254 原图（RGBA 为 6,290,064 字节）。128px 版本的 RGBA
为 65,536 字节；这是静态资源尺寸，不是运行内存测量。`fluent/` 中的 SVG 原样复制自
`frontend/src/assets/fluent/`，来自 Microsoft Fluent UI System Icons，许可为 MIT，
完整文本保存在 `fluent/LICENSE.txt`。用于界面控件及固定应用/系统快捷的语义图标，
不表示第三方应用的官方品牌图标。

所有资源由 Slint 编译器嵌入程序；运行时不读取用户图标缓存，不请求网络、不启动 HTTP
服务。缺失图标样例使用名称的首个字符。系统与应用均使用同一组 32px 显示尺寸。
