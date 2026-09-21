# Build Directory

## Velo 品牌资源

Logo 源文件为 `frontend/src/assets/logo.png`，界面和项目 README 直接引用它。
`build/appicon.png` 是同一图片的打包副本；`build/windows/icon.ico` 由 Wails
生成，用于 Windows 可执行文件以及 NSIS 安装/卸载程序。托盘从可执行文件提取图标。

替换源文件后，在项目根目录执行以下命令，同步 PNG 并重新生成 ICO：

```powershell
Copy-Item frontend/src/assets/logo.png build/appicon.png
Remove-Item build/windows/icon.ico
wails build -platform windows/amd64
```

请一并提交源 PNG、打包 PNG 和生成的 ICO，避免后续构建使用旧图标。

The build directory is used to house all the build files and assets for your application. 

The structure is:

* bin - Output directory
* darwin - macOS specific files
* windows - Windows specific files

## Mac

The `darwin` directory holds files specific to Mac builds.
These may be customised and used as part of the build. To return these files to the default state, simply delete them
and
build with `wails build`.

The directory contains the following files:

- `Info.plist` - the main plist file used for Mac builds. It is used when building using `wails build`.
- `Info.dev.plist` - same as the main plist file but used when building using `wails dev`.

## Windows

The `windows` directory contains the manifest and rc files used when building with `wails build`.
These may be customised for your application. To return these files to the default state, simply delete them and
build with `wails build`.

- `icon.ico` - The icon used for the application. This is used when building using `wails build`. If you wish to
  use a different icon, simply replace this file with your own. If it is missing, a new `icon.ico` file
  will be created using the `appicon.png` file in the build directory.
- `installer/*` - The files used to create the Windows installer. These are used when building using `wails build`.
- `info.json` - Application details used for Windows builds. The data here will be used by the Windows installer,
  as well as the application itself (right click the exe -> properties -> details)
- `wails.exe.manifest` - The main application manifest file.
