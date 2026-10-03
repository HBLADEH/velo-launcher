import assert from 'node:assert/strict'
import { test } from 'node:test'
import { choiceRows, navigateChoices } from '../src/navigation.ts'

test('grid navigation follows columns across groups and incomplete rows', () => {
  const rows = choiceRows([{ offset: 0, items: Array(8) }, { offset: 8, items: Array(4) }])
  assert.deepEqual(rows, [[0, 1, 2, 3, 4, 5], [6, 7], [8, 9, 10, 11]])
  for (const [current, key, expected] of [
    [4, 'ArrowDown', 7], [7, 'ArrowDown', 9], [9, 'ArrowUp', 7],
    [0, 'ArrowUp', 8], [11, 'ArrowDown', 3],
    [7, 'ArrowRight', 8], [8, 'ArrowLeft', 7], [0, 'ArrowLeft', 11],
  ]) assert.equal(navigateChoices(rows, current, key), expected)
})

test('empty groups and list navigation retain predictable selection', () => {
  assert.equal(navigateChoices([], 0, 'ArrowDown'), 0)
  const rows = choiceRows([{ offset: 0, items: [] }, { offset: 0, items: Array(3) }], 1)
  assert.deepEqual(rows, [[0], [1], [2]])
  assert.equal(navigateChoices(rows, 0, 'ArrowUp'), 2)
  assert.equal(navigateChoices(rows, 2, 'ArrowDown'), 0)
  assert.equal(navigateChoices(rows, 10, 'ArrowDown'), 0)
})
