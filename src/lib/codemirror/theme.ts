/**
 * CodeMirror theme wired to our CSS custom properties — editors follow
 * light/dark switches with zero re-instantiation.
 */
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import type { Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { tags } from '@lezer/highlight'

const chrome = EditorView.theme({
  '&': {
    fontSize: '13px',
    fontFamily: 'var(--font-mono)',
    color: 'var(--text-primary)',
    backgroundColor: 'transparent',
    height: '100%',
  },
  '.cm-content': {
    fontFamily: 'var(--font-mono)',
    caretColor: 'var(--accent)',
    padding: '8px 0',
  },
  '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.55' },
  '&.cm-focused': { outline: 'none' },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    color: 'var(--text-muted)',
    borderRight: '1px solid var(--border-subtle)',
  },
  '.cm-activeLine': { backgroundColor: 'var(--bg-hover)' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--text-secondary)' },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': {
    backgroundColor: 'var(--selection-bg) !important',
  },
  '.cm-cursor': { borderLeftColor: 'var(--accent)' },
  '.cm-matchingBracket': {
    outline: '1px solid var(--accent-soft-border)',
    borderRadius: '2px',
    backgroundColor: 'transparent',
  },
  '.cm-searchMatch': { backgroundColor: 'var(--warning-soft)' },
  '.cm-searchMatch-selected': { outline: '1px solid var(--warning)' },
  '.cm-panels': {
    backgroundColor: 'var(--bg-overlay)',
    color: 'var(--text-primary)',
    borderTop: '1px solid var(--border-subtle)',
  },
  '.cm-panels input, .cm-panels button': {
    fontFamily: 'var(--font-sans)',
    fontSize: '12px',
  },
  '.cm-tooltip': {
    backgroundColor: 'var(--bg-overlay)',
    border: 'none',
    borderRadius: '8px',
    boxShadow: 'var(--shadow-md)',
    color: 'var(--text-primary)',
    fontFamily: 'var(--font-sans)',
    fontSize: '12px',
    padding: '4px 8px',
  },
  '.cm-lintRange-error': {
    backgroundImage: 'none',
    textDecoration: 'underline wavy var(--danger)',
    textUnderlineOffset: '3px',
  },
  // {{variable}} decorations
  '.tomo-var-defined': {
    color: 'var(--var-defined)',
    backgroundColor: 'var(--var-defined-bg)',
    borderRadius: '3px',
  },
  '.tomo-var-undefined': {
    color: 'var(--var-undefined)',
    backgroundColor: 'var(--var-undefined-bg)',
    borderRadius: '3px',
    textDecoration: 'underline wavy var(--var-undefined)',
    textUnderlineOffset: '3px',
  },
})

const highlight = HighlightStyle.define([
  { tag: tags.comment, color: 'var(--syn-comment)', fontStyle: 'italic' },
  { tag: [tags.string, tags.special(tags.string)], color: 'var(--syn-string)' },
  { tag: [tags.number, tags.integer, tags.float], color: 'var(--syn-number)' },
  { tag: [tags.bool, tags.atom, tags.null], color: 'var(--syn-atom)' },
  { tag: [tags.propertyName, tags.attributeName], color: 'var(--syn-property)' },
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: 'var(--syn-keyword)' },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: 'var(--syn-function)' },
  { tag: [tags.definition(tags.variableName), tags.className], color: 'var(--syn-definition)' },
  { tag: tags.operator, color: 'var(--syn-operator)' },
  { tag: [tags.tagName, tags.angleBracket], color: 'var(--syn-tag)' },
  { tag: tags.invalid, color: 'var(--syn-invalid)' },
  { tag: tags.link, color: 'var(--syn-link)', textDecoration: 'underline' },
])

export function tomoTheme(): Extension {
  return [chrome, syntaxHighlighting(highlight)]
}
