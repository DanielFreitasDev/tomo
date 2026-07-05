/** Shared item styling for DropdownMenu and ContextMenu. */
// biome-ignore-all lint/suspicious/noArrayIndexKey: menu entries are static structures
import { ContextMenu as CM, DropdownMenu as DM } from 'radix-ui'
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { Kbd } from './kbd'

export const menuContentClass =
  'z-50 min-w-44 rounded-lg bg-overlay p-1 shadow-(--shadow-lg) data-[state=open]:animate-in'

export const menuItemClass = (danger?: boolean) =>
  cn(
    'flex cursor-default select-none items-center justify-between gap-3 rounded-md px-2 py-1 text-sm outline-none',
    'data-[highlighted]:bg-hover data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
    danger ? 'text-(--danger)' : 'text-primary',
  )

export interface MenuItemDef {
  label: ReactNode
  icon?: ReactNode
  kbd?: string
  danger?: boolean
  disabled?: boolean
  onSelect: () => void
}

export type MenuEntry = MenuItemDef | 'separator'

function ItemBody({ item }: { item: MenuItemDef }) {
  return (
    <>
      <span className="flex items-center gap-2">
        {item.icon}
        {item.label}
      </span>
      {item.kbd ? <Kbd>{item.kbd}</Kbd> : null}
    </>
  )
}

export function Dropdown({
  trigger,
  entries,
  align = 'start',
}: {
  trigger: ReactNode
  entries: MenuEntry[]
  align?: 'start' | 'end' | 'center'
}) {
  return (
    <DM.Root>
      <DM.Trigger asChild>{trigger}</DM.Trigger>
      <DM.Portal>
        <DM.Content align={align} sideOffset={4} className={menuContentClass}>
          {entries.map((entry, i) =>
            entry === 'separator' ? (
              <DM.Separator key={`sep-${i}`} className="mx-1 my-1 h-px bg-(--border-subtle)" />
            ) : (
              <DM.Item
                key={`item-${i}`}
                disabled={entry.disabled}
                onSelect={entry.onSelect}
                className={menuItemClass(entry.danger)}
              >
                <ItemBody item={entry} />
              </DM.Item>
            ),
          )}
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  )
}

export function ContextMenu({ children, entries }: { children: ReactNode; entries: MenuEntry[] }) {
  return (
    <CM.Root>
      <CM.Trigger asChild>{children}</CM.Trigger>
      <CM.Portal>
        <CM.Content className={menuContentClass}>
          {entries.map((entry, i) =>
            entry === 'separator' ? (
              <CM.Separator key={`sep-${i}`} className="mx-1 my-1 h-px bg-(--border-subtle)" />
            ) : (
              <CM.Item
                key={`item-${i}`}
                disabled={entry.disabled}
                onSelect={entry.onSelect}
                className={menuItemClass(entry.danger)}
              >
                <ItemBody item={entry} />
              </CM.Item>
            ),
          )}
        </CM.Content>
      </CM.Portal>
    </CM.Root>
  )
}
