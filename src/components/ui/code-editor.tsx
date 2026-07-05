/**
 * React wrapper around CodeMirror 6. One factory keeps extension sets
 * consistent app-wide; value/onChange stay controlled from outside.
 */
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import { html } from '@codemirror/lang-html'
import { javascript } from '@codemirror/lang-javascript'
import { json, jsonParseLinter } from '@codemirror/lang-json'
import { xml } from '@codemirror/lang-xml'
import { bracketMatching, foldGutter, indentOnInput } from '@codemirror/language'
import { linter, lintGutter } from '@codemirror/lint'
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
import { Compartment, EditorState, type Extension } from '@codemirror/state'
import { placeholder as cmPlaceholder, EditorView, keymap, lineNumbers } from '@codemirror/view'
import { useEffect, useRef } from 'react'
import { cn } from '@/lib/cn'
import { tomoTheme } from '@/lib/codemirror/theme'
import { variableDecorations } from '@/lib/codemirror/variables'

export type EditorLanguage = 'json' | 'javascript' | 'xml' | 'html' | 'graphql' | 'text'

async function languageExtension(lang: EditorLanguage): Promise<Extension> {
  switch (lang) {
    case 'json':
      return [json(), linter(jsonParseLinter(), { delay: 500 }), lintGutter()]
    case 'javascript':
      return javascript()
    case 'xml':
      return xml()
    case 'html':
      return html()
    case 'graphql': {
      const { graphql } = await import('cm6-graphql')
      return graphql()
    }
    default:
      return []
  }
}

export interface CodeEditorProps {
  value: string
  onChange?: (value: string) => void
  language?: EditorLanguage
  /** Enables {{var}} decorations resolved against this collection. */
  collectionId?: string
  singleLine?: boolean
  readOnly?: boolean
  placeholder?: string
  className?: string
  /** Extra keybindings, e.g. Enter-to-send on the URL bar. */
  onEnter?: () => void
  ariaLabel?: string
}

export function CodeEditor({
  value,
  onChange,
  language = 'text',
  collectionId,
  singleLine = false,
  readOnly = false,
  placeholder,
  className,
  onEnter,
  ariaLabel,
}: CodeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)
  const onChangeRef = useRef(onChange)
  const onEnterRef = useRef(onEnter)
  onChangeRef.current = onChange
  onEnterRef.current = onEnter
  const langCompartment = useRef(new Compartment())

  // editors are recreated only on identity-level prop changes; `value` flows
  // through the second effect (avoids resetting the view every keystroke) and
  // ariaLabel is static per mount
  // biome-ignore lint/correctness/useExhaustiveDependencies: see above
  useEffect(() => {
    if (!hostRef.current) return

    const extensions: Extension[] = [
      tomoTheme(),
      EditorView.contentAttributes.of(ariaLabel ? { 'aria-label': ariaLabel } : {}),
      history(),
      bracketMatching(),
      indentOnInput(),
      highlightSelectionMatches(),
      keymap.of([
        ...(onEnterRef.current
          ? [
              {
                key: 'Enter',
                run: () => {
                  onEnterRef.current?.()
                  return true
                },
              },
            ]
          : []),
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
        indentWithTab,
      ]),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) onChangeRef.current?.(update.state.doc.toString())
      }),
      langCompartment.current.of([]),
    ]

    if (collectionId) extensions.push(variableDecorations(collectionId))
    if (placeholder) extensions.push(cmPlaceholder(placeholder))
    if (readOnly) extensions.push(EditorState.readOnly.of(true))
    if (singleLine) {
      extensions.push(
        EditorState.transactionFilter.of((tr) => (tr.newDoc.lines > 1 ? [] : tr)),
        EditorView.theme({ '.cm-content': { padding: '6px 0' } }),
      )
    } else {
      extensions.push(lineNumbers(), foldGutter(), EditorView.lineWrapping)
    }

    const view = new EditorView({
      state: EditorState.create({ doc: value, extensions }),
      parent: hostRef.current,
    })
    viewRef.current = view

    void languageExtension(language).then((ext) => {
      if (viewRef.current === view) {
        view.dispatch({ effects: langCompartment.current.reconfigure(ext) })
      }
    })

    return () => {
      view.destroy()
      viewRef.current = null
    }
  }, [language, collectionId, singleLine, readOnly, placeholder])

  // external value changes (tab switch, reload from disk)
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    if (view.state.doc.toString() !== value) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } })
    }
  }, [value])

  return (
    <div
      ref={hostRef}
      data-selectable
      className={cn('min-h-0 overflow-hidden', singleLine ? 'cm-single-line' : 'h-full', className)}
    />
  )
}
