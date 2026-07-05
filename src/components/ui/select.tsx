import { Check, ChevronDown } from 'lucide-react'
import { Select as S } from 'radix-ui'
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface SelectOption<V extends string> {
  value: V
  label: ReactNode
}

export function Select<V extends string>({
  value,
  onChange,
  options,
  size = 'md',
  className,
  triggerClassName,
  ariaLabel,
}: {
  value: V
  onChange: (value: V) => void
  options: SelectOption<V>[]
  size?: 'sm' | 'md'
  className?: string
  triggerClassName?: string
  ariaLabel?: string
}) {
  return (
    <S.Root value={value} onValueChange={onChange}>
      <S.Trigger
        aria-label={ariaLabel}
        className={cn(
          'inline-flex select-none items-center justify-between gap-1.5 rounded-md border border-default bg-raised px-2 text-sm text-primary',
          'transition-colors duration-(--dur-fast) hover:bg-hover data-[state=open]:border-(--accent)',
          size === 'sm' ? 'h-6 text-xs' : 'h-7',
          triggerClassName,
        )}
      >
        <S.Value />
        <S.Icon>
          <ChevronDown size={13} className="text-muted" />
        </S.Icon>
      </S.Trigger>
      <S.Portal>
        <S.Content
          position="popper"
          sideOffset={4}
          className={cn('z-50 min-w-28 rounded-lg bg-overlay p-1 shadow-(--shadow-lg)', className)}
        >
          <S.Viewport>
            {options.map((o) => (
              <S.Item
                key={o.value}
                value={o.value}
                className={cn(
                  'flex cursor-default select-none items-center justify-between gap-2 rounded-md px-2 py-1 text-sm text-primary outline-none',
                  'data-[highlighted]:bg-hover',
                )}
              >
                <S.ItemText>{o.label}</S.ItemText>
                <S.ItemIndicator>
                  <Check size={13} className="text-accent-text" />
                </S.ItemIndicator>
              </S.Item>
            ))}
          </S.Viewport>
        </S.Content>
      </S.Portal>
    </S.Root>
  )
}
