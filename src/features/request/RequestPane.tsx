import { Send, Square, TriangleAlert } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CodeEditor } from '@/components/ui/code-editor'
import { MethodBadge } from '@/components/ui/method-badge'
import { Select } from '@/components/ui/select'
import { TabPanel, UnderlineTabs } from '@/components/ui/tabs'
import { useT } from '@/i18n'
import { cancelActiveRequest, sendActiveRequest } from '@/stores/actions/request-actions'
import { editTab, keepMyChanges, reloadTabFromDisk, tabContent } from '@/stores/actions/tab-actions'
import { useCollections } from '@/stores/collections'
import { useResponses } from '@/stores/responses'
import { type SubTab, type Tab, useTabs } from '@/stores/tabs'
import { AuthTab, BodyTab, DocsTab, HeadersTab, OptionsTab, ParamsTab, ScriptsTab, TestsTab } from './tabs'

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const

export function RequestPane({ tab }: { tab: Tab }) {
  const t = useT()
  useCollections((s) => s.requests[`${tab.collectionId} ${tab.rel}`])
  const request = tabContent(tab)
  const phase = useResponses((s) => (s.byTab[tab.id] ?? { phase: 'idle' }).phase)
  const setSubTab = useTabs((s) => s.setSubTab)

  if (!request) return null
  const sending = phase === 'sending'

  const count = {
    params: (request.http.query?.length ?? 0) + (request.http.path?.length ?? 0),
    headers: request.http.headers?.length ?? 0,
    tests: request.tests?.asserts?.length ?? 0,
  }
  const badge = (n: number) => (n > 0 ? <Badge tone="accent">{n}</Badge> : undefined)

  return (
    <div className="flex h-full min-w-0 flex-col">
      {tab.conflict !== 'none' ? (
        <div className="flex shrink-0 items-center gap-2 border-b border-(--warning) bg-warning-soft px-3 py-1.5 text-xs text-primary">
          <TriangleAlert size={13} style={{ color: 'var(--warning)' }} />
          <span className="min-w-0 flex-1 truncate">
            {tab.conflict === 'file-deleted'
              ? `${tab.rel} was deleted on disk`
              : t('toast.fileChangedOnDisk', { name: tab.rel })}
          </span>
          <Button size="sm" variant="secondary" onClick={() => keepMyChanges(tab.id)}>
            {t('toast.keepMine')}
          </Button>
          {tab.conflict === 'disk-changed' ? (
            <Button size="sm" variant="secondary" onClick={() => void reloadTabFromDisk(tab.id)}>
              {t('toast.reloadFromDisk')}
            </Button>
          ) : null}
        </div>
      ) : null}

      <div className="flex shrink-0 items-center gap-1.5 p-2">
        <Select
          ariaLabel="Method"
          value={(METHODS as readonly string[]).includes(request.http.method) ? request.http.method : 'GET'}
          onChange={(m) => editTab(tab.id, (d) => ({ ...d, http: { ...d.http, method: m } }))}
          options={METHODS.map((m) => ({ value: m, label: <MethodBadge method={m} block /> }))}
        />
        <div className="flex h-9 min-w-0 flex-1 items-center rounded-md border border-default bg-raised px-2 transition-colors focus-within:border-(--accent) focus-within:ring-2 focus-within:ring-(--accent-soft)">
          <CodeEditor
            singleLine
            className="w-full"
            collectionId={tab.collectionId}
            value={request.http.url}
            onChange={(url) => editTab(tab.id, (d) => ({ ...d, http: { ...d.http, url } }))}
            onEnter={() => void sendActiveRequest(tab.id)}
            placeholder={t('request.url.placeholder')}
            ariaLabel="Request URL"
          />
        </div>
        {sending ? (
          <Button
            variant="secondary"
            icon={<Square size={12} />}
            onClick={() => void cancelActiveRequest(tab.id)}
          >
            {t('request.cancel')}
          </Button>
        ) : (
          <Button variant="primary" icon={<Send size={13} />} onClick={() => void sendActiveRequest(tab.id)}>
            {t('request.send')}
          </Button>
        )}
      </div>

      <UnderlineTabs<SubTab>
        className="min-h-0 flex-1"
        value={tab.activeSubTab}
        onChange={(v) => setSubTab(tab.id, v)}
        tabs={[
          { value: 'params', label: t('request.tab.params'), badge: badge(count.params) },
          { value: 'headers', label: t('request.tab.headers'), badge: badge(count.headers) },
          {
            value: 'body',
            label: t('request.tab.body'),
            badge: request.body ? <Badge tone="accent">●</Badge> : undefined,
          },
          { value: 'auth', label: t('request.tab.auth') },
          { value: 'scripts', label: t('request.tab.scripts') },
          { value: 'tests', label: t('request.tab.tests'), badge: badge(count.tests) },
          { value: 'docs', label: t('request.tab.docs') },
          { value: 'settings', label: t('request.tab.settings') },
        ]}
      >
        <TabPanel value="params">
          <ParamsTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="headers">
          <HeadersTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="body">
          <BodyTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="auth">
          <AuthTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="scripts">
          <ScriptsTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="tests">
          <TestsTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="docs">
          <DocsTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
        <TabPanel value="settings">
          <OptionsTab tabId={tab.id} collectionId={tab.collectionId} request={request} />
        </TabPanel>
      </UnderlineTabs>
    </div>
  )
}
