import { useEffect } from 'react'
import { Group, Panel, Separator } from 'react-resizable-panels'
import { toast } from '@/components/ui/toast'
import { RequestPane } from '@/features/request/RequestPane'
import { ResponsePane } from '@/features/response/ResponsePane'
import { Sidebar } from '@/features/sidebar/Sidebar'
import { useT } from '@/i18n'
import { installKeyboardListener, registerShortcuts } from '@/lib/keys/keymap'
import { cancelActiveRequest, sendActiveRequest } from '@/stores/actions/request-actions'
import { closeTab, reopenLastClosed, saveAllTabs, saveTab } from '@/stores/actions/tab-actions'
import { useTabs } from '@/stores/tabs'
import { useUi } from '@/stores/ui'
import { Titlebar } from './Titlebar'

export function Shell() {
  const t = useT()
  const sidebarCollapsed = useUi((s) => s.sidebarCollapsed)
  const splitOrientation = useUi((s) => s.splitOrientation)
  const activeTab = useTabs((s) => s.tabs.find((tab) => tab.id === s.activeId))

  useEffect(() => {
    const uninstall = installKeyboardListener()
    const unregister = registerShortcuts([
      {
        id: 'send',
        combo: 'mod+enter',
        inInputs: true,
        run: () => {
          const id = useTabs.getState().activeId
          if (id) void sendActiveRequest(id)
        },
      },
      {
        id: 'cancel',
        combo: 'escape',
        run: () => {
          const id = useTabs.getState().activeId
          if (id) void cancelActiveRequest(id)
        },
      },
      {
        id: 'save',
        combo: 'mod+s',
        inInputs: true,
        run: () => {
          const id = useTabs.getState().activeId
          if (id)
            void saveTab(id).then((outcome) => {
              if (outcome === 'saved') toast.success(t('toast.saved'))
            })
        },
      },
      { id: 'save-all', combo: 'mod+alt+s', inInputs: true, run: () => void saveAllTabs() },
      {
        id: 'close-tab',
        combo: 'mod+w',
        inInputs: true,
        run: () => {
          const id = useTabs.getState().activeId
          if (id) closeTab(id)
        },
      },
      { id: 'reopen-tab', combo: 'mod+shift+t', inInputs: true, run: () => void reopenLastClosed() },
      { id: 'close-all', combo: 'mod+shift+w', inInputs: true, run: () => useTabs.getState().closeAll() },
      { id: 'toggle-sidebar', combo: 'mod+b', inInputs: true, run: () => useUi.getState().toggleSidebar() },
      { id: 'toggle-split', combo: 'mod+j', inInputs: true, run: () => useUi.getState().toggleSplit() },
      { id: 'palette', combo: 'mod+k', inInputs: true, run: () => useUi.getState().setPaletteOpen(true) },
      { id: 'settings', combo: 'mod+,', inInputs: true, run: () => useUi.getState().openModal('settings') },
      {
        id: 'environments',
        combo: 'mod+e',
        inInputs: true,
        run: () => useUi.getState().openModal('environments'),
      },
      ...Array.from({ length: 9 }, (_, i) => ({
        id: `tab-${i + 1}`,
        combo: `mod+${i + 1}`,
        inInputs: true,
        run: () => {
          const tabs = useTabs.getState().tabs
          const target = i === 8 ? tabs[tabs.length - 1] : tabs[i]
          if (target) useTabs.getState().activate(target.id)
        },
      })),
      {
        id: 'next-tab',
        combo: 'ctrl+tab',
        inInputs: true,
        run: () => cycleTab(1),
      },
      {
        id: 'prev-tab',
        combo: 'ctrl+shift+tab',
        inInputs: true,
        run: () => cycleTab(-1),
      },
    ])
    return () => {
      unregister()
      uninstall()
    }
  }, [t])

  return (
    <div className="flex h-screen flex-col bg-app">
      <Titlebar />
      <div className="flex min-h-0 flex-1 gap-0">
        <Group orientation="horizontal" className="min-h-0 flex-1">
          {!sidebarCollapsed ? (
            <>
              <Panel defaultSize="22%" minSize="180px" maxSize="40%" id="sidebar">
                <Sidebar />
              </Panel>
              <Separator className="w-px bg-(--border-subtle) transition-colors hover:bg-(--accent)" />
            </>
          ) : null}
          <Panel id="main" className="min-w-0">
            <main className="h-full min-w-0 overflow-hidden rounded-tl-lg border border-subtle bg-surface">
              {activeTab ? (
                <Group orientation={splitOrientation} className="h-full">
                  <Panel defaultSize="55%" minSize="25%" id="request">
                    <RequestPane tab={activeTab} />
                  </Panel>
                  <Separator
                    className={
                      splitOrientation === 'horizontal'
                        ? 'w-px bg-(--border-subtle) transition-colors hover:bg-(--accent)'
                        : 'h-px bg-(--border-subtle) transition-colors hover:bg-(--accent)'
                    }
                  />
                  <Panel defaultSize="45%" minSize="20%" id="response">
                    <ResponsePane tab={activeTab} />
                  </Panel>
                </Group>
              ) : (
                <WelcomePane />
              )}
            </main>
          </Panel>
        </Group>
      </div>
    </div>
  )
}

function cycleTab(delta: number) {
  const { tabs, activeId, activate } = useTabs.getState()
  if (tabs.length === 0) return
  const index = tabs.findIndex((t) => t.id === activeId)
  const next = tabs[(index + delta + tabs.length) % tabs.length]
  if (next) activate(next.id)
}

function WelcomePane() {
  const t = useT()
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5">
      <div className="font-mono text-2xl font-bold text-accent-text">友 {t('app.name')}</div>
      <div className="text-xs text-muted">{t('app.tagline')}</div>
    </div>
  )
}
