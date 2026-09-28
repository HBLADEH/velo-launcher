export type KeyInput = Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>

export function modifiers(event: KeyInput): string[] {
  return [event.ctrlKey && 'Ctrl', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Win'].filter(Boolean) as string[]
}

// Use physical letter/digit codes: Shift+1 must remain Shift+1, not Shift+!.
// Names intentionally match internal/hotkey/parse.go.
export function recordedHotkey(event: KeyInput): string | null {
  const parts = modifiers(event)
  if (!parts.length) return null
  let key = ''
  if (/^Key[A-Z]$/.test(event.code)) key = event.code.slice(3)
  else if (/^Digit[0-9]$/.test(event.code)) key = event.code.slice(5)
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(event.key)) key = event.key
  else if (event.code === 'Space') key = 'Space'
  else if (event.code === 'Enter') key = 'Enter'
  else if (event.code === 'Tab') key = 'Tab'
  // Some input providers omit the physical code (e.g. accessibility input).
  else if ((!event.code || event.code === 'Unidentified') && /^[a-z0-9]$/i.test(event.key)) key = event.key.toUpperCase()
  return key ? [...parts, key].join('+') : null
}
