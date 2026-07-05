import type { ButtonHTMLAttributes, ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { Tooltip } from './tooltip'

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string
  size?: 'sm' | 'md'
  variant?: 'ghost' | 'soft'
  children: ReactNode
  /** Skip the tooltip (e.g. inside menus). */
  noTooltip?: boolean
}

export function IconButton({
  label,
  size = 'md',
  variant = 'ghost',
  className,
  children,
  noTooltip,
  ...props
}: IconButtonProps) {
  const button = (
    <button
      type="button"
      aria-label={label}
      className={cn(
        'inline-flex select-none items-center justify-center rounded-md text-secondary transition-colors duration-(--dur-fast)',
        'hover:bg-hover hover:text-primary disabled:pointer-events-none disabled:opacity-50',
        variant === 'soft' && 'bg-accent-soft text-accent-text',
        size === 'sm' ? 'size-6' : 'size-7',
        className,
      )}
      {...props}
    >
      {children}
    </button>
  )
  if (noTooltip) return button
  return <Tooltip content={label}>{button}</Tooltip>
}
