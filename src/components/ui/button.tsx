import type { ButtonHTMLAttributes, ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { Spinner } from './spinner'

type Variant = 'primary' | 'secondary' | 'ghost' | 'soft' | 'danger'
type Size = 'sm' | 'md'

const variants: Record<Variant, string> = {
  primary:
    'bg-(--accent) text-(--accent-fg) hover:bg-(--accent-hover) active:bg-(--accent-active) border border-transparent',
  secondary:
    'bg-raised text-primary border border-default hover:bg-hover active:bg-active shadow-(--shadow-sm)',
  ghost: 'bg-transparent text-secondary hover:bg-hover hover:text-primary border border-transparent',
  soft: 'bg-accent-soft text-accent-text border border-transparent hover:bg-(--accent-soft)',
  danger: 'bg-(--danger) text-white border border-transparent hover:opacity-90',
}

const sizes: Record<Size, string> = {
  sm: 'h-6 px-2 text-xs rounded-sm gap-1',
  md: 'h-7 px-3 text-sm rounded-md gap-1.5',
}

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant
  size?: Size
  loading?: boolean
  icon?: ReactNode
}

export function Button({
  variant = 'secondary',
  size = 'md',
  loading = false,
  icon,
  className,
  children,
  disabled,
  ...props
}: ButtonProps) {
  return (
    <button
      type="button"
      className={cn(
        'inline-flex select-none items-center justify-center font-medium transition-colors duration-(--dur-fast)',
        'disabled:pointer-events-none disabled:opacity-50',
        variants[variant],
        sizes[size],
        className,
      )}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <Spinner size={12} /> : icon}
      {children}
    </button>
  )
}
