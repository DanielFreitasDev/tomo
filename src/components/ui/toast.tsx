/** Minimal toast system: a store + a viewport, styled with our tokens. */
import { AlertCircle, CheckCircle2, Info, TriangleAlert, X } from 'lucide-react'
import { create } from 'zustand'
import { cn } from '@/lib/cn'

export type ToastTone = 'info' | 'success' | 'warning' | 'danger'

export interface ToastItem {
  id: number
  tone: ToastTone
  title: string
  body?: string
  action?: { label: string; onSelect: () => void }
  /** ms; 0 = sticky */
  duration: number
}

interface ToastState {
  items: ToastItem[]
  push: (t: Omit<ToastItem, 'id' | 'duration'> & { duration?: number }) => number
  dismiss: (id: number) => void
}

let toastId = 1

export const useToasts = create<ToastState>((set) => ({
  items: [],
  push(t) {
    const id = toastId++
    const item: ToastItem = { id, duration: 5000, ...t }
    set((s) => ({ items: [...s.items, item] }))
    if (item.duration > 0) {
      setTimeout(() => {
        useToasts.getState().dismiss(id)
      }, item.duration)
    }
    return id
  },
  dismiss(id) {
    set((s) => ({ items: s.items.filter((t) => t.id !== id) }))
  },
}))

export const toast = {
  info: (title: string, body?: string) => useToasts.getState().push({ tone: 'info', title, body }),
  success: (title: string, body?: string) => useToasts.getState().push({ tone: 'success', title, body }),
  warning: (title: string, body?: string) => useToasts.getState().push({ tone: 'warning', title, body }),
  danger: (title: string, body?: string) => useToasts.getState().push({ tone: 'danger', title, body }),
  action: (tone: ToastTone, title: string, action: { label: string; onSelect: () => void }, body?: string) =>
    useToasts.getState().push({ tone, title, body, action, duration: 10_000 }),
}

const icons: Record<ToastTone, typeof Info> = {
  info: Info,
  success: CheckCircle2,
  warning: TriangleAlert,
  danger: AlertCircle,
}

const toneColor: Record<ToastTone, string> = {
  info: 'var(--info)',
  success: 'var(--success)',
  warning: 'var(--warning)',
  danger: 'var(--danger)',
}

export function ToastViewport() {
  const items = useToasts((s) => s.items)
  const dismiss = useToasts((s) => s.dismiss)
  return (
    <div
      aria-live="polite"
      className="pointer-events-none fixed bottom-3 right-3 z-50 flex w-80 flex-col gap-2"
    >
      {items.map((t) => {
        const Icon = icons[t.tone]
        return (
          <div
            key={t.id}
            className={cn(
              'pointer-events-auto flex items-start gap-2.5 rounded-lg bg-overlay p-3 shadow-(--shadow-lg)',
            )}
          >
            <Icon size={15} style={{ color: toneColor[t.tone] }} className="mt-0.5 shrink-0" />
            <div className="min-w-0 flex-1">
              <div className="text-xs font-medium text-primary">{t.title}</div>
              {t.body ? <div className="mt-0.5 text-xs text-secondary">{t.body}</div> : null}
              {t.action ? (
                <button
                  type="button"
                  className="mt-1.5 text-xs font-medium text-accent-text hover:underline"
                  onClick={() => {
                    t.action?.onSelect()
                    dismiss(t.id)
                  }}
                >
                  {t.action.label}
                </button>
              ) : null}
            </div>
            <button
              type="button"
              aria-label="Dismiss"
              className="shrink-0 text-muted hover:text-primary"
              onClick={() => dismiss(t.id)}
            >
              <X size={13} />
            </button>
          </div>
        )
      })}
    </div>
  )
}
