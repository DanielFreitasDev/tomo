import { Tooltip as T } from 'radix-ui'
import type { ReactNode } from 'react'
import { Kbd } from './kbd'

export function TooltipProvider({ children }: { children: ReactNode }) {
  return <T.Provider delayDuration={400}>{children}</T.Provider>
}

export function Tooltip({
  content,
  kbd,
  side = 'bottom',
  children,
}: {
  content: ReactNode
  kbd?: string
  side?: 'top' | 'bottom' | 'left' | 'right'
  children: ReactNode
}) {
  return (
    <T.Root>
      <T.Trigger asChild>{children}</T.Trigger>
      <T.Portal>
        <T.Content
          side={side}
          sideOffset={6}
          className="z-50 flex items-center gap-1.5 rounded-md bg-overlay px-2 py-1 text-xs text-primary shadow-(--shadow-md)"
        >
          {content}
          {kbd ? <Kbd>{kbd}</Kbd> : null}
        </T.Content>
      </T.Portal>
    </T.Root>
  )
}
