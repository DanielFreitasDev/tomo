import { Send, Square } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { MethodBadge } from '@/components/ui/method-badge'
import { Select } from '@/components/ui/select'
import { useT } from '@/i18n'
import { cancelActiveRequest, sendActiveRequest } from '@/stores/actions/request-actions'
import { editTab, tabContent } from '@/stores/actions/tab-actions'
import { useCollections } from '@/stores/collections'
import { useResponses } from '@/stores/responses'
import type { Tab } from '@/stores/tabs'

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const

export function RequestPane({ tab }: { tab: Tab }) {
  const t = useT()
  // subscribe to mirror + draft so the pane re-renders on both
  useCollections((s) => s.requests[`${tab.collectionId} ${tab.rel}`])
  const request = tabContent(tab)
  const phase = useResponses((s) => (s.byTab[tab.id] ?? { phase: 'idle' }).phase)

  if (!request) return null
  const sending = phase === 'sending'

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex shrink-0 items-center gap-1.5 p-2">
        <Select
          ariaLabel="Method"
          value={request.http.method as (typeof METHODS)[number]}
          onChange={(m) => editTab(tab.id, (d) => ({ ...d, http: { ...d.http, method: m } }))}
          options={METHODS.map((m) => ({ value: m, label: <MethodBadge method={m} block /> }))}
        />
        <Input
          mono
          className="h-9 flex-1"
          placeholder={t('request.url.placeholder')}
          value={request.http.url}
          onChange={(e) => editTab(tab.id, (d) => ({ ...d, http: { ...d.http, url: e.target.value } }))}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void sendActiveRequest(tab.id)
          }}
          aria-label="Request URL"
        />
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
      {/* Params/Headers/Body/Auth/Scripts/Tests/Docs/Settings sub-tabs land in M11 */}
      <div className="flex min-h-0 flex-1 items-center justify-center text-xs text-muted">
        request editor · M11
      </div>
    </div>
  )
}
