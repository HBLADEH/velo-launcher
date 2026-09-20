import {defineConfig, type Plugin} from 'vite'
import vue from '@vitejs/plugin-vue'
import {createReadStream, existsSync} from 'node:fs'
import {join} from 'node:path'

// 开发模式下前端页面由 Vite 直接提供，Wails 的 icon.Handler 不在请求链上，
// /icons/*.png 会被 SPA 回退成 index.html，候选图标因此全部失效。这里按与 Go
// 侧相同的规则读取本机图标缓存；生产构建仍由 icon.Handler 提供，不嵌入图标。
function veloIconCache(): Plugin {
  return {
    name: 'velo-icon-cache',
    apply: 'serve',
    configureServer(server) {
      const directory = join(process.env.LOCALAPPDATA ?? '', 'Velo', 'cache', 'icons')
      server.middlewares.use('/icons', (request, response, next) => {
        const name = (request.url ?? '').split('?')[0].replace(/^\//, '')
        const file = join(directory, name)
        if (!/^[a-f0-9]{32}\.png$/.test(name) || !existsSync(file)) {
          next()
          return
        }
        response.setHeader('Content-Type', 'image/png')
        response.setHeader('Cache-Control', 'no-cache')
        createReadStream(file).on('error', () => next()).pipe(response)
      })
    },
  }
}

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [vue(), veloIconCache()]
})
