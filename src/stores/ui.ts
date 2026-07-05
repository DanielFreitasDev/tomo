/** Layout + ephemeral UI state. Layout parts are persisted via ui_state. */
import { create } from 'zustand'
import { transport } from '@/lib/transport'

export type ModalKind = 'settings' | 'environments' | 'about' | 'shortcuts' | null

interface UiState {
  sidebarWidth: number
  sidebarCollapsed: boolean
  filter: string
  expanded: Record<string, true>
  splitOrientation: 'horizontal' | 'vertical'
  paletteOpen: boolean
  modal: ModalKind

  setFilter: (filter: string) => void
  toggleSidebar: () => void
  setSidebarWidth: (w: number) => void
  toggleExpanded: (rel: string) => void
  expand: (rel: string) => void
  toggleSplit: () => void
  setPaletteOpen: (open: boolean) => void
  openModal: (modal: ModalKind) => void
  hydrate: () => Promise<void>
}

interface PersistedUi {
  sidebarWidth: number
  sidebarCollapsed: boolean
  expanded: Record<string, true>
  splitOrientation: 'horizontal' | 'vertical'
}

let persistTimer: ReturnType<typeof setTimeout> | undefined

function persist(get: () => UiState) {
  clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    const { sidebarWidth, sidebarCollapsed, expanded, splitOrientation } = get()
    const state: PersistedUi = { sidebarWidth, sidebarCollapsed, expanded, splitOrientation }
    void transport().invoke('save_ui_state', { state })
  }, 500)
}

export const useUi = create<UiState>((set, get) => ({
  sidebarWidth: 280,
  sidebarCollapsed: false,
  filter: '',
  expanded: {},
  splitOrientation: 'horizontal',
  paletteOpen: false,
  modal: null,

  setFilter(filter) {
    set({ filter })
  },
  toggleSidebar() {
    set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed }))
    persist(get)
  },
  setSidebarWidth(w) {
    set({ sidebarWidth: Math.max(200, Math.min(480, w)) })
    persist(get)
  },
  toggleExpanded(rel) {
    set((s) => {
      const expanded = { ...s.expanded }
      if (expanded[rel]) delete expanded[rel]
      else expanded[rel] = true
      return { expanded }
    })
    persist(get)
  },
  expand(rel) {
    set((s) => ({ expanded: { ...s.expanded, [rel]: true } }))
  },
  toggleSplit() {
    set((s) => ({
      splitOrientation: s.splitOrientation === 'horizontal' ? 'vertical' : 'horizontal',
    }))
    persist(get)
  },
  setPaletteOpen(paletteOpen) {
    set({ paletteOpen })
  },
  openModal(modal) {
    set({ modal })
  },

  async hydrate() {
    const raw = (await transport().invoke('get_ui_state', {})) as PersistedUi | null
    if (raw && typeof raw === 'object') {
      set({
        sidebarWidth: raw.sidebarWidth ?? 280,
        sidebarCollapsed: raw.sidebarCollapsed ?? false,
        expanded: raw.expanded ?? {},
        splitOrientation: raw.splitOrientation ?? 'horizontal',
      })
    }
  },
}))
