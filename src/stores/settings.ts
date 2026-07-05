import { create } from 'zustand'
import { detectSystemLocale, type Locale } from '@/i18n'
import { defaultSettings, type SettingsDto, transport } from '@/lib/transport'

interface SettingsState extends SettingsDto {
  loaded: boolean
  load: () => Promise<void>
  update: (patch: Partial<SettingsDto>) => void
  resolvedTheme: () => 'light' | 'dark'
}

let persistTimer: ReturnType<typeof setTimeout> | undefined

function systemPrefersDark(): boolean {
  return typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches
}

export function applyThemeToDocument(theme: SettingsDto['theme']): void {
  const dark = theme === 'dark' || (theme === 'system' && systemPrefersDark())
  document.documentElement.classList.toggle('dark', dark)
}

export const useSettings = create<SettingsState>((set, get) => ({
  ...defaultSettings(),
  locale: undefined,
  loaded: false,

  async load() {
    const settings = await transport().invoke('get_settings', {})
    set({ ...settings, loaded: true })
    applyThemeToDocument(settings.theme)
  },

  update(patch) {
    set(patch)
    const { loaded, load, update, resolvedTheme, ...settings } = get()
    applyThemeToDocument(settings.theme)
    clearTimeout(persistTimer)
    persistTimer = setTimeout(() => {
      void transport().invoke('save_settings', { settings })
    }, 300)
  },

  resolvedTheme() {
    const theme = get().theme
    if (theme === 'system') return systemPrefersDark() ? 'dark' : 'light'
    return theme
  },
}))

/** Follow OS theme changes while in system mode. */
export function watchSystemTheme(): () => void {
  const mq = window.matchMedia('(prefers-color-scheme: dark)')
  const onChange = () => {
    if (useSettings.getState().theme === 'system') applyThemeToDocument('system')
  }
  mq.addEventListener('change', onChange)
  return () => mq.removeEventListener('change', onChange)
}

export function currentLocale(): Locale {
  return useSettings.getState().locale ?? detectSystemLocale()
}
