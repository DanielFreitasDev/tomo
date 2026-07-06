/**
 * Collection/filesystem orchestration + watcher reconciliation.
 *
 * Watcher merge rules (Bruno-proven, from the plan):
 * 1. always refresh the disk mirror
 * 2. clean tab -> nothing else (UI re-renders from the mirror)
 * 3. draft deep-equals incoming -> drop draft (our own save echo)
 * 4. seq-only change -> merge seq into draft, stay dirty
 * 5. real difference -> keep draft, mark conflict (never clobber typing)
 */
import { type RequestFileDto, type TreeNodeDto, transport } from '@/lib/transport'
import { requestKey, useCollections, walkTree } from '@/stores/collections'
import { useEnvironments } from '@/stores/environments'
import { useTabs } from '@/stores/tabs'
import { closeTabImmediately } from './tab-actions'

export async function openCollection(path: string): Promise<string> {
  const tree = await transport().invoke('open_collection', { path })
  useCollections.getState().setCollection(tree)
  // prefetch environment contents for decorations
  for (const name of tree.environments) {
    void transport()
      .invoke('read_environment', { id: tree.id, name })
      .then((env) => useEnvironments.getState().setEnv(tree.id, env))
      .catch(() => {})
  }
  return tree.id
}

export async function pickAndOpenCollection(): Promise<string | null> {
  const path = await transport().invoke('pick_collection_folder', {})
  if (!path) return null
  return openCollection(path)
}

export async function createCollection(parentDir: string, name: string): Promise<string> {
  const tree = await transport().invoke('create_collection', { parent_dir: parentDir, name })
  useCollections.getState().setCollection(tree)
  return tree.id
}

export async function createRequest(id: string, parentRel: string, name: string): Promise<string> {
  return transport().invoke('create_request', { id, parent_rel: parentRel, name })
}

export async function createFolder(id: string, parentRel: string, name: string): Promise<string> {
  return transport().invoke('create_folder', { id, parent_rel: parentRel, name })
}

export async function renameNode(
  id: string,
  rel: string,
  newName: string,
  kind: 'folder' | 'request',
): Promise<string> {
  const newRel = await transport().invoke('rename_node', { id, rel, new_name: newName, kind })
  if (newRel !== rel) {
    useCollections.getState().moveRequestKey(id, rel, newRel)
    useTabs.getState().rekey(id, rel, newRel)
  }
  if (kind === 'request') {
    // The backend rewrote meta.name INSIDE the file, so the in-memory mirror is
    // now stale (old name, old hash). Re-read it, or the first edit clones the
    // stale mirror and the next save silently reverts the rename.
    try {
      const { request, hash } = await transport().invoke('read_request', { id, rel: newRel })
      useCollections.getState().setRequest(id, newRel, request, hash)
      const tab = useTabs.getState().findByRel(id, newRel)
      if (tab) {
        useTabs.getState().setBase(tab.id, hash)
        // don't clobber a dirty draft's in-progress title
        if (tab.draft === null) useTabs.getState().retitle(tab.id, request.meta.name, request.http.method)
      }
    } catch {
      // fall back to the optimistic title if the re-read fails
      const tab = useTabs.getState().findByRel(id, newRel)
      if (tab) useTabs.getState().retitle(tab.id, newName, tab.method)
    }
  }
  return newRel
}

export async function moveNode(id: string, rel: string, newParentRel: string): Promise<string> {
  const newRel = await transport().invoke('move_node', { id, rel, new_parent_rel: newParentRel })
  useCollections.getState().moveRequestKey(id, rel, newRel)
  useTabs.getState().rekey(id, rel, newRel)
  return newRel
}

export async function reorderNodes(id: string, orderedRels: string[]): Promise<void> {
  await transport().invoke('reorder_nodes', { id, ordered_rels: orderedRels })
}

export async function deleteNode(id: string, rel: string): Promise<void> {
  await transport().invoke('delete_node', { id, rel })
  useCollections.getState().dropRequest(id, rel)
  const tab = useTabs.getState().findByRel(id, rel)
  if (tab) {
    if (tab.draft) useTabs.getState().setConflict(tab.id, 'file-deleted')
    else useTabs.getState().close(tab.id)
  }
}

export async function duplicateRequest(id: string, rel: string): Promise<string> {
  return transport().invoke('duplicate_request', { id, rel })
}

