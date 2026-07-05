/**
 * Tabs + drafts: the app's central state machine.
 * - copy-on-write draft on first edit (dirty == draft !== null)
 * - preview tabs (single click) promote to permanent on edit/double-click
 * - conflict marker when the file changed on disk under a dirty tab
 */
import { create } from 'zustand'
import type { RequestFileDto } from '@/lib/transport'

export type SubTab = 'params' | 'headers' | 'body' | 'auth' | 'scripts' | 'tests' | 'docs' | 'settings'

export type Conflict = 'none' | 'disk-changed' | 'file-deleted'

export interface Tab {
  id: string
  collectionId: string
  rel: string
  title: string
  method: string
  /** null = clean (view of the disk mirror); set on first edit. */
  draft: RequestFileDto | null
  /** Hash of the disk text this tab's content is based on. */
  baseHash: string
  conflict: Conflict
  preview: boolean
  activeSubTab: SubTab
}

interface ClosedTab {
  collectionId: string
  rel: string
}

interface TabsState {
  tabs: Tab[]
  activeId: string | null
  recentlyClosed: ClosedTab[]

  open: (tab: Omit<Tab, 'id'>) => string
  activate: (id: string) => void
  close: (id: string) => void
  closeOthers: (id: string) => void
  closeAll: () => void
  promote: (id: string) => void
  setDraft: (id: string, draft: RequestFileDto | null) => void
  setBase: (id: string, hash: string) => void
  setConflict: (id: string, conflict: Conflict) => void
  setSubTab: (id: string, sub: SubTab) => void
  retitle: (id: string, title: string, method: string) => void
  rekey: (collectionId: string, fromRel: string, toRel: string) => void
  reorder: (fromIndex: number, toIndex: number) => void
  popRecentlyClosed: () => ClosedTab | undefined
  byId: (id: string) => Tab | undefined
  findByRel: (collectionId: string, rel: string) => Tab | undefined
}

let nextId = 1

export const useTabs = create<TabsState>((set, get) => ({
  tabs: [],
  activeId: null,
  recentlyClosed: [],

  open(tab) {
    const existing = get().findByRel(tab.collectionId, tab.rel)
    if (existing) {
      set({ activeId: existing.id })
      return existing.id
    }
    const id = `tab-${nextId++}`
    set((s) => {
      // a preview tab is replaced by the next preview
      const previewIdx = tab.preview ? s.tabs.findIndex((t) => t.preview && t.draft === null) : -1
      const tabs = [...s.tabs]
      const newTab: Tab = { ...tab, id }
      if (previewIdx >= 0) tabs.splice(previewIdx, 1, newTab)
      else tabs.push(newTab)
      return { tabs, activeId: id }
    })
    return id
  },

  activate(id) {
    if (get().tabs.some((t) => t.id === id)) set({ activeId: id })
  },

  close(id) {
    set((s) => {
      const idx = s.tabs.findIndex((t) => t.id === id)
      if (idx === -1) return s
      const closing = s.tabs[idx]
      if (!closing) return s
      const tabs = s.tabs.filter((t) => t.id !== id)
      const activeId = s.activeId === id ? (tabs[Math.min(idx, tabs.length - 1)]?.id ?? null) : s.activeId
      return {
        tabs,
        activeId,
        recentlyClosed: [{ collectionId: closing.collectionId, rel: closing.rel }, ...s.recentlyClosed].slice(
          0,
          20,
        ),
      }
    })
  },

  closeOthers(id) {
    set((s) => ({ tabs: s.tabs.filter((t) => t.id === id), activeId: id }))
  },

  closeAll() {
    set({ tabs: [], activeId: null })
  },

  promote(id) {
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, preview: false } : t)) }))
  },

  setDraft(id, draft) {
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, draft, preview: draft ? false : t.preview } : t)),
    }))
  },

  setBase(id, hash) {
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, baseHash: hash } : t)) }))
  },

  setConflict(id, conflict) {
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, conflict } : t)) }))
  },

  setSubTab(id, sub) {
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, activeSubTab: sub } : t)) }))
  },

  retitle(id, title, method) {
    set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? { ...t, title, method } : t)) }))
  },

  rekey(collectionId, fromRel, toRel) {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.collectionId === collectionId && t.rel === fromRel ? { ...t, rel: toRel } : t,
      ),
    }))
  },

  reorder(fromIndex, toIndex) {
    set((s) => {
      const tabs = [...s.tabs]
      const [moved] = tabs.splice(fromIndex, 1)
      if (!moved) return s
      tabs.splice(toIndex, 0, moved)
      return { tabs }
    })
  },

  popRecentlyClosed() {
    const [first, ...rest] = get().recentlyClosed
    if (first) set({ recentlyClosed: rest })
    return first
  },

  byId(id) {
    return get().tabs.find((t) => t.id === id)
  },

  findByRel(collectionId, rel) {
    return get().tabs.find((t) => t.collectionId === collectionId && t.rel === rel)
  },
}))
