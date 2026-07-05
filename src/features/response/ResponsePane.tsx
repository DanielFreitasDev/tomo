import { Check, Copy, Download, SendHorizonal } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { CodeEditor, type EditorLanguage } from '@/components/ui/code-editor'
import { EmptyState } from '@/components/ui/empty-state'
import { IconButton } from '@/components/ui/icon-button'
import { Kbd } from '@/components/ui/kbd'
import { Spinner } from '@/components/ui/spinner'
import { StatusPill } from '@/components/ui/status-pill'
import { Segmented, TabPanel, UnderlineTabs } from '@/components/ui/tabs'
import { useT } from '@/i18n'
import { cn } from '@/lib/cn'
import { isTauri, type ResponseMetaDto, transport } from '@/lib/transport'
import { cancelActiveRequest } from '@/stores/actions/request-actions'
import { useResponses } from '@/stores/responses'
import type { Tab } from '@/stores/tabs'

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(1)} MB`
}

const PRETTY_MAX = 2 * 1024 * 1024
const HIGHLIGHT_MAX = 5 * 1024 * 1024
const MOUNT_MAX = 20 * 1024 * 1024
const HEAD_PREVIEW = 64 * 1024

const IDLE_STATE = { phase: 'idle' } as const

type BodyView = 'pretty' | 'raw' | 'preview'
type RespTab = 'body' | 'headers' | 'tests'

export function ResponsePane({ tab }: { tab: Tab }) {
  const t = useT()
  const state = useResponses((s) => s.byTab[tab.id] ?? (IDLE_STATE as never))
  const [view, setView] = useState<BodyView>('pretty')
  const [respTab, setRespTab] = useState<RespTab>('body')
  const [copied, setCopied] = useState(false)

  const meta = state.phase === 'done' ? state.meta : undefined
  const bytes = state.phase === 'done' ? state.bodyBytes : undefined
  const runId = state.phase === 'done' ? state.runId : undefined

  const bodyText = useMemo(() => {
    if (!bytes || !meta) return ''
    if (bytes.byteLength > MOUNT_MAX) return ''
    return new TextDecoder(meta.body.charset ?? 'utf-8', { fatal: false }).decode(bytes)
  }, [bytes, meta])

  const prettyText = useMemo(() => {
    if (!meta || !bodyText) return bodyText
    const isJson = meta.body.mime?.includes('json')
    if (isJson && meta.body.total_size <= PRETTY_MAX && !meta.body.truncated) {
      try {
        return JSON.stringify(JSON.parse(bodyText), null, 2)
      } catch {
        return bodyText
      }
    }
    return bodyText
  }, [meta, bodyText])

  if (state.phase === 'idle') {
    return (
      <EmptyState
        icon={SendHorizonal}
        title={t('response.empty.title')}
        hint={
          <span className="inline-flex items-center gap-1">
            <Kbd>Ctrl</Kbd>+<Kbd>⏎</Kbd> · {t('response.empty.hint')}
          </span>
        }
      />
    )
  }

  if (state.phase === 'sending') {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3">
        <Spinner size={20} className="text-accent-text" />
        <div className="text-xs text-muted">{t('response.sending')}</div>
        <Button variant="secondary" size="sm" onClick={() => void cancelActiveRequest(tab.id)}>
          {t('request.cancel')}
        </Button>
      </div>
    )
  }

  if (state.phase === 'cancelled') return <EmptyState title={t('response.cancelled')} />

  if (state.phase === 'error') {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
        <div className="text-sm font-medium text-(--danger)">{t('toast.error')}</div>
        <div className="max-w-96 font-mono text-xs text-secondary" data-selectable>
          {state.error}
        </div>
      </div>
    )
  }

  if (!meta) return null

  const testsTotal = meta.tests.length + meta.asserts.length
  const testsFailed = meta.tests.filter((x) => !x.ok).length + meta.asserts.filter((x) => !x.ok).length
  const language: EditorLanguage = meta.body.mime?.includes('json')
    ? 'json'
    : meta.body.mime?.includes('xml')
      ? 'xml'
      : meta.body.mime?.includes('html')
        ? 'html'
        : 'text'

  const copyBody = async () => {
    await navigator.clipboard.writeText(bodyText)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  const downloadBody = async () => {
    if (!bytes) return
    if (isTauri()) {
      if (!runId || !meta.body.can_download_full) return
      const dest = await transport().invoke('pick_save_file', { default_name: responseFileName(meta) })
      if (!dest) return
      await transport().invoke('save_response_body', { run_id: runId, dest })
      return
    }
    const blob = new Blob([bytes.slice() as unknown as BlobPart], {
      type: meta.body.mime ?? 'application/octet-stream',
    })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = responseFileName(meta)
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex shrink-0 items-center gap-3 px-3 py-2">
        <StatusPill status={meta.status} statusText={meta.status_text} />
        <span className="text-xs text-muted" data-tabular>
          {meta.timing.total_ms} ms
        </span>
        <span className="text-xs text-muted" data-tabular>
          {formatBytes(meta.body.total_size)}
        </span>
        <div className="min-w-0 flex-1" />
        <IconButton
          label={copied ? t('common.copied') : t('common.copy')}
          size="sm"
          disabled={!bodyText}
          onClick={() => void copyBody()}
        >
          {copied ? <Check size={13} style={{ color: 'var(--success)' }} /> : <Copy size={13} />}
        </IconButton>
        <IconButton
          label={t('common.download')}
          size="sm"
          disabled={!meta.body.can_download_full}
          onClick={() => void downloadBody()}
        >
          <Download size={13} />
        </IconButton>
      </div>

      {meta.script_error ? <Banner tone="danger" text={`script: ${meta.script_error}`} /> : null}
      {meta.warnings.length > 0 ? (
        <Banner
          tone="warning"
          text={meta.warnings.map((w) => `{{${w.name}}} ${w.kind.replace('_', ' ')}`).join(' · ')}
        />
      ) : null}
      {meta.body.truncated ? (
        <Banner tone="warning" text={t('response.truncated', { size: formatBytes(meta.body.total_size) })} />
      ) : null}

      <UnderlineTabs<RespTab>
        className="min-h-0 flex-1"
        value={respTab}
        onChange={setRespTab}
        tabs={[
          { value: 'body', label: t('response.tab.body') },
          { value: 'headers', label: t('response.tab.headers'), badge: <Badge>{meta.headers.length}</Badge> },
          {
            value: 'tests',
            label: t('response.tab.tests'),
            badge:
              testsTotal > 0 ? (
                <Badge tone={testsFailed > 0 ? 'danger' : 'success'}>
                  {testsFailed > 0 ? `${testsFailed}✗` : `${testsTotal}✓`}
                </Badge>
              ) : undefined,
          },
        ]}
      >
        <TabPanel value="body" className="flex min-h-0 flex-col">
          <div className="flex shrink-0 items-center px-3 py-1.5">
            <Segmented
              value={view}
              onChange={setView}
              options={[
                { value: 'pretty', label: t('response.view.pretty') },
                { value: 'raw', label: t('response.view.raw') },
                { value: 'preview', label: t('response.view.preview') },
              ]}
            />
          </div>
          <div className="min-h-0 flex-1 border-t border-subtle">
            <BodyView
              view={view}
              meta={meta}
              bytes={bytes}
              bodyText={bodyText}
              prettyText={prettyText}
              language={language}
            />
          </div>
        </TabPanel>

        <TabPanel value="headers" className="overflow-y-auto">
          <table className="w-full text-xs" data-selectable>
            <tbody>
              {meta.headers.map(([k, v], i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: duplicate headers are legal
                <tr key={i} className="border-b border-subtle last:border-0">
                  <td className="w-1/3 px-3 py-1.5 align-top font-mono font-medium text-accent-text">{k}</td>
                  <td className="break-all px-3 py-1.5 font-mono text-primary">{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </TabPanel>

        <TabPanel value="tests" className="overflow-y-auto p-3">
          <TestsView meta={meta} />
        </TabPanel>
      </UnderlineTabs>
    </div>
  )
}

function Banner({ tone, text }: { tone: 'warning' | 'danger'; text: string }) {
  return (
    <div
      className={cn(
        'shrink-0 truncate border-y px-3 py-1 text-2xs',
        tone === 'danger' ? 'bg-danger-soft text-(--danger)' : 'bg-warning-soft text-(--warning)',
      )}
      style={{ borderColor: tone === 'danger' ? 'var(--danger)' : 'var(--warning)' }}
      title={text}
    >
      {text}
    </div>
  )
}

function BodyView({
  view,
  meta,
  bytes,
  bodyText,
  prettyText,
  language,
}: {
  view: BodyView
  meta: ResponseMetaDto
  bytes?: Uint8Array
  bodyText: string
  prettyText: string
  language: EditorLanguage
}) {
  if (meta.body.total_size === 0) {
    return <div className="flex h-full items-center justify-center text-xs text-muted">empty body</div>
  }
  if (meta.body.is_binary && view !== 'preview') {
    return (
      <div className="flex h-full items-center justify-center text-xs text-muted">
        binary body · {formatBytes(meta.body.total_size)} — download to inspect
      </div>
    )
  }
  if ((bytes?.byteLength ?? 0) > MOUNT_MAX) {
    return (
      <pre data-selectable className="h-full overflow-auto p-3 font-mono text-xs text-primary">
        {new TextDecoder().decode(new Uint8Array())}
        (body larger than {formatBytes(MOUNT_MAX)} — showing nothing; use download)
      </pre>
    )
  }

  if (view === 'preview') {
    if (meta.body.mime?.startsWith('image/')) {
      return bytes ? <ImagePreview mime={meta.body.mime} bytes={bytes} /> : null
    }
    if (meta.body.mime?.includes('html')) {
      return (
        <iframe
          title="HTML preview"
          sandbox=""
          srcDoc={bodyText.slice(0, HEAD_PREVIEW * 4)}
          className="h-full w-full bg-white"
        />
      )
    }
    return (
      <div className="flex h-full items-center justify-center text-xs text-muted">
        no preview for {meta.body.mime ?? 'unknown type'}
      </div>
    )
  }

  // raw / pretty text views with size guardrails
  if ((bytes?.byteLength ?? 0) > HIGHLIGHT_MAX || meta.body.truncated) {
    return (
      <pre data-selectable className="h-full overflow-auto p-3 font-mono text-xs leading-5 text-primary">
        {bodyText.slice(0, HEAD_PREVIEW)}
        {bodyText.length > HEAD_PREVIEW || meta.body.truncated
          ? '\n... (preview shown - download for the full body)'
          : ''}
      </pre>
    )
  }

  return (
    <CodeEditor
      readOnly
      className="h-full"
      language={view === 'pretty' ? language : 'text'}
      value={view === 'pretty' ? prettyText : bodyText}
      ariaLabel="Response body"
    />
  )
}

function ImagePreview({ mime, bytes }: { mime: string; bytes: Uint8Array }) {
  const [src, setSrc] = useState<string | null>(null)

  useEffect(() => {
    const url = URL.createObjectURL(
      new Blob([bytes.slice() as unknown as BlobPart], {
        type: mime,
      }),
    )
    setSrc(url)
    return () => URL.revokeObjectURL(url)
  }, [bytes, mime])

  if (!src) return null
  return (
    <div className="flex h-full items-center justify-center overflow-auto bg-inset p-4">
      <img src={src} alt="Response preview" className="max-h-full max-w-full" />
    </div>
  )
}

function responseFileName(meta: ResponseMetaDto): string {
  const urlName = meta.final_url.split(/[/?#]/).filter(Boolean).pop()
  const ext = meta.body.mime?.includes('json')
    ? 'json'
    : meta.body.mime?.includes('html')
      ? 'html'
      : meta.body.mime?.includes('xml')
        ? 'xml'
        : meta.body.mime?.startsWith('image/')
          ? meta.body.mime.slice('image/'.length)
          : 'bin'
  const base = (urlName || 'response').replace(/[^a-zA-Z0-9._-]+/g, '-').replace(/^-+|-+$/g, '')
  return base.includes('.') ? base : `${base || 'response'}.${ext}`
}

function TestsView({ meta }: { meta: ResponseMetaDto }) {
  if (meta.tests.length === 0 && meta.asserts.length === 0 && meta.console.length === 0) {
    return <div className="text-xs text-muted">no tests or asserts on this request</div>
  }
  return (
    <div className="flex flex-col gap-3">
      {meta.tests.length > 0 ? (
        <section className="flex flex-col gap-1">
          <h3 className="text-2xs font-semibold uppercase tracking-wide text-muted">tests</h3>
          {meta.tests.map((test, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: results are positional
            <div key={i} className="flex items-start gap-2 text-xs">
              <span style={{ color: test.ok ? 'var(--success)' : 'var(--danger)' }}>
                {test.ok ? '✓' : '✗'}
              </span>
              <div className="min-w-0">
                <div className="text-primary">{test.name}</div>
                {test.message ? (
                  <div className="font-mono text-2xs text-secondary">{test.message}</div>
                ) : null}
              </div>
            </div>
          ))}
        </section>
      ) : null}

      {meta.asserts.length > 0 ? (
        <section className="flex flex-col gap-1">
          <h3 className="text-2xs font-semibold uppercase tracking-wide text-muted">asserts</h3>
          {meta.asserts.map((a, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: results are positional
            <div key={i} className="flex items-start gap-2 text-xs">
              <span style={{ color: a.ok ? 'var(--success)' : 'var(--danger)' }}>{a.ok ? '✓' : '✗'}</span>
              <div className="min-w-0 font-mono">
                <span className="text-primary">
                  {a.expr} {a.op} {a.expected !== undefined ? JSON.stringify(a.expected) : ''}
                </span>
                {!a.ok && a.message ? <div className="text-2xs text-secondary">{a.message}</div> : null}
              </div>
            </div>
          ))}
        </section>
      ) : null}

      {meta.console.length > 0 ? (
        <section className="flex flex-col gap-0.5">
          <h3 className="text-2xs font-semibold uppercase tracking-wide text-muted">console</h3>
          {meta.console.map((line, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: log lines are positional
            <div key={i} className="font-mono text-2xs" data-selectable>
              <span
                className="mr-2 uppercase"
                style={{
                  color:
                    line.level === 'error'
                      ? 'var(--danger)'
                      : line.level === 'warn'
                        ? 'var(--warning)'
                        : 'var(--text-muted)',
                }}
              >
                {line.level}
              </span>
              <span className="text-secondary">{line.message}</span>
            </div>
          ))}
        </section>
      ) : null}
    </div>
  )
}
