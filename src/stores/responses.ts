/** Per-tab response state — in-memory only, never persisted. */
import { create } from 'zustand'
import type { ResponseMetaDto } from '@/lib/transport'

export type ResponsePhase = 'idle' | 'sending' | 'done' | 'error' | 'cancelled'

export interface ResponseState {
  phase: ResponsePhase
  runId?: string
  startedAt?: number
  meta?: ResponseMetaDto
  /** In-memory preview bytes fetched via get_response_body. */
  bodyBytes?: Uint8Array
  error?: string
}

interface ResponsesState {
  byTab: Record<string, ResponseState>
  start: (tabId: string, runId: string) => void
  complete: (tabId: string, meta: ResponseMetaDto, bodyBytes: Uint8Array) => void
  fail: (tabId: string, error: string) => void
  cancelledAt: (tabId: string) => void
  clear: (tabId: string) => void
  of: (tabId: string) => ResponseState
}

const IDLE: ResponseState = { phase: 'idle' }

export const useResponses = create<ResponsesState>((set, get) => ({
  byTab: {},

  start(tabId, runId) {
    set((s) => ({
      byTab: { ...s.byTab, [tabId]: { phase: 'sending', runId, startedAt: Date.now() } },
    }))
  },

  complete(tabId, meta, bodyBytes) {
    set((s) => {
      const current = s.byTab[tabId]
      return {
        byTab: {
          ...s.byTab,
          [tabId]: { ...current, phase: 'done', meta, bodyBytes },
        },
      }
    })
  },

  fail(tabId, error) {
    set((s) => ({
      byTab: { ...s.byTab, [tabId]: { ...s.byTab[tabId], phase: 'error', error } },
    }))
  },

  cancelledAt(tabId) {
    set((s) => ({
      byTab: { ...s.byTab, [tabId]: { ...s.byTab[tabId], phase: 'cancelled' } },
    }))
  },

  clear(tabId) {
    set((s) => {
      const byTab = { ...s.byTab }
      delete byTab[tabId]
      return { byTab }
    })
  },

  of(tabId) {
    return get().byTab[tabId] ?? IDLE
  },
}))
