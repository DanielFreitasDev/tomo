/** Send/cancel: the UI generates run ids so cancel can never race. */
import { transport } from '@/lib/transport'
import { useCollections } from '@/stores/collections'
import { useResponses } from '@/stores/responses'
import { useTabs } from '@/stores/tabs'
import { tabContent } from './tab-actions'

function newRunId(): string {
  return crypto.randomUUID()
}

export async function sendActiveRequest(tabId: string): Promise<void> {
  const tab = useTabs.getState().byId(tabId)
  if (!tab) return
  const responses = useResponses.getState()
  const current = responses.of(tabId)
  if (current.phase === 'sending') return

  const runId = newRunId()
  responses.start(tabId, runId)

  const info = useCollections.getState().byId[tab.collectionId]
  try {
    const meta = await transport().invoke('send_request', {
      id: tab.collectionId,
      rel: tab.rel,
      run_id: runId,
      draft: tab.draft ?? tabContent(tab),
      env: info?.selectedEnv,
    })
    const bytes = await transport().invoke('get_response_body', { run_id: runId })
    // ignore if a newer run replaced this one
    if (useResponses.getState().of(tabId).runId === runId) {
      useResponses.getState().complete(tabId, meta, bytes)
    }
  } catch (e) {
    if (useResponses.getState().of(tabId).runId !== runId) return
    const message = e instanceof Error ? e.message : String(e)
    if (message.includes('cancelled')) useResponses.getState().cancelledAt(tabId)
    else useResponses.getState().fail(tabId, message)
  }
}

export async function cancelActiveRequest(tabId: string): Promise<void> {
  const state = useResponses.getState().of(tabId)
  if (state.phase !== 'sending' || !state.runId) return
  await transport().invoke('cancel_request', { run_id: state.runId })
}
