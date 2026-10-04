import { reactive } from 'vue'
import type { update } from '../wailsjs/go/models'

// Owned by App, so closing a settings panel cannot discard an active download.
export function createUpdater(api: { check: () => Promise<update.Info>; install: () => Promise<void> }) {
  const state = reactive({
    info: undefined as update.Info | undefined,
    message: '', checking: false, installing: false, progress: 0,
  })
  async function check() {
    if (state.checking || state.installing) return
    state.checking = true
    state.message = ''
    try {
      state.info = await api.check()
      state.message = state.info.available ? '' : `已是最新版本 ${state.info.current}。`
    } catch (cause) { state.message = String(cause) }
    finally { state.checking = false }
  }
  async function install() {
    if (state.installing || state.checking) return
    state.installing = true
    state.message = ''
    state.progress = 0
    try {
      await api.install()
      state.message = '正在退出并完成更新，Velo 会自动重新启动。'
    } catch (cause) {
      state.message = String(cause)
      state.installing = false
    }
  }
  return { state, check, install }
}

export type Updater = ReturnType<typeof createUpdater>
