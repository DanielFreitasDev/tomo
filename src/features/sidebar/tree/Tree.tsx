// biome-ignore-all lint/a11y/useKeyWithClickEvents: keyboard nav is roving-tabindex at the tree container
/**
 * Custom virtualized collection tree: @tanstack/react-virtual rows, roving
 * tabindex keyboard nav, inline rename (F2), context menus, HTML5 drag &
 * drop (into folders and between siblings).
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { ChevronRight, FilePlus2, Folder as FolderIcon, FolderPlus } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { ConfirmDialog } from '@/components/ui/dialog'
import { ContextMenu, type MenuEntry } from '@/components/ui/menu'
import { MethodBadge } from '@/components/ui/method-badge'
import { toast } from '@/components/ui/toast'
import { useT } from '@/i18n'
import { cn } from '@/lib/cn'
import type { TreeNodeDto } from '@/lib/transport'
import {
  createFolder,
  createRequest,
  deleteNode,
  duplicateRequest,
  moveNode,
  renameNode,
  reorderNodes,
} from '@/stores/actions/fs-actions'
import { openRequestTab } from '@/stores/actions/tab-actions'
import { useTabs } from '@/stores/tabs'
import { useUi } from '@/stores/ui'
import { flattenVisible, type VisibleRow } from './flatten'
import { orderedRelsAfterDrop, parentRelOf } from './reorder'

const ROW_HEIGHT = 28

export function Tree({ collectionId, nodes }: { collectionId: string; nodes: TreeNodeDto[] }) {
  const t = useT()
  const expanded = useUi((s) => s.expanded)
  const filter = useUi((s) => s.filter)
  const toggleExpanded = useUi((s) => s.toggleExpanded)
  const activeTab = useTabs((s) => s.tabs.find((tab) => tab.id === s.activeId))
  const allTabs = useTabs((s) => s.tabs)
  const dirtyRels = useMemo(
    () => new Set(allTabs.filter((tab) => tab.draft !== null).map((tab) => tab.rel)),
    [allTabs],
  )

  const rows = useMemo(() => flattenVisible(nodes, expanded, filter), [nodes, expanded, filter])

  const scrollRef = useRef<HTMLDivElement>(null)
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  })

  const [focusedIndex, setFocusedIndex] = useState(0)
  const [renaming, setRenaming] = useState<string | null>(null)
  const [dropTarget, setDropTarget] = useState<{ rel: string; mode: 'into' | 'after' } | null>(null)
  const [pendingDelete, setPendingDelete] = useState<VisibleRow | null>(null)

  useEffect(() => {
    if (focusedIndex >= rows.length) setFocusedIndex(Math.max(0, rows.length - 1))
  }, [rows.length, focusedIndex])

  const openRow = (row: VisibleRow, preview: boolean) => {
    if (row.kind === 'folder') toggleExpanded(row.rel)
    else void openRequestTab(collectionId, row.rel, { preview })
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    const row = rows[focusedIndex]
    if (!row) return
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setFocusedIndex((i) => Math.min(i + 1, rows.length - 1))
        break
      case 'ArrowUp':
        e.preventDefault()
        setFocusedIndex((i) => Math.max(i - 1, 0))
        break
      case 'ArrowRight':
        e.preventDefault()
        if (row.kind === 'folder' && !row.isExpanded) toggleExpanded(row.rel)
        else setFocusedIndex((i) => Math.min(i + 1, rows.length - 1))
        break
      case 'ArrowLeft':
        e.preventDefault()
        if (row.kind === 'folder' && row.isExpanded) toggleExpanded(row.rel)
        else {
          const parentIdx = rows.findIndex((r) => r.rel === row.parentRel)
          if (parentIdx >= 0) setFocusedIndex(parentIdx)
        }
        break
      case 'Enter':
        e.preventDefault()
        openRow(row, false)
        break
      case ' ':
        e.preventDefault()
        openRow(row, true)
        break
      case 'F2':
        e.preventDefault()
        setRenaming(row.rel)
        break
      case 'Delete':
        e.preventDefault()
        setPendingDelete(row)
        break
      default:
        break
    }
  }

  const doDelete = async (row: VisibleRow) => {
    setPendingDelete(null)
    try {
      await deleteNode(collectionId, row.rel)
      toast.success(t('toast.deleted'), row.name)
    } catch (err) {
      toast.danger(t('toast.error'), err instanceof Error ? err.message : row.name)
    }
  }

  const entriesFor = (row: VisibleRow): MenuEntry[] => {
    const common: MenuEntry[] = [
      { label: t('common.rename'), kbd: 'F2', onSelect: () => setRenaming(row.rel) },
      'separator',
      { label: t('common.delete'), danger: true, onSelect: () => setPendingDelete(row) },
    ]
    if (row.kind === 'folder') {
      return [
        {
          label: t('sidebar.newRequest'),
          icon: <FilePlus2 size={13} />,
          onSelect: () => void createRequest(collectionId, row.rel, 'New request'),
        },
        {
          label: t('sidebar.newFolder'),
          icon: <FolderPlus size={13} />,
          onSelect: () => void createFolder(collectionId, row.rel, 'New folder'),
        },
        'separator',
        ...common,
      ]
    }
    return [
      {
        label: t('common.duplicate'),
        kbd: 'Ctrl+D',
        onSelect: () => void duplicateRequest(collectionId, row.rel),
      },
      ...common,
    ]
  }

  // ---- HTML5 drag & drop --------------------------------------------------
  const onDragStart = (e: React.DragEvent, row: VisibleRow) => {
    e.dataTransfer.setData('application/x-tomo-rel', row.rel)
    e.dataTransfer.effectAllowed = 'move'
  }
  const onDragOver = (e: React.DragEvent, row: VisibleRow) => {
    e.preventDefault()
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const intoFolder = row.kind === 'folder' && e.clientY < rect.bottom - 8
    setDropTarget({ rel: row.rel, mode: intoFolder ? 'into' : 'after' })
  }
  const onDrop = async (e: React.DragEvent, row: VisibleRow) => {
    e.preventDefault()
    const sourceRel = e.dataTransfer.getData('application/x-tomo-rel')
    const mode = dropTarget?.rel === row.rel ? dropTarget.mode : 'after'
    setDropTarget(null)
    if (!sourceRel || sourceRel === row.rel) return
    const targetParent = row.kind === 'folder' && mode === 'into' ? row.rel : row.parentRel
    const sourceParent = parentRelOf(sourceRel)
    try {
      let movedRel = sourceRel
      if (targetParent !== sourceParent) {
        movedRel = await moveNode(collectionId, sourceRel, targetParent)
      }
      const orderedRels = orderedRelsAfterDrop(nodes, sourceRel, movedRel, row.rel, mode)
      if (orderedRels.length > 0) await reorderNodes(collectionId, orderedRels)
    } catch (err) {
      toast.danger(t('toast.error'), err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <>
      <div
        ref={scrollRef}
        className="min-h-0 flex-1 overflow-y-auto px-1.5 pb-2"
        onKeyDown={onKeyDown}
        role="tree"
        aria-label={t('sidebar.collections')}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((v) => {
            const row = rows[v.index]
            if (!row) return null
            const isActive = activeTab?.rel === row.rel && activeTab?.collectionId === collectionId
            const isFocused = v.index === focusedIndex
            const isDirty = dirtyRels.has(row.rel)
            const isDrop = dropTarget?.rel === row.rel

            return (
              <div
                key={row.rel}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: v.size,
                  transform: `translateY(${v.start}px)`,
                }}
              >
                <ContextMenu entries={entriesFor(row)}>
                  <div
                    role="treeitem"
                    aria-level={row.depth + 1}
                    aria-posinset={row.posinset}
                    aria-setsize={row.setsize}
                    aria-expanded={row.hasChildren ? row.isExpanded : undefined}
                    aria-selected={isActive}
                    tabIndex={isFocused ? 0 : -1}
                    draggable={renaming !== row.rel}
                    onDragStart={(e) => onDragStart(e, row)}
                    onDragOver={(e) => onDragOver(e, row)}
                    onDragLeave={() => setDropTarget((d) => (d?.rel === row.rel ? null : d))}
                    onDrop={(e) => void onDrop(e, row)}
                    onClick={() => {
                      setFocusedIndex(v.index)
                      openRow(row, true)
                    }}
                    onDoubleClick={() => openRow(row, false)}
                    className={cn(
                      'flex h-7 w-full cursor-default select-none items-center gap-1.5 rounded-md pr-2 text-sm',
                      'transition-colors duration-(--dur-fast)',
                      isActive
                        ? 'bg-selected text-primary'
                        : 'text-secondary hover:bg-hover hover:text-primary',
                      isDrop && dropTarget?.mode === 'into' && 'ring-1 ring-(--accent) bg-accent-soft',
                      isDrop && dropTarget?.mode === 'after' && 'shadow-[inset_0_-2px_0_var(--accent)]',
                    )}
                    style={{ paddingLeft: 6 + row.depth * 14 }}
                  >
                    {row.kind === 'folder' ? (
                      <>
                        <ChevronRight
                          size={13}
                          className={cn(
                            'shrink-0 text-muted transition-transform duration-(--dur-fast)',
                            row.isExpanded && 'rotate-90',
                          )}
                        />
                        <FolderIcon size={13} className="shrink-0 text-muted" />
                      </>
                    ) : (
                      <MethodBadge method={row.method ?? 'GET'} className="ml-4" />
                    )}

                    {renaming === row.rel ? (
                      <RenameInput
                        initial={row.name}
                        onDone={async (name) => {
                          setRenaming(null)
                          if (name && name !== row.name) {
                            await renameNode(collectionId, row.rel, name, row.kind)
                          }
                        }}
                      />
                    ) : (
                      <span className="truncate">{row.name}</span>
                    )}

                    {isDirty ? (
                      <span
                        role="status"
                        className="ml-auto size-1.5 shrink-0 rounded-full bg-(--accent)"
                        aria-label={t('tabs.unsaved')}
                      />
                    ) : null}
                  </div>
                </ContextMenu>
              </div>
            )
          })}
        </div>
      </div>
      <ConfirmDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null)
        }}
        title={t('sidebar.deleteTitle', { name: pendingDelete?.name ?? '' })}
        body={pendingDelete?.kind === 'folder' ? t('sidebar.deleteFolderBody') : t('sidebar.deleteBody')}
        confirmLabel={t('common.delete')}
        cancelLabel={t('common.cancel')}
        danger
        onConfirm={() => {
          if (pendingDelete) void doDelete(pendingDelete)
        }}
      />
    </>
  )
}

function RenameInput({ initial, onDone }: { initial: string; onDone: (name: string) => void }) {
  const [value, setValue] = useState(initial)
  return (
    <input
      ref={(el) => el?.focus()}
      data-selectable
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onFocus={(e) => e.target.select()}
      onBlur={() => onDone(value.trim())}
      onKeyDown={(e) => {
        e.stopPropagation()
        if (e.key === 'Enter') onDone(value.trim())
        if (e.key === 'Escape') onDone(initial)
      }}
      onClick={(e) => e.stopPropagation()}
      className="w-full min-w-0 rounded-sm border border-(--accent) bg-raised px-1 text-sm text-primary outline-none"
    />
  )
}
