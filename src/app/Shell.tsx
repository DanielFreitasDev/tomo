/** App shell placeholder — the full layout (titlebar/sidebar/panes) lands in M10. */
import { BookOpenText } from 'lucide-react'
import { EmptyState } from '@/components/ui/empty-state'
import { useT } from '@/i18n'

export function Shell() {
  const t = useT()
  return (
    <div className="flex h-screen flex-col bg-app">
      <div className="flex h-9.5 shrink-0 items-center px-3 text-xs text-muted" data-tauri-drag-region>
        {t('app.name')}
      </div>
      <div className="min-h-0 flex-1 rounded-t-lg border border-subtle bg-surface">
        <EmptyState icon={BookOpenText} title={t('app.name')} hint={t('app.tagline')} />
      </div>
    </div>
  )
}
