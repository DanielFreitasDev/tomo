import { beforeEach, describe, expect, it } from 'vitest'
import type { RequestFileDto } from '@/lib/transport'
import { setTransport } from '@/lib/transport'
import { createMockTransport } from '@/lib/transport/mock'
import { openCollection, reconcileFileChanged } from './actions/fs-actions'
import { editTab, openRequestTab } from './actions/tab-actions'
import { useCollections } from './collections'
import { useTabs } from './tabs'

const ID = '/mock/acme-api'
const REL = 'users/create-user.toml'

function diskRequest(): RequestFileDto {
  const mirror = useCollections.getState().requests[`${ID} ${REL}`]
  if (!mirror) throw new Error('mirror missing')
  return structuredClone(mirror.request)
}

describe('watcher reconciliation matrix', () => {
  let tabId: string

  beforeEach(async () => {
    setTransport(createMockTransport())
    useTabs.setState({ tabs: [], activeId: null, recentlyClosed: [] })
    useCollections.setState({ order: [], byId: {}, trees: {}, requests: {} })
    await openCollection(ID)
    tabId = await openRequestTab(ID, REL)
  })

  it('rule 2 — clean tab follows the disk', () => {
    const incoming = diskRequest()
    incoming.meta.name = 'Edited outside'
    const verdict = reconcileFileChanged(ID, REL, 'h2', incoming)
    expect(verdict).toBe('clean')
    const tab = useTabs.getState().byId(tabId)
    expect(tab?.title).toBe('Edited outside')
    expect(tab?.baseHash).toBe('h2')
    expect(tab?.conflict).toBe('none')
  })

  it('rule 3 — incoming equals draft: our save echo drops the draft', () => {
    editTab(tabId, (d) => {
      d.http.url = 'https://new.example'
      return d
    })
    const tab = useTabs.getState().byId(tabId)
    const incoming = structuredClone(tab?.draft)
    if (!incoming) throw new Error('draft missing')

    const verdict = reconcileFileChanged(ID, REL, 'h3', incoming)
    expect(verdict).toBe('echo')
    const after = useTabs.getState().byId(tabId)
    expect(after?.draft).toBeNull()
    expect(after?.baseHash).toBe('h3')
  })

  it('rule 4 — seq-only disk change merges into the draft, stays dirty', () => {
    editTab(tabId, (d) => {
      d.http.url = 'https://dirty.example'
      return d
    })
    const incoming = diskRequest() // content as on disk…
    incoming.meta.seq = 42 // …but reordered

    const verdict = reconcileFileChanged(ID, REL, 'h4', incoming)
    expect(verdict).toBe('seq-only')
    const tab = useTabs.getState().byId(tabId)
    expect(tab?.draft?.meta.seq).toBe(42)
    expect(tab?.draft?.http.url).toBe('https://dirty.example')
    expect(tab?.conflict).toBe('none')
  })

  it('rule 5 — real external change under a dirty tab marks conflict, keeps draft', () => {
    editTab(tabId, (d) => {
      d.http.url = 'https://mine.example'
      return d
    })
    const incoming = diskRequest()
    incoming.http.url = 'https://theirs.example'

    const verdict = reconcileFileChanged(ID, REL, 'h5', incoming)
    expect(verdict).toBe('conflict')
    const tab = useTabs.getState().byId(tabId)
    expect(tab?.conflict).toBe('disk-changed')
    expect(tab?.draft?.http.url).toBe('https://mine.example')
    // the mirror still reflects reality
    expect(useCollections.getState().requests[`${ID} ${REL}`]?.request.http.url).toBe(
      'https://theirs.example',
    )
  })

  it('untracked files just refresh the mirror', () => {
    const incoming = diskRequest()
    const verdict = reconcileFileChanged(ID, 'health.toml', 'h6', incoming)
    expect(verdict).toBe('untracked')
  })
})
