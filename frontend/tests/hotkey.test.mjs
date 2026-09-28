import assert from 'node:assert/strict'
import { test } from 'node:test'
import { recordedHotkey } from '../src/hotkey.ts'

const input = (overrides) => ({ key: '', code: '', ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...overrides })
test('recorded chords match backend names and retain shifted digit keys', () => {
  const cases = [
    [{ key: ' ', code: 'Space', altKey: true }, 'Alt+Space'],
    [{ key: 'k', code: 'KeyK', ctrlKey: true, altKey: true }, 'Ctrl+Alt+K'],
    [{ key: '!', code: 'Digit1', ctrlKey: true, shiftKey: true }, 'Ctrl+Shift+1'],
    [{ key: 'F24', code: 'F24', metaKey: true, shiftKey: true }, 'Shift+Win+F24'],
    [{ key: 'Enter', code: 'Enter', ctrlKey: true }, 'Ctrl+Enter'],
    [{ key: 'Tab', code: 'Tab', ctrlKey: true }, 'Ctrl+Tab'],
    [{ key: 'ф', code: 'KeyA', ctrlKey: true }, 'Ctrl+A'],
    [{ key: 'k', code: '', ctrlKey: true, shiftKey: true }, 'Ctrl+Shift+K'],
  ]
  for (const [event, expected] of cases) assert.equal(recordedHotkey(input(event)), expected)
})
test('standalone, modifier-only and unsupported keys cannot become shortcuts', () => {
  for (const event of [
    { key: 'a', code: 'KeyA' },
    { key: 'Control', code: 'ControlLeft', ctrlKey: true },
    { key: ',', code: 'Comma', ctrlKey: true },
    { key: 'Escape', code: 'Escape', altKey: true },
    { key: 'F25', code: 'F25', ctrlKey: true },
    { key: '1', code: 'Numpad1', ctrlKey: true },
  ]) assert.equal(recordedHotkey(input(event)), null)
})
