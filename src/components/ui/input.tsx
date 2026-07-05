import type { InputHTMLAttributes, ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean
  mono?: boolean
  inputSize?: 'sm' | 'md'
  prefixEl?: ReactNode
  suffixEl?: ReactNode
}

export function Input({
  invalid,
  mono,
  inputSize = 'md',
  prefixEl,
  suffixEl,
  className,
  ...props
}: InputProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-1.5 rounded-md border bg-raised px-2 transition-colors duration-(--dur-fast)',
        'focus-within:border-(--accent) focus-within:ring-2 focus-within:ring-(--accent-soft)',
        invalid ? 'border-(--danger)' : 'border-default',
        inputSize === 'sm' ? 'h-6' : 'h-7',
        className,
      )}
    >
      {prefixEl}
      <input
        data-selectable
        className={cn(
          'w-full min-w-0 flex-1 bg-transparent text-primary outline-none placeholder:text-muted',
          mono ? 'font-mono text-xs' : 'text-sm',
        )}
        {...props}
      />
      {suffixEl}
    </div>
  )
}
