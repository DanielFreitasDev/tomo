import { Modal } from '@/components/ui/dialog'
import { useT } from '@/i18n'
import { useUi } from '@/stores/ui'

export function AboutModal() {
  const t = useT()
  const open = useUi((s) => s.modal === 'about')
  const closeModal = useUi((s) => s.openModal)

  return (
    <Modal open={open} onOpenChange={(o) => !o && closeModal(null)} title="About" size="sm">
      <div className="flex flex-col items-center gap-2 py-4 text-center">
        <div className="font-mono text-3xl font-bold text-accent-text">友 {t('app.name')}</div>
        <div className="text-xs text-muted">{t('app.tagline')}</div>
        <div className="mt-2 text-2xs text-muted">v0.1.0 · TOM(L) · 友 friend · a volume of requests</div>
        <div className="text-2xs text-muted">MIT · Tauri 2 · Rust · React 19</div>
      </div>
    </Modal>
  )
}
