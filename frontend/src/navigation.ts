export type ChoiceSection = { offset: number; items: readonly unknown[] }

export function choiceRows(sections: readonly ChoiceSection[], columns = 6): number[][] {
  return sections.flatMap(section => Array.from({ length: Math.ceil(section.items.length / columns) }, (_, row) =>
    Array.from({ length: Math.min(columns, section.items.length - row * columns) }, (_, column) => section.offset + row * columns + column)))
}

export function navigateChoices(rows: readonly number[][], selected: number, key: string): number {
  const choices = rows.flat()
  if (!choices.length) return 0
  const currentRow = rows.findIndex(row => row.includes(selected))
  if (currentRow < 0) return choices[0] ?? 0
  const direction = key === 'ArrowDown' || key === 'ArrowRight' ? 1 : -1
  if (key === 'ArrowDown' || key === 'ArrowUp') {
    const column = rows[currentRow]?.indexOf(selected) ?? 0
    const target = rows[(currentRow + direction + rows.length) % rows.length]
    return target?.[Math.min(column, target.length - 1)] ?? 0
  }
  return choices[(choices.indexOf(selected) + direction + choices.length) % choices.length] ?? 0
}
