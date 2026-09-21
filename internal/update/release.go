package update

import "strings"

// Asset 是 release 中的一个下载文件。
type Asset struct {
	Name string `json:"name"`
	URL  string `json:"browser_download_url"`
	Size int64  `json:"size"`
}
type Release struct {
	TagName     string  `json:"tag_name"`
	Name        string  `json:"name"`
	Body        string  `json:"body"`
	Draft       bool    `json:"draft"`
	Prerelease  bool    `json:"prerelease"`
	PublishedAt string  `json:"published_at"`
	Assets      []Asset `json:"assets"`
}

const (
	// PortableName 是应用随包发布的独立程序，可原地替换当前可执行文件。
	PortableName = "velo-launcher.exe"
	// InstallerSuffix 标记 NSIS 安装包，用于无法写入的安装目录。
	InstallerSuffix = "-installer.exe"
	// ChecksumName 是发布方提供的 SHA-256 清单；缺失时拒绝自动安装。
	ChecksumName = "SHA256SUMS.txt"
)

// Asset 按文件名精确查找，重命名过的发布不会被误用。
func (r Release) Find(name string) (Asset, bool) {
	for _, asset := range r.Assets {
		if asset.Name == name {
			return asset, true
		}
	}
	return Asset{}, false
}

// Installer 返回首个安装包资产，发布命名规则为 <项目>-<架构>-installer.exe。
func (r Release) Installer() (Asset, bool) {
	for _, asset := range r.Assets {
		if strings.HasSuffix(asset.Name, InstallerSuffix) {
			return asset, true
		}
	}
	return Asset{}, false
}

// Info 是一次更新检查的结果，也是界面显示与安装所依据的全部数据。
type Info struct {
	Available   bool   `json:"available"`
	Current     string `json:"current"`
	Version     string `json:"version"`
	Notes       string `json:"notes"`
	PublishedAt string `json:"published_at"`
	Portable    Asset  `json:"portable"`
	Installer   Asset  `json:"installer"`
	Checksum    Asset  `json:"checksum"`
}
