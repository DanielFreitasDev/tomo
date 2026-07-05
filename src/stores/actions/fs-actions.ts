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
import { type RequestFileDto, transport } from '@/lib/transport'
import { requestKey, useCollections } from '@/stores/collections'
import { useEnvironments } from '@/stores/environments'
import { useTabs } from '@/stores/tabs'

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
  const tab = useTabs.getState().findByRel(id, newRel)
  if (tab && kind === 'request') useTabs.getState().retitle(tab.id, newName, tab.method)
  return newRel
}

export async function moveNode(id: string, rel: string, newParentRel: string): Promise<string> {
  const newRel = await transport().invoke('move_node', { id, rel, new_parent_rel: newParentRel })
  useCollections.getState().moveRequestKey(id, rel, newRel)
  useTabs.getState().rekey(id, rel, newRel)
  return newRel
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

/** Wire transport events into the stores. Call once at bootstrap. */
export function bootTransportListeners(): () => void {
  const t = transport()
  const un1 = t.listen('watcher:tree-changed', ({ tree }) => {
    useCollections.getState().setCollection(tree)
  })
  const un2 = t.listen('watcher:file-changed', ({ id, rel, hash, request }) => {
    reconcileFileChanged(id, rel, hash, request)
  })
  return () => {
    un1()
    un2()
  }
}
