import { beforeEach, describe, expect, it } from 'vitest'
import { setTransport } from '@/lib/transport'
import { createMockTransport } from '@/lib/transport/mock'
import { openCollection } from './actions/fs-actions'
import { editTab, openRequestTab, saveTab, tabContent } from './actions/tab-actions'
import { useCollections } from './collections'
import { useTabs } from './tabs'

async function freshWorkspace(): Promise<string> {
  setTransport(createMockTransport())
  useTabs.setState({ tabs: [], activeId: null, recentlyClosed: [] })
  useCollections.setState({ order: [], byId: {}, trees: {}, requests: {} })
  return openCollection('/mock/acme-api')
}

describe('tab lifecycle', () => {
  beforeEach(async () => {
    await freshWorkspace()
  })

  it('opens a preview tab, promotes on edit, saves clean', async () => {
    const id = await openRequestTab('/mock/acme-api', 'users/create-user.toml', { preview: true })
    let tab = useTabs.getState().byId(id)
    expect(tab?.preview).toBe(true)
    expect(tab?.draft).toBeNull()
    expect(tab?.title).toBe('Create user')

    // first edit: copy-on-write + promote
    editTab(id, (draft) => {
      draft.http.url = 'https://api.acme.test/v2/users'
      return draft
    })
    tab = useTabs.getState().byId(id)
    expect(tab?.draft?.http.url).toBe('https://api.acme.test/v2/users')
    expect(tab?.preview).toBe(false)

    // the disk mirror is untouched while dirty
    const mirror = useCollections.getState().requests['/mock/acme-api users/create-user.toml']
    expect(mirror?.request.http.url).toBe('{{base_url}}/anything/users')

    // save: draft becomes the mirror, tab is clean again
    const outcome = await saveTab(id)
    expect(outcome).toBe('saved')
    tab = useTabs.getState().byId(id)
    expect(tab?.draft).toBeNull()
    const after = useCollections.getState().requests['/mock/acme-api users/create-user.toml']
    expect(after?.request.http.url).toBe('https://api.acme.test/v2/users')
  })

  it('a second open of the same rel focuses the existing tab', async () => {
    const a = await openRequestTab('/mock/acme-api', 'health.toml')
    const b = await openRequestTab('/mock/acme-api', 'health.toml')
    expect(a).toBe(b)
    expect(useTabs.getState().tabs).toHaveLength(1)
  })

  it('preview tabs are replaced by the next preview; permanent ones stay', async () => {
    await openRequestTab('/mock/acme-api', 'health.toml', { preview: true })
    await openRequestTab('/mock/acme-api', 'users/list-users.toml', { preview: true })
    const tabs = useTabs.getState().tabs
    expect(tabs).toHaveLength(1)
    expect(tabs[0]?.rel).toBe('users/list-users.toml')

    useTabs.getState().promote(tabs[0]?.id ?? '')
    await openRequestTab('/mock/acme-api', 'health.toml', { preview: true })
    expect(useTabs.getState().tabs).toHaveLength(2)
  })

  it('close records recently-closed; save with stale base reports conflict', async () => {
    const id = await openRequestTab('/mock/acme-api', 'health.toml')
    editTab(id, (d) => {
      d.meta.name = 'Renamed'
      return d
    })

    // simulate an external save bumping the hash
    const t = createMockTransport()
    setTransport(t)
    await t.invoke('open_collection', { path: '/mock/acme-api' })
    // stale base hash on our side -> conflict
    useTabs.getState().setBase(id, 'stale-hash')
    const outcome = await saveTab(id)
    expect(outcome).toBe('conflict')
    expect(useTabs.getState().byId(id)?.conflict).toBe('disk-changed')

    useTabs.getState().close(id)
    expect(useTabs.getState().tabs).toHaveLength(0)
    expect(useTabs.getState().recentlyClosed[0]?.rel).toBe('health.toml')
  })

  it('tabContent returns the draft when dirty, the mirror when clean', async () => {
    const id = await openRequestTab('/mock/acme-api', 'health.toml')
    const tab = () => {
      const t = useTabs.getState().byId(id)
      if (!t) throw new Error('tab gone')
      return t
    }
    expect(tabContent(tab())?.meta.name).toBe('Health check')
    editTab(id, (d) => {
      d.meta.name = 'Edited'
      return d
    })
    expect(tabContent(tab())?.meta.name).toBe('Edited')
  })

  it('preserves custom HTTP methods exactly as edited', async () => {
    const id = await openRequestTab('/mock/acme-api', 'health.toml')
    editTab(id, (d) => {
      d.http.method = 'PROPFIND'
      return d
    })

    const tab = useTabs.getState().byId(id)
    if (!tab) throw new Error('tab gone')
    expect(tabContent(tab)?.http.method).toBe('PROPFIND')
  })
})
