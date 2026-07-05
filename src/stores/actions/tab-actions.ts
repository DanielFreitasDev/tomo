/** Tab lifecycle: open (preview/permanent), edit (copy-on-write), save, close. */
import { type RequestFileDto, transport } from '@/lib/transport'
import { requestKey, useCollections } from '@/stores/collections'
import { useResponses } from '@/stores/responses'
import { type Tab, useTabs } from '@/stores/tabs'

export async function openRequestTab(
  collectionId: string,
  rel: string,
  opts: { preview?: boolean } = {},
): Promise<string> {
  const tabs = useTabs.getState()
  const existing = tabs.findByRel(collectionId, rel)
  if (existing) {
    tabs.activate(existing.id)
    if (!opts.preview) tabs.promote(existing.id)
    return existing.id
  }

  const { request, hash } = await transport().invoke('read_request', { id: collectionId, rel })
  useCollections.getState().setRequest(collectionId, rel, request, hash)
  return tabs.open({
    collectionId,
    rel,
    title: request.meta.name,
    method: request.http.method,
    draft: null,
    baseHash: hash,
    conflict: 'none',
    preview: opts.preview ?? false,
    activeSubTab: 'params',
  })
}

/** The request the tab currently shows: its draft or the disk mirror. */
export function tabContent(tab: Tab): RequestFileDto | undefined {
  if (tab.draft) return tab.draft
  return useCollections.getState().requests[requestKey(tab.collectionId, tab.rel)]?.request
}

/** First edit copies the mirror into a draft (copy-on-write). */
export function editTab(tabId: string, mutate: (draft: RequestFileDto) => RequestFileDto): void {
  const tabs = useTabs.getState()
  const tab = tabs.byId(tabId)
  if (!tab) return
  const base = tabContent(tab)
  if (!base) return
  const draft = mutate(structuredClone(tab.draft ?? base))
  tabs.setDraft(tabId, draft)
  tabs.retitle(tabId, draft.meta.name, draft.http.method)
}

export type SaveOutcome = 'saved' | 'conflict' | 'noop'

export async function saveTab(tabId: string): Promise<SaveOutcome> {
  const tabs = useTabs.getState()
  const tab = tabs.byId(tabId)
  if (!tab?.draft) return 'noop'

  const result = await transport().invoke('save_request', {
    id: tab.collectionId,
    rel: tab.rel,
    request: tab.draft,
    base_hash: tab.baseHash,
  })

  if (result.outcome === 'conflict') {
    tabs.setConflict(tabId, 'disk-changed')
    return 'conflict'
  }

  useCollections.getState().setRequest(tab.collectionId, tab.rel, tab.draft, result.hash ?? '')
  tabs.setDraft(tabId, null)
  tabs.setBase(tabId, result.hash ?? '')
  tabs.setConflict(tabId, 'none')
  return 'saved'
}

export async function saveAllTabs(): Promise<void> {
  const dirty = useTabs.getState().tabs.filter((t) => t.draft !== null)
  for (const tab of dirty) {
    await saveTab(tab.id)
  }
}

/** Resolve a disk conflict by discarding the draft and reloading. */
export async function reloadTabFromDisk(tabId: string): Promise<void> {
  const tabs = useTabs.getState()
  const tab = tabs.byId(tabId)
  if (!tab) return
  const { request, hash } = await transport().invoke('read_request', {
    id: tab.collectionId,
    rel: tab.rel,
  })
  useCollections.getState().setRequest(tab.collectionId, tab.rel, request, hash)
  tabs.setDraft(tabId, null)
  tabs.setBase(tabId, hash)
  tabs.setConflict(tabId, 'none')
  tabs.retitle(tabId, request.meta.name, request.http.method)
}

/** Keep my changes: clear the conflict flag; the next save overwrites disk. */
export function keepMyChanges(tabId: string): void {
  const tabs = useTabs.getState()
  const tab = tabs.byId(tabId)
  if (!tab) return
  // rebase onto current disk hash so the next save wins
  const mirror = useCollections.getState().requests[requestKey(tab.collectionId, tab.rel)]
  if (mirror) tabs.setBase(tabId, mirror.hash)
  tabs.setConflict(tabId, 'none')
}

export function closeTab(tabId: string): void {
  useResponses.getState().clear(tabId)
  useTabs.getState().close(tabId)
}

export async function reopenLastClosed(): Promise<void> {
  const closed = useTabs.getState().popRecentlyClosed()
  if (closed) await openRequestTab(closed.collectionId, closed.rel)
}
