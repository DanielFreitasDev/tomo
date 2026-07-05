/**
 * Mirror of on-disk truth: the tree and lazily-loaded parsed requests.
 * NEVER contains edits — drafts live on tabs. Mutations happen only through
 * stores/actions (optimistic ops + watcher reconciliation).
 */
import { create } from 'zustand'
import type { CollectionTreeDto, RequestFileDto, TreeNodeDto } from '@/lib/transport'

export interface CollectionInfo {
  id: string
  name: string
  path: string
  environments: string[]
  selectedEnv?: string
  invalid: { rel: string; error: string }[]
}

interface CollectionsState {
  /** Open collections in open order. */
  order: string[]
  byId: Record<string, CollectionInfo>
  /** Tree roots per collection. */
  trees: Record<string, TreeNodeDto[]>
  /** Parsed request cache: `${id} ${rel}` -> { request, hash }. */
  requests: Record<string, { request: RequestFileDto; hash: string }>

  setCollection: (tree: CollectionTreeDto) => void
  removeCollection: (id: string) => void
  setRequest: (id: string, rel: string, request: RequestFileDto, hash: string) => void
  dropRequest: (id: string, rel: string) => void
  moveRequestKey: (id: string, fromRel: string, toRel: string) => void
  setSelectedEnv: (id: string, env?: string) => void
}

export const requestKey = (id: string, rel: string) => `${id} ${rel}`

export const useCollections = create<CollectionsState>((set) => ({
  order: [],
  byId: {},
  trees: {},
  requests: {},

  setCollection(tree) {
    set((s) => ({
      order: s.order.includes(tree.id) ? s.order : [...s.order, tree.id],
      byId: {
        ...s.byId,
        [tree.id]: {
          id: tree.id,
          name: tree.name,
          path: tree.path,
          environments: tree.environments,
          selectedEnv: tree.selected_environment,
          invalid: tree.invalid,
        },
      },
      trees: { ...s.trees, [tree.id]: tree.nodes },
    }))
  },

  removeCollection(id) {
    set((s) => {
      const byId = { ...s.byId }
      const trees = { ...s.trees }
      delete byId[id]
      delete trees[id]
      const requests = Object.fromEntries(Object.entries(s.requests).filter(([k]) => !k.startsWith(`${id} `)))
      return { order: s.order.filter((x) => x !== id), byId, trees, requests }
    })
  },

  setRequest(id, rel, request, hash) {
    set((s) => ({ requests: { ...s.requests, [requestKey(id, rel)]: { request, hash } } }))
  },

  dropRequest(id, rel) {
    set((s) => {
      const requests = { ...s.requests }
      delete requests[requestKey(id, rel)]
      return { requests }
    })
  },

  moveRequestKey(id, fromRel, toRel) {
    set((s) => {
      const requests = { ...s.requests }
      const entry = requests[requestKey(id, fromRel)]
      if (entry) {
        delete requests[requestKey(id, fromRel)]
        requests[requestKey(id, toRel)] = entry
      }
      return { requests }
    })
  },

  setSelectedEnv(id, env) {
    set((s) => {
      const info = s.byId[id]
      if (!info) return s
      return { byId: { ...s.byId, [id]: { ...info, selectedEnv: env } } }
    })
  },
}))

/** Depth-first walk of a collection tree. */
export function walkTree(nodes: TreeNodeDto[], visit: (node: TreeNodeDto, depth: number) => void): void {
  const rec = (list: TreeNodeDto[], depth: number) => {
    for (const n of list) {
      visit(n, depth)
      if (n.children) rec(n.children, depth + 1)
    }
  }
  rec(nodes, 0)
}

export function findNode(nodes: TreeNodeDto[], rel: string): TreeNodeDto | undefined {
  let found: TreeNodeDto | undefined
  walkTree(nodes, (n) => {
    if (n.rel === rel) found = n
  })
  return found
}
