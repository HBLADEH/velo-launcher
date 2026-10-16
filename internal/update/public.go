package update

import (
	"context"
	"encoding/xml"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"

	"golang.org/x/net/html"

	"velo-launcher/internal/version"
)

// Atom 包含预发布版本；/releases/latest 会忽略这些版本，不能用于限流兜底。
func (c *Client) publicReleases(ctx context.Context) ([]Release, error) {
	response, err := c.publicResponse(ctx, c.webURL()+"/"+c.Repository+"/releases.atom", "application/atom+xml")
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	var feed struct {
		XMLName xml.Name `xml:"http://www.w3.org/2005/Atom feed"`
		Entries []struct {
			Title     string `xml:"title"`
			Content   string `xml:"content"`
			Published string `xml:"published"`
			Updated   string `xml:"updated"`
			Links     []struct {
				Rel  string `xml:"rel,attr"`
				Href string `xml:"href,attr"`
			} `xml:"link"`
		} `xml:"entry"`
	}
	if err := xml.NewDecoder(io.LimitReader(response.Body, 4<<20)).Decode(&feed); err != nil {
		return nil, fmt.Errorf("无法解析公开发布信息：%w", err)
	}
	base, err := url.Parse(c.webURL())
	if err != nil {
		return nil, err
	}
	prefix := strings.TrimSuffix(base.Path, "/") + "/" + c.Repository + "/releases/tag/"
	releases := make([]Release, 0, len(feed.Entries))
	for _, entry := range feed.Entries {
		for _, link := range entry.Links {
			if link.Rel != "alternate" {
				continue
			}
			tag, ok := publicPathSuffix(base, link.Href, prefix)
			if !ok {
				continue
			}
			published := entry.Published
			if published == "" {
				published = entry.Updated
			}
			releases = append(releases, Release{
				TagName: tag, Name: entry.Title, Body: publicNotes(entry.Content),
				PublishedAt: published, fromFeed: true,
			})
			break
		}
	}
	if len(feed.Entries) > 0 && len(releases) == 0 {
		return nil, fmt.Errorf("公开发布信息中没有可识别的版本链接")
	}
	return releases, nil
}

// 仅从实际资产列表提取下载地址，不猜测文件是否存在；校验清单仍须由发布方提供。
func (c *Client) publicAssets(ctx context.Context, tag string) ([]Asset, error) {
	endpoint := c.webURL() + "/" + c.Repository + "/releases/expanded_assets/" + url.PathEscape(tag)
	response, err := c.publicResponse(ctx, endpoint, "text/html")
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	base, err := url.Parse(c.webURL())
	if err != nil {
		return nil, err
	}
	prefix := strings.TrimSuffix(base.Path, "/") + "/" + c.Repository + "/releases/download/" + tag + "/"
	var assets []Asset
	tokens := html.NewTokenizer(io.LimitReader(response.Body, 4<<20))
	for {
		switch tokens.Next() {
		case html.ErrorToken:
			if err := tokens.Err(); err != io.EOF {
				return nil, fmt.Errorf("无法解析发布文件列表：%w", err)
			}
			return assets, nil
		case html.StartTagToken:
			token := tokens.Token()
			if token.Data != "a" {
				continue
			}
			for _, attr := range token.Attr {
				if attr.Key != "href" {
					continue
				}
				name, ok := publicPathSuffix(base, attr.Val, prefix)
				if !ok {
					continue
				}
				target, _ := base.Parse(attr.Val)
				assets = append(assets, Asset{Name: name, URL: target.String()})
			}
		}
	}
}

// 公开端点不接收 Token；备用检查复用下载的 HTTP 客户端与系统代理设置。
func (c *Client) publicResponse(ctx context.Context, endpoint, accept string) (*http.Response, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, err
	}
	request.Header.Set("User-Agent", "Velo-Launcher/"+version.Number)
	request.Header.Set("Accept", accept)
	response, err := c.http().Do(request)
	if err != nil {
		return nil, networkError("无法连接 GitHub 公开发布页", err)
	}
	if response.StatusCode != http.StatusOK {
		response.Body.Close()
		return nil, fmt.Errorf("公开发布页返回 %s", response.Status)
	}
	return response, nil
}

// 限定同一主机、仓库和 tag，排除源码归档、其他仓库及外部下载链接。
func publicPathSuffix(base *url.URL, href, prefix string) (string, bool) {
	target, err := base.Parse(href)
	if err != nil || target.Scheme != base.Scheme || !strings.EqualFold(target.Host, base.Host) ||
		target.User != nil || target.RawQuery != "" || target.Fragment != "" {
		return "", false
	}
	suffix, ok := strings.CutPrefix(target.Path, prefix)
	return suffix, ok && suffix != "" && suffix != "." && suffix != ".." && !strings.ContainsAny(suffix, `/\`)
}

// Atom 的说明是 HTML，转成纯文本交给现有的 <pre> 展示，避免显示标签。
func publicNotes(content string) string {
	var text strings.Builder
	tokens := html.NewTokenizer(strings.NewReader(content))
	for {
		switch tokens.Next() {
		case html.ErrorToken:
			return strings.TrimSpace(text.String())
		case html.TextToken:
			text.Write(tokens.Text())
		case html.StartTagToken, html.EndTagToken:
			token := tokens.Token()
			switch token.Data {
			case "br", "p", "li", "h1", "h2", "h3", "h4", "h5", "h6", "pre", "tr":
				if text.Len() > 0 && !strings.HasSuffix(text.String(), "\n") {
					text.WriteByte('\n')
				}
			case "td", "th":
				text.WriteByte('\t')
			}
		}
	}
}

func (c *Client) webURL() string {
	if c.WebURL == "" {
		return "https://github.com"
	}
	return strings.TrimSuffix(c.WebURL, "/")
}
