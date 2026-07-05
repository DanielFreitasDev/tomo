/** Environment contents cache + variable resolution for editor decorations. */
import { create } from 'zustand'
import type { EnvironmentDto } from '@/lib/transport'
import { useCollections } from './collections'

interface EnvironmentsState {
  /** `${collectionId} ${envName}` -> env contents. */
  byKey: Record<string, EnvironmentDto>
  setEnv: (collectionId: string, env: EnvironmentDto) => void
  dropEnv: (collectionId: string, name: string) => void
}

export const envKey = (collectionId: string, name: string) => `${collectionId} ${name}`

export const useEnvironments = create<EnvironmentsState>((set) => ({
  byKey: {},

  setEnv(collectionId, env) {
    set((s) => ({ byKey: { ...s.byKey, [envKey(collectionId, env.meta.name)]: env } }))
  },

  dropEnv(collectionId, name) {
    set((s) => {
      const byKey = { ...s.byKey }
      delete byKey[envKey(collectionId, name)]
      return { byKey }
    })
  },
}))

export interface ResolvedVar {
  value: string
  scope: 'environment' | 'collection' | 'request'
  secret: boolean
}

/**
 * Display-only resolution for {{var}} decorations (the engine does the real
 * one). Environment vars of the active env, marked secret when listed.
 */
export function resolveVar(collectionId: string, name: string): ResolvedVar | undefined {
  const info = useCollections.getState().byId[collectionId]
  if (!info?.selectedEnv) return undefined
  const env = useEnvironments.getState().byKey[envKey(collectionId, info.selectedEnv)]
  if (!env) return undefined
  const secret = env.meta.secrets?.includes(name) ?? false
  if (name in env.vars) {
    const raw = env.vars[name]
    return {
      value: typeof raw === 'string' ? raw : JSON.stringify(raw),
      scope: 'environment',
      secret,
    }
  }
  if (secret) return { value: '', scope: 'environment', secret: true }
  return undefined
}
