package update

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"velo-launcher/internal/version"
)

// Client 读取 GitHub release。BaseURL 可替换，测试与镜像都不需要联网访问 GitHub。
type Client struct {
	Repository string
	BaseURL    string
	Token      string
	HTTP       *http.Client
}

func NewClient() *Client {
	return &Client{
		Repository: version.Repository,
		BaseURL:    "https://api.github.com",
		HTTP:       &http.Client{Timeout: 60 * time.Second},
	}
}

// Check 返回比 current 更新的最高版本。项目至今只发布 prerelease，
// 因此使用列表接口而不是 /releases/latest（后者会忽略预发布版本）。
func (c *Client) Check(ctx context.Context, current Version) (Info, error) {
	releases, err := c.releases(ctx)
	if err != nil {
		return Info{}, err
	}
	var (
		best   Release
		latest Version
		found  bool
	)
	for _, release := range releases {
		if release.Draft {
			continue
		}
		candidate, err := ParseVersion(release.TagName)
		if err != nil {
			continue
		}
		if candidate.Compare(current) <= 0 {
			continue
		}
		if !found || candidate.Newer(latest) {
			best, latest, found = release, candidate, true
		}
	}
	info := Info{Current: current.String()}
	if !found {
		return info, nil
	}
	portable, hasPortable := best.Find(PortableName)
	installer, hasInstaller := best.Installer()
	if !hasPortable && !hasInstaller {
		return info, fmt.Errorf("v%s 没有可下载的更新文件，请前往发布页手动下载", latest.String())
	}
	info.Available = true
	info.Version = latest.String()
	info.Notes = strings.TrimSpace(best.Body)
	info.PublishedAt = best.PublishedAt
	info.Portable, info.Installer = portable, installer
	info.Checksum, _ = best.Find(ChecksumName)
	return info, nil
}

func (c *Client) releases(ctx context.Context) ([]Release, error) {
	endpoint := fmt.Sprintf("%s/repos/%s/releases?per_page=20", c.baseURL(), c.Repository)
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	c.headers(request)
	response, err := c.http().Do(request)
	if err != nil {
		return nil, fmt.Errorf("无法连接更新服务器：%w", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("更新服务器返回 %s：%s", response.Status, apiMessage(response.Body))
	}
	var releases []Release
	if err := json.NewDecoder(io.LimitReader(response.Body, 4<<20)).Decode(&releases); err != nil {
		return nil, fmt.Errorf("无法解析更新信息：%w", err)
	}
	return releases, nil
}

// Download 把 url 写入 dest 并返回 SHA-256。先写临时文件，成功后原子替换，
// 中途失败不会留下半个可执行文件。
func (c *Client) Download(ctx context.Context, url, dest string, onProgress func(done, total int64)) (string, error) {
	if err := os.MkdirAll(filepath.Dir(dest), 0700); err != nil {
		return "", err
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return "", err
	}
	c.headers(request)
	response, err := c.http().Do(request)
	if err != nil {
		return "", fmt.Errorf("下载失败：%w", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return "", fmt.Errorf("下载失败，服务器返回 %s", response.Status)
	}
	file, err := os.CreateTemp(filepath.Dir(dest), ".update-*.tmp")
	if err != nil {
		return "", err
	}
	name := file.Name()
	defer os.Remove(name)
	sum := sha256.New()
	writer := io.MultiWriter(file, sum)
	var done int64
	buffer := make([]byte, 64<<10)
	for {
		read, readErr := response.Body.Read(buffer)
		if read > 0 {
			if _, err := writer.Write(buffer[:read]); err != nil {
				file.Close()
				return "", err
			}
			done += int64(read)
			if onProgress != nil {
				onProgress(done, response.ContentLength)
			}
		}
		if readErr == io.EOF {
			break
		}
		if readErr != nil {
			file.Close()
			return "", fmt.Errorf("下载中断：%w", readErr)
		}
	}
	if err := file.Close(); err != nil {
		return "", err
	}
	if err := os.Rename(name, dest); err != nil {
		return "", err
	}
	return hex.EncodeToString(sum.Sum(nil)), nil
}

// Checksums 解析 SHA256SUMS.txt：<64 位十六进制>  <文件名>。
func (c *Client) Checksums(ctx context.Context, url string) (map[string]string, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	c.headers(request)
	response, err := c.http().Do(request)
	if err != nil {
		return nil, fmt.Errorf("无法读取校验文件：%w", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("校验文件不可用，服务器返回 %s", response.Status)
	}
	body, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		return nil, err
	}
	return ParseChecksums(string(body)), nil
}

// ParseChecksums 忽略注释与空行，文件名保留原样（不含路径前缀）。
func ParseChecksums(text string) map[string]string {
	sums := map[string]string{}
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) < 2 {
			continue
		}
		name := strings.TrimPrefix(fields[1], "*")
		name = strings.TrimPrefix(name, "./")
		sums[filepath.Base(name)] = strings.ToLower(fields[0])
	}
	return sums
}

func (c *Client) baseURL() string {
	if c.BaseURL == "" {
		return "https://api.github.com"
	}
	return strings.TrimSuffix(c.BaseURL, "/")
}

func (c *Client) http() *http.Client {
	if c.HTTP == nil {
		c.HTTP = &http.Client{Timeout: 60 * time.Second}
	}
	return c.HTTP
}

func (c *Client) headers(request *http.Request) {
	request.Header.Set("Accept", "application/vnd.github+json")
	request.Header.Set("User-Agent", "Velo-Launcher/"+version.Number)
	if c.Token != "" {
		request.Header.Set("Authorization", "Bearer "+c.Token)
	}
}

// apiMessage 提取 GitHub 错误说明，便于显示限流或被删除的原因。
func apiMessage(body io.Reader) string {
	var payload struct {
		Message string `json:"message"`
	}
	data, err := io.ReadAll(io.LimitReader(body, 1<<16))
	if err != nil {
		return "未知错误"
	}
	if err := json.Unmarshal(data, &payload); err != nil || payload.Message == "" {
		return "未知错误"
	}
	return payload.Message
}
