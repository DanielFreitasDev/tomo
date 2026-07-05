import { ConfirmDialog } from '@/components/ui/dialog'
import { useT } from '@/i18n'
import { resolveClosePrompt } from '@/stores/actions/tab-actions'
import { type Tab, useTabs } from '@/stores/tabs'
import { useUi } from '@/stores/ui'

export function DirtyCloseDialog() {
  const t = useT()
  const prompt = useUi((s) => s.closePrompt)
  const setClosePrompt = useUi((s) => s.setClosePrompt)
  const tabs = useTabs((s) => s.tabs)

  const dirtyTabs = prompt
    ? prompt.tabIds.map((id) => tabs.find((tab) => tab.id === id)).filter(isDirtyTab)
    : []
  const first = dirtyTabs[0]
  const title =
    dirtyTabs.length > 1
      ? t('tabs.confirmCloseMany.title', { count: dirtyTabs.length })
      : t('tabs.confirmClose.title', { name: first?.title ?? '' })

  return (
    <ConfirmDialog
      open={Boolean(prompt)}
      onOpenChange={(open) => {
        if (!open) setClosePrompt(null)
      }}
      title={title}
      body={t('tabs.confirmClose.body')}
      confirmLabel={t('common.save')}
      cancelLabel={t('common.cancel')}
      extraAction={{
        label: t('common.discard'),
        onSelect: () => void resolveClosePrompt('discard'),
      }}
      onConfirm={() => void resolveClosePrompt('save')}
    />
  )
}

function isDirtyTab(tab: Tab | undefined): tab is Tab {
  return Boolean(tab?.draft)
}
