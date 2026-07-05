import { BookOpenText, FilePlus2, FolderOpen, FolderPlus, Search, Settings2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { EmptyState } from '@/components/ui/empty-state'
import { IconButton } from '@/components/ui/icon-button'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { useT } from '@/i18n'
import { transport } from '@/lib/transport'
import { createFolder, createRequest, pickAndOpenCollection } from '@/stores/actions/fs-actions'
import { useCollections } from '@/stores/collections'
import { useUi } from '@/stores/ui'
import { Tree } from './tree/Tree'

const NO_ENV = '__none__'

export function Sidebar() {
  const t = useT()
  const filter = useUi((s) => s.filter)
  const setFilter = useUi((s) => s.setFilter)
  const openModal = useUi((s) => s.openModal)
  const order = useCollections((s) => s.order)
  const byId = useCollections((s) => s.byId)
  const trees = useCollections((s) => s.trees)
  const setSelectedEnv = useCollections((s) => s.setSelectedEnv)

  const activeId = order[0]
  const info = activeId ? byId[activeId] : undefined
  const nodes = activeId ? (trees[activeId] ?? []) : []

  return (
    <aside className="flex h-full min-w-0 flex-col bg-app" aria-label={t('sidebar.collections')}>
      <div className="flex shrink-0 items-center gap-1 px-2 pb-1.5">
        <span className="min-w-0 flex-1 truncate text-xs font-semibold text-primary">
          {info?.name ?? t('app.name')}
        </span>
        {info ? (
          <>
            <IconButton
              label={t('sidebar.newRequest')}
              size="sm"
              onClick={() => activeId && void createRequest(activeId, '', 'New request')}
            >
              <FilePlus2 size={14} />
            </IconButton>
            <IconButton
              label={t('sidebar.newFolder')}
              size="sm"
              onClick={() => activeId && void createFolder(activeId, '', 'New folder')}
            >
              <FolderPlus size={14} />
            </IconButton>
          </>
        ) : null}
        <IconButton
          label={t('sidebar.openCollection')}
          size="sm"
          onClick={() => void pickAndOpenCollection()}
        >
          <FolderOpen size={14} />
        </IconButton>
      </div>

      {info ? (
        <>
          <div className="shrink-0 px-2 pb-1.5">
            <Input
              inputSize="sm"
              placeholder={t('sidebar.filter')}
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              prefixEl={<Search size={12} className="shrink-0 text-muted" />}
              aria-label={t('sidebar.filter')}
            />
          </div>
          <Tree collectionId={info.id} nodes={nodes} />
          <footer className="flex shrink-0 items-center gap-1.5 border-t border-subtle px-2 py-1.5">
            <Select
              size="sm"
              ariaLabel={t('env.title')}
              triggerClassName="flex-1 min-w-0"
              value={info.selectedEnv ?? NO_ENV}
              onChange={(v) => {
                const env = v === NO_ENV ? undefined : v
                setSelectedEnv(info.id, env)
                void transport().invoke('select_environment', { id: info.id, name: env })
              }}
              options={[
                { value: NO_ENV, label: t('env.noEnvironment') },
                ...info.environments.map((e) => ({ value: e, label: e })),
              ]}
            />
            <IconButton label={t('common.settings')} size="sm" onClick={() => openModal('settings')}>
              <Settings2 size={14} />
            </IconButton>
          </footer>
        </>
      ) : (
        <EmptyState
          icon={BookOpenText}
          title={t('sidebar.empty.title')}
          hint={t('sidebar.empty.hint')}
          action={
            <Button variant="primary" onClick={() => void pickAndOpenCollection()}>
              {t('sidebar.openCollection')}
            </Button>
          }
        />
      )}
    </aside>
  )
}
