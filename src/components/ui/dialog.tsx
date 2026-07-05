import { X } from 'lucide-react'
import { AlertDialog as AD, Dialog as D } from 'radix-ui'
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { Button } from './button'
import { IconButton } from './icon-button'

const sizes = { sm: 'max-w-105', md: 'max-w-140', lg: 'max-w-190' } as const

export function Modal({
  open,
  onOpenChange,
  title,
  size = 'md',
  children,
  footer,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: ReactNode
  size?: keyof typeof sizes
  children: ReactNode
  footer?: ReactNode
}) {
  return (
    <D.Root open={open} onOpenChange={onOpenChange}>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <D.Content
          className={cn(
            'fixed left-1/2 top-1/2 z-50 flex max-h-[85vh] w-[92vw] -translate-x-1/2 -translate-y-1/2 flex-col',
            'rounded-xl bg-overlay shadow-(--shadow-lg)',
            sizes[size],
          )}
        >
          <div className="flex shrink-0 items-center justify-between border-b border-subtle px-4 py-3">
            <D.Title className="text-sm font-semibold text-primary">{title}</D.Title>
            <D.Close asChild>
              <IconButton label="Close" size="sm" noTooltip>
                <X size={14} />
              </IconButton>
            </D.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-4">{children}</div>
          {footer ? (
            <div className="flex shrink-0 items-center justify-end gap-2 border-t border-subtle px-4 py-3">
              {footer}
            </div>
          ) : null}
        </D.Content>
      </D.Portal>
    </D.Root>
  )
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  body,
  confirmLabel,
  cancelLabel,
  danger,
  extraAction,
  onConfirm,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: ReactNode
  body?: ReactNode
  confirmLabel: string
  cancelLabel: string
  danger?: boolean
  /** e.g. "Discard" in a save/discard/cancel close dialog */
  extraAction?: { label: string; onSelect: () => void }
  onConfirm: () => void
}) {
  return (
    <AD.Root open={open} onOpenChange={onOpenChange}>
      <AD.Portal>
        <AD.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <AD.Content className="fixed left-1/2 top-1/2 z-50 w-[92vw] max-w-105 -translate-x-1/2 -translate-y-1/2 rounded-xl bg-overlay p-4 shadow-(--shadow-lg)">
          <AD.Title className="text-sm font-semibold text-primary">{title}</AD.Title>
          {body ? <AD.Description className="mt-1.5 text-xs text-secondary">{body}</AD.Description> : null}
          <div className="mt-4 flex items-center justify-end gap-2">
            <AD.Cancel asChild>
              <Button variant="ghost">{cancelLabel}</Button>
            </AD.Cancel>
            {extraAction ? (
              <AD.Action asChild>
                <Button variant="secondary" onClick={extraAction.onSelect}>
                  {extraAction.label}
                </Button>
              </AD.Action>
            ) : null}
            <AD.Action asChild>
              <Button variant={danger ? 'danger' : 'primary'} onClick={onConfirm} autoFocus>
                {confirmLabel}
              </Button>
            </AD.Action>
          </div>
        </AD.Content>
      </AD.Portal>
    </AD.Root>
  )
}
