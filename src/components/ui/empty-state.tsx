import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export function EmptyState({
  icon: Icon,
  title,
  hint,
  action,
  className,
}: {
  icon?: LucideIcon
  title: string
  hint?: ReactNode
  action?: ReactNode
  className?: string
}) {
  return (
    <div className={cn('flex h-full flex-col items-center justify-center gap-2 p-8 text-center', className)}>
      {Icon ? <Icon size={24} strokeWidth={1.75} className="text-muted" /> : null}
      <div className="text-sm font-medium text-secondary">{title}</div>
      {hint ? <div className="max-w-64 text-xs text-muted">{hint}</div> : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  )
}
