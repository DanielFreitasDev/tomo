import { useEffect, useState } from 'react'
import { ToastViewport } from '@/components/ui/toast'
import { TooltipProvider } from '@/components/ui/tooltip'
import { EnvironmentsModal } from '@/features/environments/EnvironmentsModal'
import { CommandPalette } from '@/features/palette/CommandPalette'
import { AboutModal } from '@/features/settings/AboutModal'
import { SettingsModal } from '@/features/settings/SettingsModal'
import { DirtyCloseDialog } from '@/features/tabs/DirtyCloseDialog'
import { isTauri } from '@/lib/transport'
import { bootTransportListeners, openCollection } from '@/stores/actions/fs-actions'
import { requestCloseWindow, shouldAllowWindowClose } from '@/stores/actions/tab-actions'
import { useSettings, watchSystemTheme } from '@/stores/settings'
import { useUi } from '@/stores/ui'
import { Gallery } from './Gallery'
import { Shell } from './Shell'

function useHashRoute(): string {
  const [hash, setHash] = useState(() => window.location.hash)
  useEffect(() => {
    const onChange = () => setHash(window.location.hash)
    window.addEventListener('hashchange', onChange)
    return () => window.removeEventListener('hashchange', onChange)
  }, [])
  return hash
}

export function App() {
  const route = useHashRoute()
  const loaded = useSettings((s) => s.loaded)

  useEffect(() => {
    void useSettings.getState().load()
    void useUi.getState().hydrate()
    const unlisten = bootTransportListeners()
    const unwatch = watchSystemTheme()
    // browser dev/e2e: the mock workspace opens automatically
    if (!isTauri()) void openCollection('/mock/acme-api')
    return () => {
      unlisten()
      unwatch()
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) return
    let disposed = false
    let unlisten: (() => void) | undefined
    void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      if (disposed) return
      void getCurrentWindow()
        .onCloseRequested((event) => {
          if (shouldAllowWindowClose()) return
          event.preventDefault()
          requestCloseWindow()
        })
        .then((fn) => {
          if (disposed) fn()
          else unlisten = fn
        })
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  if (!loaded) return <div className="h-full bg-app" />

  if (route === '#/gallery') {
    return (
      <TooltipProvider>
        <Gallery />
        <ToastViewport />
      </TooltipProvider>
    )
  }

  return (
    <TooltipProvider>
      <Shell />
      <CommandPalette />
      <SettingsModal />
      <EnvironmentsModal />
      <AboutModal />
      <DirtyCloseDialog />
      <ToastViewport />
    </TooltipProvider>
  )
}
