/**
 * cmdk command palette. Default mode: fuzzy across actions AND requests
 * (with method badges). Prefixes: `>` actions only, `@` environments,
 * ctrl+p file mode.
 */
import { Command } from 'cmdk'
import {
  FilePlus2,
  FolderOpen,
  Languages,
  Layers,
  Moon,
  PanelLeft,
  Save,
  Send,
  Settings2,
  SplitSquareHorizontal,
} from 'lucide-react'
import { useMemo, useState } from 'react'
import { MethodBadge } from '@/components/ui/method-badge'
import { toast } from '@/components/ui/toast'
import { useT } from '@/i18n'
import { transport } from '@/lib/transport'
import { createRequest, pickAndOpenCollection } from '@/stores/actions/fs-actions'
import { sendActiveRequest } from '@/stores/actions/request-actions'
import { openRequestTab, saveTab } from '@/stores/actions/tab-actions'
import { useCollections, walkTree } from '@/stores/collections'
import { useSettings } from '@/stores/settings'
import { useTabs } from '@/stores/tabs'
import { useUi } from '@/stores/ui'

interface Action {
  id: string
  label: string
  icon: React.ReactNode
  keywords?: string
  run: () => void
}

export function CommandPalette() {
  const t = useT()
  const open = useUi((s) => s.paletteOpen)
  const setOpen = useUi((s) => s.setPaletteOpen)
  const openModal = useUi((s) => s.openModal)
  const [search, setSearch] = useState('')

  const collectionId = useCollections((s) => s.order[0])
  const trees = useCollections((s) => s.trees)
  const info = useCollections((s) => (collectionId ? s.byId[collectionId] : undefined))

  const close = () => {
    setOpen(false)
    setSearch('')
  }

  const requests = useMemo(() => {
    if (!collectionId) return []
    const list: { rel: string; name: string; method: string }[] = []
    walkTree(trees[collectionId] ?? [], (node) => {
      if (node.kind === 'request') list.push({ rel: node.rel, name: node.name, method: node.method ?? 'GET' })
    })
    return list
  }, [collectionId, trees])

  const actions: Action[] = [
    {
      id: 'send',
      label: t('request.send'),
      icon: <Send size={14} />,
      run: () => {
        const id = useTabs.getState().activeId
        if (id) void sendActiveRequest(id)
      },
    },
    {
      id: 'save',
      label: t('common.save'),
      icon: <Save size={14} />,
      run: () => {
        const id = useTabs.getState().activeId
        if (id) void saveTab(id).then((o) => o === 'saved' && toast.success(t('toast.saved')))
      },
    },
    {
      id: 'new-request',
      label: t('sidebar.newRequest'),
      icon: <FilePlus2 size={14} />,
      run: () => collectionId && void createRequest(collectionId, '', 'New request'),
    },
    {
      id: 'open-collection',
      label: t('sidebar.openCollection'),
      icon: <FolderOpen size={14} />,
      run: () => void pickAndOpenCollection(),
    },
    {
      id: 'environments',
      label: t('env.edit'),
      icon: <Layers size={14} />,
      run: () => openModal('environments'),
    },
    {
      id: 'settings',
      label: t('common.settings'),
      icon: <Settings2 size={14} />,
      run: () => openModal('settings'),
    },
    {
      id: 'toggle-theme',
      label: t('settings.theme'),
      icon: <Moon size={14} />,
      keywords: 'dark light',
      run: () => {
        const s = useSettings.getState()
        s.update({ theme: s.resolvedTheme() === 'dark' ? 'light' : 'dark' })
      },
    },
    {
      id: 'toggle-sidebar',
      label: 'Toggle sidebar',
      icon: <PanelLeft size={14} />,
      run: () => useUi.getState().toggleSidebar(),
    },
    {
      id: 'toggle-split',
      label: 'Toggle split orientation',
      icon: <SplitSquareHorizontal size={14} />,
      run: () => useUi.getState().toggleSplit(),
    },
    {
      id: 'language',
      label: t('settings.language'),
      icon: <Languages size={14} />,
      run: () => {
        const s = useSettings.getState()
        s.update({ locale: (s.locale ?? 'en') === 'en' ? 'pt-BR' : 'en' })
      },
    },
  ]

  const mode = search.startsWith('>') ? 'actions' : search.startsWith('@') ? 'env' : 'all'

  const runAndClose = (fn: () => void) => {
    fn()
    close()
  }

  return (
    <Command.Dialog
      open={open}
      onOpenChange={(o) => (o ? setOpen(true) : close())}
      label={t('palette.placeholder')}
      shouldFilter
      className="fixed left-1/2 top-[15%] z-50 w-[92vw] max-w-140 -translate-x-1/2 overflow-hidden rounded-xl bg-overlay shadow-(--shadow-lg)"
    >
      <Command.Input
        autoFocus
        value={search}
        onValueChange={setSearch}
        placeholder={t('palette.placeholder')}
        className="w-full border-b border-subtle bg-transparent px-4 py-3 text-sm text-primary outline-none placeholder:text-muted"
      />
      <Command.List className="max-h-96 overflow-y-auto p-1.5">
        <Command.Empty className="px-3 py-6 text-center text-xs text-muted">
          {t('palette.noResults')}
        </Command.Empty>

        {mode !== 'env' ? (
          <Command.Group
            heading="Actions"
            className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1 [&_[cmdk-group-heading]]:text-2xs [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:text-muted"
          >
            {actions.map((a) => (
              <Command.Item
                key={a.id}
                value={`${a.label} ${a.keywords ?? ''}`}
                onSelect={() => runAndClose(a.run)}
                className="flex cursor-default items-center gap-2.5 rounded-md px-2 py-1.5 text-sm text-primary data-[selected=true]:bg-hover"
              >
                <span className="text-muted">{a.icon}</span>
                {a.label}
              </Command.Item>
            ))}
          </Command.Group>
        ) : null}

        {mode === 'env' || mode === 'all' ? (
          <Command.Group
            heading={t('env.title')}
            className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1 [&_[cmdk-group-heading]]:text-2xs [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:text-muted"
          >
            {(info?.environments ?? []).map((name) => (
              <Command.Item
                key={name}
                value={`@${name}`}
                onSelect={() =>
                  runAndClose(() => {
                    if (collectionId) {
                      useCollections.getState().setSelectedEnv(collectionId, name)
                      void transport().invoke('select_environment', { id: collectionId, name })
                    }
                  })
                }
                className="flex cursor-default items-center gap-2.5 rounded-md px-2 py-1.5 text-sm text-primary data-[selected=true]:bg-hover"
              >
                <Layers size={14} className="text-muted" />
                {name}
                {info?.selectedEnv === name ? (
                  <span className="ml-auto text-2xs text-accent-text">active</span>
                ) : null}
              </Command.Item>
            ))}
          </Command.Group>
        ) : null}

        {mode !== 'env' && collectionId ? (
          <Command.Group
            heading="Requests"
            className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1 [&_[cmdk-group-heading]]:text-2xs [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:text-muted"
          >
            {requests.slice(0, 200).map((r) => (
              <Command.Item
                key={r.rel}
                value={`${r.name} ${r.rel}`}
                onSelect={() => runAndClose(() => void openRequestTab(collectionId, r.rel))}
                className="flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-sm text-primary data-[selected=true]:bg-hover"
              >
                <MethodBadge method={r.method} />
                <span className="truncate">{r.name}</span>
              </Command.Item>
            ))}
          </Command.Group>
        ) : null}
      </Command.List>
    </Command.Dialog>
  )
}
