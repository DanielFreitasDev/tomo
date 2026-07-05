/**
 * Single source of truth for keyboard shortcuts. Window-level listener with
 * simple precedence: when a modal/palette is open, global shortcuts (except
 * palette/escape) are inert. CodeMirror editors re-register the same actions
 * with Prec.high keymaps later (M11).
 */

export interface ShortcutDef {
  id: string
  /** e.g. "mod+enter", "mod+shift+t", "f2" — mod = Ctrl (⌘ on mac later) */
  combo: string
  run: () => void
  /** allow while typing in inputs/editors */
  inInputs?: boolean
}

function comboOf(e: KeyboardEvent): string {
  const parts: string[] = []
  if (e.ctrlKey || e.metaKey) parts.push('mod')
  if (e.altKey) parts.push('alt')
  if (e.shiftKey) parts.push('shift')
  const key = e.key.toLowerCase()
  if (!['control', 'meta', 'alt', 'shift'].includes(key)) parts.push(key === ' ' ? 'space' : key)
  return parts.join('+')
}

function isTypingTarget(e: KeyboardEvent): boolean {
  const el = e.target as HTMLElement | null
  if (!el) return false
  const tag = el.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || el.isContentEditable || Boolean(el.closest('.cm-editor'))
}

const registry = new Map<string, ShortcutDef>()

export function registerShortcuts(defs: ShortcutDef[]): () => void {
  for (const def of defs) registry.set(def.id, def)
  return () => {
    for (const def of defs) registry.delete(def.id)
  }
}

let installed = false

export function installKeyboardListener(): () => void {
  if (installed) return () => {}
  installed = true
  const onKeyDown = (e: KeyboardEvent) => {
    const combo = comboOf(e)
    for (const def of registry.values()) {
      if (def.combo !== combo) continue
      if (!def.inInputs && isTypingTarget(e)) continue
      e.preventDefault()
      def.run()
      return
    }
  }
  window.addEventListener('keydown', onKeyDown)
  return () => {
    installed = false
    window.removeEventListener('keydown', onKeyDown)
  }
}

/** Render helper: "mod+enter" -> "Ctrl+⏎" for tooltips/menus. */
export function formatCombo(combo: string): string {
  return combo
    .split('+')
    .map((part) => {
      switch (part) {
        case 'mod':
          return 'Ctrl'
        case 'enter':
          return '⏎'
        case 'shift':
          return 'Shift'
        case 'alt':
          return 'Alt'
        default:
          return part.length === 1 ? part.toUpperCase() : part[0]?.toUpperCase() + part.slice(1)
      }
    })
    .join('+')
}
