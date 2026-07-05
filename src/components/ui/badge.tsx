import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

type Tone = 'neutral' | 'info' | 'success' | 'warning' | 'danger' | 'accent'

const tones: Record<Tone, string> = {
  neutral: 'bg-inset text-secondary',
  info: 'bg-info-soft text-(--info)',
  success: 'bg-success-soft text-(--success)',
  warning: 'bg-warning-soft text-(--warning)',
  danger: 'bg-danger-soft text-(--danger)',
  accent: 'bg-accent-soft text-accent-text',
}

export function Badge({
  tone = 'neutral',
  className,
  children,
}: {
  tone?: Tone
  className?: string
  children: ReactNode
}) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-2xs font-medium',
        tones[tone],
        className,
      )}
    >
      {children}
    </span>
  )
}
