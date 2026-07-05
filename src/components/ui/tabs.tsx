import { Tabs as T } from 'radix-ui'
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface TabDef<V extends string> {
  value: V
  label: ReactNode
  badge?: ReactNode
}

/** Underline tabs (request/response sub-tabs). */
export function UnderlineTabs<V extends string>({
  value,
  onChange,
  tabs,
  children,
  className,
  listClassName,
}: {
  value: V
  onChange: (v: V) => void
  tabs: TabDef<V>[]
  children?: ReactNode
  className?: string
  listClassName?: string
}) {
  return (
    <T.Root
      value={value}
      onValueChange={(v) => onChange(v as V)}
      className={cn('flex min-h-0 flex-col', className)}
    >
      <T.List
        className={cn(
          'flex shrink-0 items-center gap-0.5 overflow-x-auto border-b border-subtle px-2',
          listClassName,
        )}
      >
        {tabs.map((t) => (
          <T.Trigger
            key={t.value}
            value={t.value}
            className={cn(
              'relative flex select-none items-center gap-1.5 whitespace-nowrap px-2.5 py-1.5 text-xs font-medium text-secondary outline-none',
              'transition-colors duration-(--dur-fast) hover:text-primary',
              'data-[state=active]:text-primary',
              'after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:rounded-full after:bg-transparent',
              'data-[state=active]:after:bg-(--accent)',
            )}
          >
            {t.label}
            {t.badge}
          </T.Trigger>
        ))}
      </T.List>
      {children}
    </T.Root>
  )
}

export function TabPanel<V extends string>({
  value,
  children,
  className,
}: {
  value: V
  children: ReactNode
  className?: string
}) {
  return (
    <T.Content value={value} className={cn('min-h-0 flex-1 outline-none', className)}>
      {children}
    </T.Content>
  )
}

/** Segmented control (Pretty / Raw / Preview). */
export function Segmented<V extends string>({
  value,
  onChange,
  options,
  className,
}: {
  value: V
  onChange: (v: V) => void
  options: { value: V; label: ReactNode }[]
  className?: string
}) {
  return (
    <div
      className={cn('inline-flex items-center gap-0.5 rounded-md bg-inset p-0.5', className)}
      role="tablist"
    >
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="tab"
          aria-selected={value === o.value}
          onClick={() => onChange(o.value)}
          className={cn(
            'select-none rounded-sm px-2 py-0.5 text-xs font-medium transition-colors duration-(--dur-fast)',
            value === o.value
              ? 'bg-raised text-primary shadow-(--shadow-sm)'
              : 'text-secondary hover:text-primary',
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}
