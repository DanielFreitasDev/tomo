import { SendHorizonal } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { EmptyState } from '@/components/ui/empty-state'
import { Kbd } from '@/components/ui/kbd'
import { Spinner } from '@/components/ui/spinner'
import { StatusPill } from '@/components/ui/status-pill'
import { useT } from '@/i18n'
import { cancelActiveRequest } from '@/stores/actions/request-actions'
import { useResponses } from '@/stores/responses'
import type { Tab } from '@/stores/tabs'

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}

const IDLE_STATE = { phase: 'idle' } as const

export function ResponsePane({ tab }: { tab: Tab }) {
  const t = useT()
  const state = useResponses((s) => s.byTab[tab.id] ?? (IDLE_STATE as never))

  if (state.phase === 'idle') {
    return (
      <EmptyState
        icon={SendHorizonal}
        title={t('response.empty.title')}
        hint={
          <span className="inline-flex items-center gap-1">
            <Kbd>Ctrl</Kbd>+<Kbd>⏎</Kbd> · {t('response.empty.hint')}
          </span>
        }
      />
    )
  }

  if (state.phase === 'sending') {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3">
        <Spinner size={20} className="text-accent-text" />
        <div className="text-xs text-muted" data-tabular>
          {t('response.sending')}
        </div>
        <Button variant="secondary" size="sm" onClick={() => void cancelActiveRequest(tab.id)}>
          {t('request.cancel')}
        </Button>
      </div>
    )
  }

  if (state.phase === 'cancelled') {
    return <EmptyState title={t('response.cancelled')} />
  }

  if (state.phase === 'error') {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
        <div className="text-sm font-medium text-(--danger)">{t('toast.error')}</div>
        <div className="max-w-96 font-mono text-xs text-secondary" data-selectable>
          {state.error}
        </div>
      </div>
    )
  }

  const meta = state.meta
  if (!meta) return null
  const bodyText = state.bodyBytes ? new TextDecoder().decode(state.bodyBytes) : ''

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex shrink-0 items-center gap-3 border-b border-subtle px-3 py-2">
        <StatusPill status={meta.status} statusText={meta.status_text} />
        <span className="text-xs text-muted" data-tabular>
          {meta.timing.total_ms} ms
        </span>
        <span className="text-xs text-muted" data-tabular>
          {formatBytes(meta.body.total_size)}
        </span>
      </div>
      {/* Pretty/Raw/Preview + Headers/Cookies/Tests land in M12 */}
      <pre
        data-selectable
        className="min-h-0 flex-1 overflow-auto p-3 font-mono text-xs leading-5 text-primary"
      >
        {bodyText}
      </pre>
    </div>
  )
}