// ---------------------------------------------------------------------------
// watcher reconciliation
// ---------------------------------------------------------------------------

/** Deep equality ignoring meta.seq (rule 4). */
export function equalsIgnoringSeq(a: RequestFileDto, b: RequestFileDto): boolean {
  const strip = (r: RequestFileDto) => ({ ...r, meta: { ...r.meta, seq: undefined } })
  return JSON.stringify(strip(a)) === JSON.stringify(strip(b))
}

export function reconcileFileChanged(
  id: string,
  rel: string,
  hash: string,
  incoming: RequestFileDto | undefined,
): 'clean' | 'echo' | 'seq-only' | 'conflict' | 'untracked' {
  const collections = useCollections.getState()
  const tabs = useTabs.getState()

  // capture the previous mirror BEFORE overwriting it (rule 4 compares against it)
  const prev = collections.requests[requestKey(id, rel)]?.request

  // rule 1: the disk mirror always reflects reality
  if (incoming) collections.setRequest(id, rel, incoming, hash)

  const tab = tabs.findByRel(id, rel)
  if (!tab) return 'untracked'

  // rule 2: clean tab just follows the disk
  if (tab.draft === null) {
    tabs.setBase(tab.id, hash)
    if (incoming) tabs.retitle(tab.id, incoming.meta.name, incoming.http.method)
    return 'clean'
  }

  // rule 3: incoming equals the draft — our own save echo (or identical edit)
  if (incoming && equalsIgnoringSeq(tab.draft, incoming)) {
    tabs.setDraft(tab.id, null)
    tabs.setBase(tab.id, hash)
    tabs.setConflict(tab.id, 'none')
    return 'echo'
  }

  // rule 4: only seq changed on disk (a reorder) — merge it into the draft
  if (incoming && prev && equalsIgnoringSeq(prev, incoming) && prev.meta.seq !== incoming.meta.seq) {
    tabs.setDraft(tab.id, { ...tab.draft, meta: { ...tab.draft.meta, seq: incoming.meta.seq } })
    tabs.setBase(tab.id, hash)
    return 'seq-only'
  }

  // rule 5: genuine conflict — never clobber typing
  tabs.setConflict(tab.id, 'disk-changed')
  return 'conflict'
}

/**
 * A structural change (git pull, external rename/delete, an atomic-save editor)
 * emits only `tree-changed`, never per-file `file-changed`. Reconcile open tabs
 * against the fresh tree: files that vanished become deleted-conflicts (or close
 * when clean), and files still present are re-read so a rename-style external
 * edit doesn't leave a tab showing stale content forever.
 */
export async function reconcileTabsWithTree(
  id: string,
  nodes: TreeNodeDto[],
  invalidRels: Set<string>,
): Promise<void> {
  const present = new Set<string>()
  walkTree(nodes, (n) => {
    if (n.kind === 'request') present.add(n.rel)
  })
  for (const rel of invalidRels) present.add(rel) // a broken file still exists

  const tabs = useTabs.getState().tabs.filter((tab) => tab.collectionId === id)
  for (const tab of tabs) {
    if (!present.has(tab.rel)) {
      if (tab.draft) useTabs.getState().setConflict(tab.id, 'file-deleted')
      else closeTabImmediately(tab.id)
      continue
    }
    if (invalidRels.has(tab.rel)) continue // can't re-read an unparseable file
    try {
      const { request, hash } = await transport().invoke('read_request', { id, rel: tab.rel })
      const mirror = useCollections.getState().requests[requestKey(id, tab.rel)]
      if (mirror && mirror.hash === hash) continue // disk unchanged — nothing to do
      reconcileFileChanged(id, tab.rel, hash, request)
    } catch {
      // a transient read failure will be retried on the next tree-changed
    }
  }
}

/** Wire transport events into the stores. Call once at bootstrap. */
export function bootTransportListeners(): () => void {
  const t = transport()
  const un1 = t.listen('watcher:tree-changed', ({ tree }) => {
    useCollections.getState().setCollection(tree)
    const invalidRels = new Set((tree.invalid ?? []).map((entry) => entry.rel))
    void reconcileTabsWithTree(tree.id, tree.nodes, invalidRels)
  })
  const un2 = t.listen('watcher:file-changed', ({ id, rel, hash, request }) => {
    reconcileFileChanged(id, rel, hash, request)
  })
  return () => {
    un1()
    un2()
  }
}
