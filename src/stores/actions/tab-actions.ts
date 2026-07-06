/** Tab lifecycle: open (preview/permanent), edit (copy-on-write), save, close. */
import { type RequestFileDto, transport } from '@/lib/transport'
import { requestKey, useCollections } from '@/stores/collections'
import { useResponses } from '@/stores/responses'
import { type Tab, useTabs } from '@/stores/tabs'
import { useUi } from '@/stores/ui'

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
  const tab = useTabs.getState().byId(tabId)
  if (!tab?.draft) return 'noop'
  const snapshot = tab.draft

  const result = await transport().invoke('save_request', {
    id: tab.collectionId,
    rel: tab.rel,
    request: snapshot,
    base_hash: tab.baseHash,
  })

  if (result.outcome === 'conflict') {
    useTabs.getState().setConflict(tabId, 'disk-changed', result.current_hash)
    return 'conflict'
  }

  const hash = result.hash ?? ''
  // the mirror now reflects exactly what we wrote
  useCollections.getState().setRequest(tab.collectionId, tab.rel, snapshot, hash)
  useTabs.getState().setBase(tabId, hash)
  useTabs.getState().setConflict(tabId, 'none')
  // only clear the draft if nothing was typed during the (possibly slow) await;
  // otherwise the newer keystrokes stay dirty instead of being discarded
  const latest = useTabs.getState().byId(tabId)
  if (latest?.draft && JSON.stringify(latest.draft) === JSON.stringify(snapshot)) {
    useTabs.getState().setDraft(tabId, null)
  }
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
  // Rebase onto the disk hash that actually triggered the conflict so the next
  // save wins. Falling back to the mirror hash (which can be stale when the
  // conflict came from a save, not a watcher event) caused an endless
  // conflict → keep-mine → conflict loop.
  const diskHash =
    tab.conflictHash ?? useCollections.getState().requests[requestKey(tab.collectionId, tab.rel)]?.hash
  if (diskHash) tabs.setBase(tabId, diskHash)
  tabs.setConflict(tabId, 'none')
}

let allowWindowClose = false

function closeTabsNow(tabIds: string[]): void {
  for (const tabId of tabIds) {
    if (!useTabs.getState().byId(tabId)) continue
    useResponses.getState().clear(tabId)
    useTabs.getState().close(tabId)
  }
}

function requestCloseTabs(tabIds: string[], after?: 'window'): void {
  const tabs = useTabs.getState()
  const existing = tabIds.filter((id) => tabs.byId(id))
  if (existing.length === 0) {
    if (after === 'window') void closeWindowNow()
    return
  }
  const hasDirty = existing.some((id) => tabs.byId(id)?.draft !== null)
  if (hasDirty) {
    useUi.getState().setClosePrompt({ tabIds: existing, after })
    return
  }
  closeTabsNow(existing)
  if (after === 'window') void closeWindowNow()
}

async function closeWindowNow(): Promise<void> {
  allowWindowClose = true
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close()
}

export function shouldAllowWindowClose(): boolean {
  return allowWindowClose
}

export function closeTab(tabId: string): void {
  requestCloseTabs([tabId])
}

export function closeOtherTabs(tabId: string): void {
  const ids = useTabs
    .getState()
    .tabs.filter((tab) => tab.id !== tabId)
    .map((tab) => tab.id)
  requestCloseTabs(ids)
}

export function closeAllTabs(): void {
  requestCloseTabs(useTabs.getState().tabs.map((tab) => tab.id))
}

export function requestCloseWindow(): void {
  requestCloseTabs(
    useTabs.getState().tabs.map((tab) => tab.id),
    'window',
  )
}

export async function resolveClosePrompt(action: 'save' | 'discard' | 'cancel'): Promise<void> {
  const prompt = useUi.getState().closePrompt
  if (!prompt) return
  if (action === 'cancel') {
    useUi.getState().setClosePrompt(null)
    return
  }
  if (action === 'save') {
    for (const tabId of prompt.tabIds) {
      const tab = useTabs.getState().byId(tabId)
      if (!tab?.draft) continue
      const outcome = await saveTab(tabId)
      if (outcome === 'conflict') {
        useUi.getState().setClosePrompt(null)
        return
      }
    }
  }
  useUi.getState().setClosePrompt(null)
  closeTabsNow(prompt.tabIds)
  if (prompt.after === 'window') await closeWindowNow()
}

export function closeTabImmediately(tabId: string): void {
  useResponses.getState().clear(tabId)
  useTabs.getState().close(tabId)
}

export async function reopenLastClosed(): Promise<void> {
  const closed = useTabs.getState().popRecentlyClosed()
  if (closed) await openRequestTab(closed.collectionId, closed.rel)
}
