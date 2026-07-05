/**
 * {{variable}} decorations: defined = green chip with a hover tooltip showing
 * the resolved value (secrets masked) and its scope; undefined = red wavy.
 * Shared by the URL bar, KV value cells and body editors.
 */
import type { Extension } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  hoverTooltip,
  MatchDecorator,
  ViewPlugin,
  type ViewUpdate,
} from '@codemirror/view'
import { resolveVar } from '@/stores/environments'

const VAR_RE = /\{\{\s*(\$?[\w.-]+(?:\[\d+\])?[\w.-]*)\s*\}\}/g

const DYNAMIC = new Set(['$uuid', '$timestamp', '$isoTimestamp', '$randomInt'])

function classify(collectionId: string, token: string): 'defined' | 'undefined' | 'dynamic' {
  if (token.startsWith('$')) return DYNAMIC.has(token) ? 'dynamic' : 'undefined'
  const root = token.split(/[.[]/, 1)[0] ?? token
  return resolveVar(collectionId, root) ? 'defined' : 'undefined'
}

export function variableDecorations(collectionId: string): Extension {
  const decorator = new MatchDecorator({
    regexp: VAR_RE,
    decoration: (match) => {
      const token = match[1] ?? ''
      const kind = classify(collectionId, token)
      return Decoration.mark({
        class: kind === 'undefined' ? 'tomo-var-undefined' : 'tomo-var-defined',
      })
    },
  })

  const plugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet
      constructor(view: EditorView) {
        this.decorations = decorator.createDeco(view)
      }
      update(update: ViewUpdate) {
        this.decorations = decorator.updateDeco(update, this.decorations)
      }
    },
    { decorations: (v) => v.decorations },
  )

  const tooltip = hoverTooltip((view, pos) => {
    const line = view.state.doc.lineAt(pos)
    const text = line.text
    VAR_RE.lastIndex = 0
    let m = VAR_RE.exec(text)
    while (m) {
      const from = line.from + m.index
      const to = from + m[0].length
      if (pos >= from && pos <= to) {
        const token = m[1] ?? ''
        return {
          pos: from,
          end: to,
          above: true,
          create: () => ({ dom: tooltipDom(collectionId, token) }),
        }
      }
      m = VAR_RE.exec(text)
    }
    return null
  })

  return [plugin, tooltip]
}

function tooltipDom(collectionId: string, token: string): HTMLElement {
  const dom = document.createElement('div')
  dom.className = 'flex flex-col gap-0.5 py-0.5'

  const name = document.createElement('div')
  name.className = 'font-mono text-xs font-semibold'
  name.textContent = token
  dom.appendChild(name)

  const detail = document.createElement('div')
  detail.className = 'font-mono text-xs'

  if (token.startsWith('$')) {
    detail.textContent = DYNAMIC.has(token) ? 'dynamic — fresh value per send' : 'unknown dynamic variable'
    detail.style.color = DYNAMIC.has(token) ? 'var(--text-secondary)' : 'var(--danger)'
  } else {
    const root = token.split(/[.[]/, 1)[0] ?? token
    const resolved = resolveVar(collectionId, root)
    if (resolved) {
      detail.textContent = resolved.secret ? '••••••••' : resolved.value
      detail.style.color = 'var(--var-defined)'
      const scope = document.createElement('div')
      scope.className = 'text-2xs'
      scope.style.color = 'var(--text-muted)'
      scope.textContent = resolved.secret ? `secret · ${resolved.scope}` : resolved.scope
      dom.appendChild(scope)
    } else {
      detail.textContent = 'not defined in the active environment'
      detail.style.color = 'var(--danger)'
    }
  }
  dom.insertBefore(detail, dom.children[1] ?? null)
  return dom
}
