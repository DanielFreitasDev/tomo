import { cn } from '@/lib/cn'

const KNOWN = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const

export function methodColorVar(method: string): string {
  const m = method.toUpperCase()
  return (KNOWN as readonly string[]).includes(m)
    ? `var(--method-${m.toLowerCase()})`
    : 'var(--method-custom)'
}

/** Abbreviations keep the tree column narrow and aligned. */
function short(method: string): string {
  const m = method.toUpperCase()
  switch (m) {
    case 'DELETE':
      return 'DEL'
    case 'OPTIONS':
      return 'OPT'
    case 'PATCH':
      return 'PAT'
    default:
      return m.slice(0, 5)
  }
}

export function MethodBadge({
  method,
  block = false,
  className,
}: {
  method: string
  /** soft pill variant for dense lists / palette */
  block?: boolean
  className?: string
}) {
  return (
    <span
      className={cn(
        'inline-block shrink-0 select-none text-right font-mono text-2xs font-bold uppercase tracking-wide',
        block ? 'w-auto rounded-sm bg-inset px-1.5 py-0.5 text-left' : 'w-9',
        className,
      )}
      style={{ color: methodColorVar(method) }}
    >
      {short(method)}
    </span>
  )
}
