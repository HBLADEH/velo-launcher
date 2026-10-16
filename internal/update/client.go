package update

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"velo-launcher/internal/version"
)

// Client 读取 GitHub release，API 限流时改读公开发布页。
// BaseURL 与 WebURL 可替换，测试与镜像都不需要联网访问 GitHub。
type Client struct {
	Repository string
	BaseURL    string
	WebURL     string
	Token      string
	HTTP       *http.Client

	mu         sync.Mutex
	apiRetryAt time.Time
}

func NewClient() *Client {
	return &Client{
		Repository: version.Repository,
		BaseURL:    "https://api.github.com",
		WebURL:     "https://github.com",
		HTTP:       newHTTPClient(),
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
	if best.fromFeed {
		best.Assets, err = c.publicAssets(ctx, best.TagName)
		if err != nil {
			return info, c.publicCheckError(err)
		}
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
	c.mu.Lock()
	limited := time.Now().Before(c.apiRetryAt)
	c.mu.Unlock()
	if limited {
		return c.limitedReleases(ctx)
	}
	endpoint := fmt.Sprintf("%s/repos/%s/releases?per_page=20", c.baseURL(), c.Repository)
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	c.headers(request)
	response, err := c.http().Do(request)
	if err != nil {
		return nil, networkError("无法连接更新服务器", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		message := apiMessage(response.Body)
		if response.StatusCode == http.StatusTooManyRequests ||
			(response.StatusCode == http.StatusForbidden &&
				(response.Header.Get("X-RateLimit-Remaining") == "0" || strings.Contains(strings.ToLower(message), "rate limit"))) {
			c.deferAPI(response.Header)
			response.Body.Close()
			return c.limitedReleases(ctx)
		}
		return nil, fmt.Errorf("更新服务器返回 %s：%s", response.Status, message)
	}
	var releases []Release
	if err := json.NewDecoder(io.LimitReader(response.Body, 4<<20)).Decode(&releases); err != nil {
		return nil, fmt.Errorf("无法解析更新信息：%w", err)
	}
	return releases, nil
}

// 限流期间直接读公开发布页，避免手动检查和安装前复查反复请求 API。
func (c *Client) deferAPI(headers http.Header) {
	now := time.Now()
	retryAt := now.Add(5 * time.Minute)
	if reset, err := strconv.ParseInt(headers.Get("X-RateLimit-Reset"), 10, 64); err == nil && reset > now.Unix() {
		retryAt = time.Unix(reset, 0)
	}
	if delay, err := strconv.ParseInt(headers.Get("Retry-After"), 10, 64); err == nil && delay > 0 && delay <= 86400 {
		retryAt = now.Add(time.Duration(delay) * time.Second)
	} else if date, err := http.ParseTime(headers.Get("Retry-After")); err == nil && date.After(now) {
		retryAt = date
	}
	c.mu.Lock()
	if retryAt.After(c.apiRetryAt) {
		c.apiRetryAt = retryAt
	}
	c.mu.Unlock()
}

func (c *Client) limitedReleases(ctx context.Context) ([]Release, error) {
	releases, err := c.publicReleases(ctx)
	if err != nil {
		return nil, c.publicCheckError(err)
	}
	return releases, nil
}

func (c *Client) publicCheckError(err error) error {
	return fmt.Errorf("GitHub API 请求已限流，发布页备用检查失败；请稍后重试、切换代理节点，或前往 %s/%s/releases 手动下载：%w", c.webURL(), c.Repository, err)
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
		return "", networkError("下载更新文件失败", err)
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
		return nil, networkError("无法读取校验文件", err)
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
		return newHTTPClient()
	}
	return c.HTTP
}

func newHTTPClient() *http.Client {
	transport := http.DefaultTransport.(*http.Transport).Clone()
	transport.Proxy = updateProxy
	return &http.Client{Transport: transport, Timeout: 60 * time.Second}
}

func networkError(action string, err error) error {
	return fmt.Errorf("%s，请检查网络及代理设置；也可前往 GitHub 发布页手动下载：%w", action, err)
}

func (c *Client) headers(request *http.Request) {
	request.Header.Set("Accept", "application/vnd.github+json")
	request.Header.Set("User-Agent", "Velo-Launcher/"+version.Number)
	// API 凭据只发给配置的 API 服务，不附带到公开发布页或下载主机。
	api, err := url.Parse(c.baseURL())
	if c.Token != "" && err == nil && request.URL.Scheme == api.Scheme && strings.EqualFold(request.URL.Host, api.Host) {
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
