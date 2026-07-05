import { useEffect, useState } from 'react'
import { ToastViewport } from '@/components/ui/toast'
import { TooltipProvider } from '@/components/ui/tooltip'
import { isTauri } from '@/lib/transport'
import { bootTransportListeners, openCollection } from '@/stores/actions/fs-actions'
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

  if (!loaded) return <div className="h-full bg-app" />

  return (
    <TooltipProvider>
      {route === '#/gallery' ? <Gallery /> : <Shell />}
      <ToastViewport />
    </TooltipProvider>
  )
}
