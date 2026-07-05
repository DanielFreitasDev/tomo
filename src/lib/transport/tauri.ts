/** 1:1 mapping of the contract onto Tauri invoke/listen. */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { type Transport, TransportError } from './contract'

export function createTauriTransport(): Transport {
  return {
    async invoke(cmd, args) {
      try {
        return await invoke(cmd as string, args as Record<string, unknown>)
      } catch (raw) {
        // ApiError serializes as { code, message }
        if (raw && typeof raw === 'object' && 'message' in raw) {
          const e = raw as { code?: string; message: string }
          throw new TransportError(e.code ?? 'error', e.message)
        }
        throw new TransportError('error', String(raw))
      }
    },
    listen(event, handler) {
      let disposed = false
      let unlisten: (() => void) | undefined
      listen(event as string, (e) => handler(e.payload as never)).then((fn) => {
        if (disposed) fn()
        else unlisten = fn
      })
      return () => {
        disposed = true
        unlisten?.()
      }
    },
  }
}
