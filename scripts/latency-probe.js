// Injected into the latencyprobe build only. No query text is collected.
(() => {
  let attached = false
  let ready = false
  let checking = false
  let lastState = ''
  const bridge = () => window.go?.main?.App?.LatencyProbeMark
  const state = () => {
    const input = document.querySelector('input[role="combobox"]')
    const bounds = input?.getBoundingClientRect()
    const style = input && getComputedStyle(input)
    return {
      stage: 'ready',
      documentFocused: document.hasFocus(),
      inputFocused: Boolean(input && document.activeElement === input),
      writable: Boolean(input && !input.disabled && !input.readOnly),
      displayed: Boolean(input?.isConnected && bounds.width > 0 && bounds.height > 0 && style.visibility === 'visible' && style.display !== 'none'),
      browserTimeMS: performance.now(),
    }
  }
  const inspect = () => {
    checking = false
    const mark = bridge()
    if (!mark) return
    if (!attached) {
      attached = true
      void mark({ stage: 'attach' }).catch(() => { attached = false })
    }
    const snapshot = state()
    const next = snapshot.documentFocused && snapshot.inputFocused && snapshot.writable && snapshot.displayed
    const signature = JSON.stringify([snapshot.documentFocused, snapshot.inputFocused, snapshot.writable, snapshot.displayed])
    if (next && !ready) {
      ready = true
      void mark(snapshot).catch(() => { ready = false })
    } else if (!next && (ready || signature !== lastState)) {
      ready = false
      void mark({ ...snapshot, stage: 'not-ready' }).catch(() => { lastState = '' })
    }
    lastState = signature
  }
  const schedule = () => {
    if (!checking) { checking = true; queueMicrotask(inspect) }
  }
  for (const name of ['focus', 'blur', 'focusin', 'focusout', 'DOMContentLoaded']) {
    window.addEventListener(name, schedule, true)
  }
  new MutationObserver(schedule).observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ['disabled', 'readonly', 'style', 'class'] })
  setInterval(schedule, 25)
  schedule()
})()
