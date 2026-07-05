import { X } from 'lucide-react'
import { useState } from 'react'
import { ContextMenu } from '@/components/ui/menu'
import { MethodBadge } from '@/components/ui/method-badge'
import { useT } from '@/i18n'
import { cn } from '@/lib/cn'
import { closeTab } from '@/stores/actions/tab-actions'
import { type Tab, useTabs } from '@/stores/tabs'

export function TabStrip() {
  const t = useT()
  const tabs = useTabs((s) => s.tabs)
  const activeId = useTabs((s) => s.activeId)
  const activate = useTabs((s) => s.activate)
  const promote = useTabs((s) => s.promote)
  const closeOthers = useTabs((s) => s.closeOthers)
  const closeAll = useTabs((s) => s.closeAll)
  const reorder = useTabs((s) => s.reorder)
  const [dragIndex, setDragIndex] = useState<number | null>(null)

  if (tabs.length === 0) return <div className="h-full min-w-0 flex-1" data-tauri-drag-region />

  return (
    <div
      className="flex h-full min-w-0 flex-1 items-end gap-0.5 overflow-x-auto px-1"
      role="tablist"
      aria-label="Open requests"
    >
      {tabs.map((tab, index) => (
        <TabItem
          key={tab.id}
          tab={tab}
          active={tab.id === activeId}
          onActivate={() => activate(tab.id)}
          onPromote={() => promote(tab.id)}
          onClose={() => closeTab(tab.id)}
          onCloseOthers={() => closeOthers(tab.id)}
          onCloseAll={closeAll}
          labels={{
            close: t('tabs.closeTab'),
            closeOthers: t('tabs.closeOthers'),
            closeAll: t('tabs.closeAll'),
          }}
          onDragStart={() => setDragIndex(index)}
          onDragOverItem={(e) => {
            e.preventDefault()
            if (dragIndex !== null && dragIndex !== index) {
              reorder(dragIndex, index)
              setDragIndex(index)
            }
          }}
          onDragEnd={() => setDragIndex(null)}
        />
      ))}
      <div className="h-full min-w-6 flex-1" data-tauri-drag-region />
    </div>
  )
}

function TabItem({
  tab,
  active,
  onActivate,
  onPromote,
  onClose,
  onCloseOthers,
  onCloseAll,
  labels,
  onDragStart,
  onDragOverItem,
  onDragEnd,
}: {
  tab: Tab
  active: boolean
  onActivate: () => void
  onPromote: () => void
  onClose: () => void
  onCloseOthers: () => void
  onCloseAll: () => void
  labels: { close: string; closeOthers: string; closeAll: string }
  onDragStart: () => void
  onDragOverItem: (e: React.DragEvent) => void
  onDragEnd: () => void
}) {
  const dirty = tab.draft !== null
  return (
    <ContextMenu
      entries={[
        { label: labels.close, kbd: 'Ctrl+W', onSelect: onClose },
        { label: labels.closeOthers, onSelect: onCloseOthers },
        { label: labels.closeAll, kbd: 'Ctrl+Shift+W', onSelect: onCloseAll },
      ]}
    >
      <div
        role="tab"
        aria-selected={active}
        tabIndex={active ? 0 : -1}
        draggable
        onDragStart={onDragStart}
        onDragOver={onDragOverItem}
        onDragEnd={onDragEnd}
        onClick={onActivate}
        onDoubleClick={onPromote}
        onAuxClick={(e) => {
          if (e.button === 1) onClose()
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onActivate()
        }}
        className={cn(
          'group flex h-8 min-w-0 max-w-48 shrink-0 cursor-default select-none items-center gap-1.5 rounded-t-lg border border-b-0 px-2.5',
          'transition-colors duration-(--dur-fast)',
          active
            ? 'border-subtle bg-surface text-primary'
            : 'border-transparent bg-transparent text-secondary hover:bg-hover hover:text-primary',
        )}
      >
        <MethodBadge method={tab.method} className="w-auto" />
        <span className={cn('truncate text-xs', tab.preview && 'italic')}>{tab.title}</span>
        {tab.conflict !== 'none' ? (
          <span
            role="status"
            className="size-1.5 shrink-0 rounded-full bg-(--warning)"
            aria-label="conflict"
          />
        ) : dirty ? (
          <span role="status" className="size-1.5 shrink-0 rounded-full bg-(--accent)" aria-label="unsaved" />
        ) : null}
        <button
          type="button"
          aria-label={labels.close}
          onClick={(e) => {
            e.stopPropagation()
            onClose()
          }}
          className={cn(
            'shrink-0 rounded-sm p-0.5 text-muted opacity-0 transition-opacity duration-(--dur-fast)',
            'hover:bg-active hover:text-primary group-hover:opacity-100',
            active && 'opacity-100',
          )}
        >
          <X size={11} />
        </button>
      </div>
    </ContextMenu>
  )
}
