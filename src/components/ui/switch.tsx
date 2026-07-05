import { Check, Minus } from 'lucide-react'
import { Checkbox as C, Switch as S } from 'radix-ui'
import { cn } from '@/lib/cn'

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  ariaLabel,
}: {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  disabled?: boolean
  ariaLabel?: string
}) {
  return (
    <S.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={ariaLabel}
      className={cn(
        'relative h-4.5 w-8 shrink-0 rounded-full border border-transparent transition-colors duration-(--dur-base)',
        checked ? 'bg-(--accent)' : 'bg-(--border-strong)',
        'disabled:opacity-50',
      )}
    >
      <S.Thumb
        className={cn(
          'block size-3.5 translate-x-0.5 rounded-full bg-white shadow-sm transition-transform duration-(--dur-base)',
          checked && 'translate-x-4',
        )}
      />
    </S.Root>
  )
}

export function Checkbox({
  checked,
  onCheckedChange,
  disabled,
  ariaLabel,
  indeterminate,
}: {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  disabled?: boolean
  ariaLabel?: string
  indeterminate?: boolean
}) {
  return (
    <C.Root
      checked={indeterminate ? 'indeterminate' : checked}
      onCheckedChange={(v) => onCheckedChange(v === true)}
      disabled={disabled}
      aria-label={ariaLabel}
      className={cn(
        'flex size-3.5 shrink-0 items-center justify-center rounded-sm border transition-colors duration-(--dur-fast)',
        checked || indeterminate
          ? 'border-(--accent) bg-(--accent) text-(--accent-fg)'
          : 'border-strong bg-raised hover:border-(--accent)',
        'disabled:opacity-50',
      )}
    >
      <C.Indicator>
        {indeterminate ? <Minus size={10} strokeWidth={3} /> : <Check size={10} strokeWidth={3} />}
      </C.Indicator>
    </C.Root>
  )
}
