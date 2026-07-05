import { cn } from '@/lib/cn'

function classOf(status: number): { fg: string; bg: string } {
  if (status >= 500) return { fg: 'var(--danger)', bg: 'var(--danger-soft)' }
  if (status >= 400) return { fg: 'var(--warning)', bg: 'var(--warning-soft)' }
  if (status >= 300) return { fg: 'var(--info)', bg: 'var(--info-soft)' }
  if (status >= 200) return { fg: 'var(--success)', bg: 'var(--success-soft)' }
  return { fg: 'var(--text-muted)', bg: 'var(--bg-inset)' }
}

export function StatusPill({
  status,
  statusText,
  className,
}: {
  status: number
  statusText?: string
  className?: string
}) {
  const { fg, bg } = classOf(status)
  return (
    <span
      data-tabular
      className={cn(
        'inline-flex items-center gap-1 rounded-md px-2 py-0.5 font-mono text-xs font-semibold',
        className,
      )}
      style={{ color: fg, background: bg }}
    >
      {status}
      {statusText ? <span className="font-medium opacity-90">{statusText}</span> : null}
    </span>
  )
}
