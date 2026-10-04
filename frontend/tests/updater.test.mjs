import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createUpdater } from '../src/updater.ts'

function deferred() {
  let resolve, reject
  const promise = new Promise((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

test('an ongoing download shares progress across settings views and rejects repeated starts', async () => {
  const download = deferred()
  let installs = 0, checks = 0
  const updater = createUpdater({
    check: async () => { checks++; return { available: true, version: '9.9.9' } },
    install: () => { installs++; return download.promise },
  })
  await updater.check()
  const task = updater.install()
  const firstPanel = updater.state
  firstPanel.progress = 38
  const reopenedPanel = updater.state
  assert.equal(reopenedPanel.progress, 38)
  assert.equal(reopenedPanel.installing, true)
  assert.equal(reopenedPanel.info.version, '9.9.9')
  await updater.install()
  await updater.check()
  assert.equal(installs, 1)
  assert.equal(checks, 1)
  download.resolve()
  await task
  assert.equal(reopenedPanel.installing, true, 'successful scheduling stays locked until exit')
  assert.match(reopenedPanel.message, /自动重新启动/)
})

test('a failed download retains the error and permits a fresh attempt', async () => {
  let installs = 0
  const updater = createUpdater({
    check: async () => ({ available: true, version: '9.9.9' }),
    install: async () => { if (++installs === 1) throw new Error('网络中断') },
  })
  await updater.install()
  assert.equal(updater.state.installing, false)
  assert.match(updater.state.message, /网络中断/)
  updater.state.progress = 75
  await updater.install()
  assert.equal(installs, 2)
  assert.equal(updater.state.progress, 0)
  assert.equal(updater.state.installing, true)
})

test('checking is serialized with installing and recovers from a failed check', async () => {
  const check = deferred()
  let checks = 0, installs = 0
  const updater = createUpdater({ check: () => { checks++; return check.promise }, install: async () => { installs++ } })
  const task = updater.check()
  await updater.check()
  await updater.install()
  assert.equal(checks, 1)
  assert.equal(installs, 0)
  check.reject(new Error('连接失败'))
  await task
  assert.equal(updater.state.checking, false)
  assert.match(updater.state.message, /连接失败/)
})
