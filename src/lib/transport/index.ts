/**
 * Transport selection happens exactly once, at bootstrap:
 * - inside Tauri: the real backend
 * - in a plain browser (dev, e2e): the in-memory mock
 * Nothing outside this folder may import @tauri-apps/api.
 */
import type { Transport } from './contract'
import { createMockTransport } from './mock'
import { createTauriTransport } from './tauri'

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined
}

let instance: Transport | undefined

export function transport(): Transport {
  if (!instance) {
    instance = isTauri() ? createTauriTransport() : createMockTransport()
  }
  return instance
}

/** Test seam: swap the transport (unit tests inject their own). */
export function setTransport(t: Transport): void {
  instance = t
}

export * from './contract'
