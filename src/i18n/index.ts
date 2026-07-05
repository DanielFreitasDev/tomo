/**
 * Hand-rolled typed i18n (~60 lines, zero deps): en is the source of truth,
 * pt-BR completeness is compile-enforced, {param} interpolation, instant
 * locale switching via the settings store.
 */
import { useSettings } from '@/stores/settings'
import { en, type MessageKey } from './messages/en'
import { ptBR } from './messages/pt-BR'

export type Locale = 'en' | 'pt-BR'

const dictionaries: Record<Locale, Record<MessageKey, string>> = {
  en,
  'pt-BR': ptBR,
}

export function detectSystemLocale(): Locale {
  const lang = typeof navigator !== 'undefined' ? navigator.language : 'en'
  return lang.toLowerCase().startsWith('pt') ? 'pt-BR' : 'en'
}

export function translate(locale: Locale, key: MessageKey, params?: Record<string, string | number>): string {
  const template = dictionaries[locale][key] ?? en[key] ?? key
  if (!params) return template
  return template.replace(/\{(\w+)\}/g, (_, name: string) =>
    params[name] !== undefined ? String(params[name]) : `{${name}}`,
  )
}

/** Reactive hook — re-renders when the locale setting changes. */
export function useT() {
  const locale = useSettings((s) => s.locale ?? detectSystemLocale())
  return (key: MessageKey, params?: Record<string, string | number>) => translate(locale, key, params)
}

export type { MessageKey }
