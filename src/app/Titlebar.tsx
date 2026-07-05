/**
 * Custom titlebar (38px): app menu + tab strip + drag zone + window controls.
 * Window controls only render inside Tauri; the browser build keeps the row
 * as a plain header. `settings.titlebar: native` support can layer on later.
 */
import { Copy, Maximize, Menu, Minus, X } from 'lucide-react'
import { IconButton } from '@/components/ui/icon-button'
import { Dropdown } from '@/components/ui/menu'
import { TabStrip } from '@/features/tabs/TabStrip'
import { useT } from '@/i18n'
import { isTauri } from '@/lib/transport'
import { useUi } from '@/stores/ui'

async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  const w = getCurrentWindow()
  if (action === 'minimize') await w.minimize()
  else if (action === 'toggleMaximize') await w.toggleMaximize()
  else await w.close()
}

export function Titlebar() {
  const t = useT()
  const openModal = useUi((s) => s.openModal)

  return (
    <header className="flex h-9.5 shrink-0 items-stretch gap-1 pl-1.5 pt-1.5" data-tauri-drag-region>
      <div className="flex items-center pb-1">
        <Dropdown
          trigger={
            <span>
              <IconButton label={t('app.name')} size="sm" noTooltip>
                <Menu size={14} />
              </IconButton>
            </span>
          }
          entries={[
            { label: t('common.settings'), kbd: 'Ctrl+,', onSelect: () => openModal('settings') },
            { label: t('env.edit'), kbd: 'Ctrl+E', onSelect: () => openModal('environments') },
            'separator',
            { label: 'About', onSelect: () => openModal('about') },
          ]}
        />
      </div>

      <TabStrip />

      <div className="w-16 shrink-0" data-tauri-drag-region />

      {isTauri() ? (
        <div className="flex shrink-0 items-center pb-1 pr-1.5">
          <IconButton label="Minimize" size="sm" noTooltip onClick={() => void windowAction('minimize')}>
            <Minus size={13} />
          </IconButton>
          <IconButton
            label="Maximize"
            size="sm"
            noTooltip
            onClick={() => void windowAction('toggleMaximize')}
          >
            <span className="relative inline-flex">
              <Maximize size={11} />
              <Copy size={0} className="hidden" />
            </span>
          </IconButton>
          <IconButton
            label="Close"
            size="sm"
            noTooltip
            className="hover:bg-(--danger) hover:text-white"
            onClick={() => void windowAction('close')}
          >
            <X size={13} />
          </IconButton>
        </div>
      ) : null}
    </header>
  )
}
