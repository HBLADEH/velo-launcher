import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { compileScript, parse } from '@vue/compiler-sfc'
import ts from 'typescript'
import * as vue from 'vue'
import { choiceRows, navigateChoices } from '../src/navigation.ts'
import { createUpdater } from '../src/updater.ts'

// Exercise App's actual script setup, with Vue's real refs, computed values and
// watchers. Only its native bridge, browser surface and lifecycle hooks are doubles.
const source = readFileSync(new URL('../src/App.vue', import.meta.url), 'utf8')
const script = compileScript(parse(source).descriptor, { id: 'launcher-test' }).content
const compiled = ts.transpileModule(script, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const evaluate = new Function('require', 'module', 'exports', 'window', 'document',
  'localStorage', 'setTimeout', 'clearTimeout', compiled)

async function settle() {
  for (let i = 0; i < 8; i++) await vue.nextTick()
}

function deferred() {
  let resolve, reject
  const promise = new Promise((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

function clock() {
  let now = 0, nextID = 0
  const tasks = new Map()
  return {
    setTimeout(callback, delay = 0) {
      const id = ++nextID
      tasks.set(id, { callback, at: now + delay })
      return id
    },
    clearTimeout(id) { tasks.delete(id) },
    get pending() { return tasks.size },
    async advance(ms) {
      const until = now + ms
      for (let count = 0; count < 1000; count++) {
        const next = [...tasks].sort((a, b) => a[1].at - b[1].at)[0]
        if (!next || next[1].at > until) { now = until; await settle(); return }
        tasks.delete(next[0])
        now = next[1].at
        next[1].callback()
        await settle()
      }
      throw new Error('Focus retries did not settle within 1000 timer callbacks')
    },
  }
}

const matches = [0, 1, 2].map(id => ({ id: `app-${id}`, name: `Application ${id}`, source: 'Custom' }))

async function launcher(overrides = {}) {
  const timers = clock()
  const mounted = [], unmounted = [], nativeEvents = new Map(), windowEvents = new Map()
  const launchCalls = [], focusCalls = []
  let hasFocus = true, disposed = false
  const document = { documentElement: { dataset: {} }, hasFocus: () => hasFocus, activeElement: undefined }
  const input = vue.markRaw({
    focus(options) { focusCalls.push(options); document.activeElement = input },
    blur() { if (document.activeElement === input) document.activeElement = undefined },
  })
  const noop = () => {}
  const window = {
    setTimeout: timers.setTimeout, clearTimeout: timers.clearTimeout,
    matchMedia: () => ({ matches: false, addEventListener: noop, removeEventListener: noop }),
    addEventListener: (name, callback) => windowEvents.set(name, callback),
    removeEventListener: name => windowEvents.delete(name),
  }
  const bridge = new Proxy({
    GetSettings: async () => ({ theme: 'system', result_layout: 'list' }),
    GetStatus: async () => ({ visible: overrides.visible ?? true }),
    GetHome: async () => { overrides.home?.(); return { pinned: [], frequent: [], tools: [] } },
    FrontendReady: async () => { overrides.frontendReady?.() },
    Search: async () => matches,
    Launch: async (...args) => { launchCalls.push({ admin: false, args }); await overrides.launch?.(...args) },
    LaunchAsAdmin: async (...args) => { launchCalls.push({ admin: true, args }); await overrides.launch?.(...args) },
  }, { get: (target, name) => target[name] ?? (async () => {}) })
  const runtime = new Proxy({
    EventsOn(name, callback) { nativeEvents.set(name, callback); return () => nativeEvents.delete(name) },
  }, { get: (target, name) => target[name] ?? noop })
  const module = { exports: {} }
  const require = name => {
    if (name === 'vue') return { ...vue, onMounted: callback => mounted.push(callback), onUnmounted: callback => unmounted.push(callback) }
    if (name === '../wailsjs/go/main/App') return bridge
    if (name === '../wailsjs/runtime/runtime') return runtime
    if (name === './navigation') return { choiceRows, navigateChoices }
    if (name === './updater') return { createUpdater }
    if (name.endsWith('.vue') || name.endsWith('.png')) return { default: {} }
    throw new Error(`Unexpected App dependency: ${name}`)
  }
  evaluate(require, module, module.exports, window, document, { setItem: noop },
    timers.setTimeout, timers.clearTimeout)
  const scope = vue.effectScope()
  const state = scope.run(() => module.exports.default.setup({}, { expose: noop }))
  state.input.value = input
  for (const callback of mounted) await callback()
  await settle()
  focusCalls.length = 0

  return {
    state, timers, launchCalls, focusCalls, document,
    set hasFocus(value) { hasFocus = value; if (!value) document.activeElement = undefined },
    async emit(name) { nativeEvents.get(name)?.(); await settle() },
    async windowEvent(name) { windowEvents.get(name)?.(); await settle() },
    dispose() {
      if (disposed) return
      disposed = true
      unmounted.forEach(callback => callback())
      scope.stop()
    },
  }
}

test('merged search grid preserves ranking and selection across sources and layout changes', async t => {
  const app = await launcher()
  t.after(() => app.dispose())
  app.state.query.value = 'application'
  await settle()
  const ranked = Array.from({ length: 8 }, (_, index) => ({
    id: `ranked-${index}`, name: `Application ${index}`, source: index % 2 ? 'Custom' : 'System',
  }))
  app.state.results.value = ranked
  app.state.settings.value.result_layout = 'grid'
  await settle()

  assert.deepEqual(app.state.choices.value.map(item => item.id), ranked.map(item => item.id))
  assert.equal(app.state.activeID.value, 'app-ranked-0', 'the top ranked system tool stays selected first')
  app.state.selected.value = navigateChoices(app.state.navigationRows.value, 1, 'ArrowDown')
  assert.equal(app.state.activeID.value, 'app-ranked-7', 'the next row continues across application sources')
  app.state.selected.value = navigateChoices(app.state.navigationRows.value, 7, 'ArrowLeft')
  assert.equal(app.state.activeID.value, 'app-ranked-6')

  app.state.settings.value.result_layout = 'list'
  await settle()
  assert.equal(app.state.activeID.value, 'app-ranked-6', 'switching layouts preserves the selected item')
  app.state.settings.value.result_layout = 'grid'
  await settle()
  await app.state.launch(app.state.selected.value)
  assert.deepEqual(app.launchCalls, [{ admin: false, args: ['ranked-6', 'application'] }])
})

for (const admin of [false, true]) {
  test(`${admin ? 'administrator' : 'normal'} launch clears the search immediately after dispatch`, async t => {
    const pending = deferred()
    const app = await launcher({ launch: () => pending.promise })
    t.after(() => app.dispose())
    app.state.query.value = 'application'
    await settle()
    app.state.selected.value = 2
    const task = app.state.launch(2, admin)
    assert.deepEqual(app.launchCalls, [{ admin, args: ['app-2', 'application'] }])
    assert.equal(app.state.query.value, '')
    assert.equal(app.state.selected.value, 0)
    assert.deepEqual(app.state.results.value, [])
    assert.equal(app.state.launching.value, true)
    await app.emit('launcher:hidden')
    await app.emit('launcher:shown')
    assert.equal(app.state.isHome.value, true)
    pending.resolve()
    await task
    await settle()
    assert.deepEqual(app.launchCalls, [{ admin, args: ['app-2', 'application'] }])
    assert.equal(app.state.query.value, '')
    assert.equal(app.state.selected.value, 0)
    assert.deepEqual(app.state.results.value, [])
    assert.equal(app.state.launching.value, false)
    await app.emit('launcher:shown')
    assert.equal(app.state.isHome.value, true)
  })

  test(`${admin ? 'administrator' : 'normal'} launch failure reports the error without restoring the old search`, async t => {
    const pending = deferred()
    const app = await launcher({ launch: () => pending.promise })
    t.after(() => app.dispose())
    app.state.query.value = 'application'
    await settle()
    app.state.selected.value = 2
    const task = app.state.launch(2, admin)
    assert.equal(app.state.query.value, '')
    await settle()
    pending.reject(new Error('Launch failed'))
    await task
    await settle()
    assert.deepEqual(app.launchCalls, [{ admin, args: ['app-2', 'application'] }])
    assert.equal(app.state.query.value, '')
    assert.equal(app.state.selected.value, 0)
    assert.deepEqual(app.state.results.value, [])
    assert.match(app.state.error.value, /Launch failed/)
    assert.equal(app.state.launching.value, false)
  })

  for (const fails of [false, true]) {
    test(`${admin ? 'administrator' : 'normal'} launch ${fails ? 'failure' : 'completion'} preserves a newer search`, async t => {
      const pending = deferred()
      const app = await launcher({ launch: () => pending.promise })
      t.after(() => app.dispose())
      app.state.query.value = 'application'
      await settle()
      const task = app.state.launch(2, admin)
      await settle()
      app.state.query.value = 'new query'
      await settle()
      app.state.selected.value = 1
      if (fails) pending.reject(new Error('Launch failed'))
      else pending.resolve()
      await task
      await settle()
      assert.deepEqual(app.launchCalls, [{ admin, args: ['app-2', 'application'] }])
      assert.equal(app.state.query.value, 'new query')
      assert.equal(app.state.selected.value, 1)
      assert.deepEqual(app.state.results.value, matches)
      assert.equal(app.state.launching.value, false)
      if (fails) assert.match(app.state.error.value, /Launch failed/)
    })
  }
}

test('initially hidden startup loads cached home before the handshake and focuses only after show', async t => {
  const startup = []
  const app = await launcher({
    visible: false,
    home: () => startup.push('home'),
    frontendReady: () => startup.push('frontend-ready'),
  })
  t.after(() => app.dispose())
  assert.deepEqual(startup, ['home', 'frontend-ready'])
  assert.equal(app.document.activeElement, undefined, 'hidden startup must not focus search')
  await app.emit('launcher:shown')
  assert.equal(app.document.activeElement, app.state.input.value)
  assert.ok(app.focusCalls.length > 0)
})

test('a shown event preceding native focus still focuses search without a second focus event', async t => {
  const app = await launcher()
  t.after(() => app.dispose())
  await app.emit('launcher:hidden')
  app.hasFocus = false
  await app.emit('launcher:shown')
  await app.timers.advance(100)
  assert.equal(app.focusCalls.length, 0)
  app.hasFocus = true
  await app.timers.advance(100)
  assert.ok(app.focusCalls.length > 0)
  assert.deepEqual(app.focusCalls.at(-1), { preventScroll: true })
  assert.equal(app.document.activeElement, app.state.input.value)
})

test('native focus restores a hidden launcher even when no shown event is emitted', async t => {
  const app = await launcher()
  t.after(() => app.dispose())
  app.hasFocus = false
  await app.emit('launcher:hidden')
  assert.equal(app.state.visible.value, false)
  app.hasFocus = true
  await app.windowEvent('focus')
  assert.equal(app.state.visible.value, true)
  assert.ok(app.focusCalls.length > 0)
  assert.equal(app.document.activeElement, app.state.input.value)
})

for (const panel of ['settingsOpen', 'importing', 'contextMenu']) {
  test(`pending focus restoration does not steal focus from ${panel}`, async t => {
    const app = await launcher()
    t.after(() => app.dispose())
    app.hasFocus = false
    await app.emit('launcher:shown')
    assert.ok(app.timers.pending > 0, 'native focus is still pending')
    app.state[panel].value = panel === 'contextMenu' ? { item: matches[0], x: 0, y: 0 } : true
    await settle()
    app.hasFocus = true
    await app.timers.advance(2000)
    assert.equal(app.focusCalls.length, 0)
    assert.equal(app.timers.pending, 0)
  })
}

for (const cancel of ['hidden', 'blur', 'unmount']) {
  test(`${cancel} cancels focus restoration before the window gains focus`, async t => {
    const app = await launcher()
    t.after(() => app.dispose())
    app.hasFocus = false
    await app.emit('launcher:shown')
    assert.ok(app.timers.pending > 0, 'native focus is still pending')
    if (cancel === 'hidden') await app.emit('launcher:hidden')
    else if (cancel === 'blur') await app.windowEvent('blur')
    else app.dispose()
    app.hasFocus = true
    await app.timers.advance(2000)
    assert.equal(app.focusCalls.length, 0)
    assert.equal(app.timers.pending, 0)
  })
}

test('focus restoration stops retrying when native focus never arrives', async t => {
  const app = await launcher()
  t.after(() => app.dispose())
  app.hasFocus = false
  await app.emit('launcher:shown')
  assert.ok(app.timers.pending > 0, 'native focus is still pending')
  await app.timers.advance(5000)
  assert.equal(app.focusCalls.length, 0)
  assert.equal(app.timers.pending, 0)
  app.hasFocus = true
  await app.timers.advance(1000)
  assert.equal(app.focusCalls.length, 0)
})
