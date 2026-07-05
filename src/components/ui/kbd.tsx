import { cn } from '@/lib/cn'

export function Kbd({ children, className }: { children: string; className?: string }) {
  return (
    <kbd
      className={cn(
        'inline-flex h-4.5 min-w-4.5 items-center justify-center rounded-sm border border-default bg-inset px-1',
        'font-mono text-2xs text-muted',
        className,
      )}
    >
      {children}
    </kbd>
  )
}
