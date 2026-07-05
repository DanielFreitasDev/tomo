/** The eight request sub-tab editors. Each mutates the tab draft via editTab. */
import { Plus, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { CodeEditor } from '@/components/ui/code-editor'
import { IconButton } from '@/components/ui/icon-button'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { Checkbox, Switch } from '@/components/ui/switch'
import { useT } from '@/i18n'
import type { AssertDto, AssertOp, AuthDto, BodyDto, MultipartPartDto, RequestFileDto } from '@/lib/transport'
import { editTab } from '@/stores/actions/tab-actions'
import { KeyValueEditor } from './KeyValueEditor'

interface TabProps {
  tabId: string
  collectionId: string
  request: RequestFileDto
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-3 text-xs text-secondary">
      <span className="w-32 shrink-0">{label}</span>
      {children}
    </div>
  )
}

// ---------------------------------------------------------------------------

export function ParamsTab({ tabId, collectionId, request }: TabProps) {
  const detectedPathParams = (request.http.url.match(/(?<=\/):([\w-]+)/g) ?? []).map((m) => m.slice(1))
  const path = request.http.path ?? []
  const missing = detectedPathParams.filter((name) => !path.some((p) => p.name === name))
  const shownPath = [...path, ...missing.map((name) => ({ name, value: '' }))]

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <SectionLabel text="Query" />
      <KeyValueEditor
        collectionId={collectionId}
        pairs={request.http.query ?? []}
        onChange={(query) => editTab(tabId, (d) => ({ ...d, http: { ...d.http, query } }))}
      />
      {shownPath.length > 0 ? (
        <>
          <SectionLabel text="Path" />
          <KeyValueEditor
            collectionId={collectionId}
            pairs={shownPath}
            onChange={(p) => editTab(tabId, (d) => ({ ...d, http: { ...d.http, path: p } }))}
          />
        </>
      ) : null}
    </div>
  )
}

function SectionLabel({ text }: { text: string }) {
  return <div className="px-3 pt-2 text-2xs font-semibold uppercase tracking-wide text-muted">{text}</div>
}

export function HeadersTab({ tabId, collectionId, request }: TabProps) {
  return (
    <div className="h-full overflow-y-auto">
      <KeyValueEditor
        collectionId={collectionId}
        pairs={request.http.headers ?? []}
        onChange={(headers) => editTab(tabId, (d) => ({ ...d, http: { ...d.http, headers } }))}
        keyPlaceholder="Header-Name"
      />
    </div>
  )
}

// ---------------------------------------------------------------------------

type BodyKind = 'none' | BodyDto['type']

const BODY_KINDS: { value: BodyKind; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'json', label: 'JSON' },
  { value: 'text', label: 'Text' },
  { value: 'xml', label: 'XML' },
  { value: 'form_urlencoded', label: 'Form URL-encoded' },
  { value: 'multipart_form', label: 'Multipart form' },
  { value: 'binary', label: 'Binary file' },
  { value: 'graphql', label: 'GraphQL' },
]

function defaultBody(kind: BodyKind): BodyDto | undefined {
  switch (kind) {
    case 'none':
      return undefined
    case 'json':
      return { type: 'json', content: '{\n  \n}\n' }
    case 'text':
      return { type: 'text', content: '' }
    case 'xml':
      return { type: 'xml', content: '' }
    case 'form_urlencoded':
      return { type: 'form_urlencoded', fields: [] }
    case 'multipart_form':
      return { type: 'multipart_form', parts: [] }
    case 'binary':
      return { type: 'binary', path: '' }
    case 'graphql':
      return { type: 'graphql', query: '', variables: '{}\n' }
  }
}

export function BodyTab({ tabId, collectionId, request }: TabProps) {
  const body = request.body
  const kind: BodyKind = body?.type ?? 'none'
  const setBody = (b: BodyDto | undefined) => editTab(tabId, (d) => ({ ...d, body: b }))

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 px-3 py-2">
        <Select
          size="sm"
          ariaLabel="Body type"
          value={kind}
          onChange={(k) => setBody(defaultBody(k))}
          options={BODY_KINDS}
        />
      </div>
      <div className="min-h-0 flex-1">
        {body?.type === 'json' || body?.type === 'text' || body?.type === 'xml' ? (
          <CodeEditor
            className="h-full border-t border-subtle"
            language={body.type === 'json' ? 'json' : body.type === 'xml' ? 'xml' : 'text'}
            collectionId={collectionId}
            value={body.content}
            onChange={(content) => setBody({ ...body, content })}
            ariaLabel="Request body"
          />
        ) : body?.type === 'form_urlencoded' ? (
          <div className="h-full overflow-y-auto border-t border-subtle">
            <KeyValueEditor
              collectionId={collectionId}
              pairs={body.fields}
              onChange={(fields) => setBody({ ...body, fields })}
            />
          </div>
        ) : body?.type === 'multipart_form' ? (
          <MultipartEditor parts={body.parts} onChange={(parts) => setBody({ ...body, parts })} />
        ) : body?.type === 'binary' ? (
          <div className="flex flex-col gap-2 border-t border-subtle p-3">
            <Row label="File path">
              <Input
                inputSize="sm"
                mono
                className="flex-1"
                placeholder="assets/payload.bin (relative to collection)"
                value={body.path}
                onChange={(e) => setBody({ ...body, path: e.target.value })}
              />
            </Row>
            <Row label="Content-Type">
              <Input
                inputSize="sm"
                mono
                className="flex-1"
                placeholder="application/octet-stream"
                value={body.content_type ?? ''}
                onChange={(e) => setBody({ ...body, content_type: e.target.value || undefined })}
              />
            </Row>
          </div>
        ) : body?.type === 'graphql' ? (
          <div className="grid h-full grid-rows-[2fr_1fr] border-t border-subtle">
            <CodeEditor
              language="graphql"
              collectionId={collectionId}
              value={body.query}
              onChange={(query) => setBody({ ...body, query })}
              placeholder="query { ... }"
              ariaLabel="GraphQL query"
            />
            <CodeEditor
              className="border-t border-subtle"
              language="json"
              collectionId={collectionId}
              value={body.variables ?? ''}
              onChange={(variables) => setBody({ ...body, variables })}
              placeholder='{ "id": "{{user_id}}" }'
              ariaLabel="GraphQL variables"
            />
          </div>
        ) : (
          <div className="flex h-full items-center justify-center text-xs text-muted">no body</div>
        )}
      </div>
    </div>
  )
}

function MultipartEditor({
  parts,
  onChange,
}: {
  parts: MultipartPartDto[]
  onChange: (parts: MultipartPartDto[]) => void
}) {
  const t = useT()
  const update = (i: number, patch: Partial<MultipartPartDto>) =>
    onChange(parts.map((p, idx) => (idx === i ? { ...p, ...patch } : p)))

  return (
    <div className="flex h-full flex-col gap-1 overflow-y-auto border-t border-subtle p-2">
      {parts.map((part, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: rows are positional
        <div key={i} className="flex items-center gap-1.5">
          <Checkbox
            checked={part.enabled !== false}
            onCheckedChange={(v) => update(i, { enabled: v ? undefined : false })}
            ariaLabel={t('common.enabled')}
          />
          <Input
            inputSize="sm"
            mono
            className="w-1/5"
            placeholder="name"
            value={part.name}
            onChange={(e) => update(i, { name: e.target.value })}
          />
          <Select
            size="sm"
            ariaLabel="Part kind"
            value={part.kind}
            onChange={(kind) => update(i, { kind, value: undefined, path: undefined })}
            options={[
              { value: 'text', label: 'text' },
              { value: 'file', label: 'file' },
            ]}
          />
          <Input
            inputSize="sm"
            mono
            className="flex-1"
            placeholder={part.kind === 'file' ? 'assets/file.png' : 'value'}
            value={part.kind === 'file' ? (part.path ?? '') : (part.value ?? '')}
            onChange={(e) =>
              update(i, part.kind === 'file' ? { path: e.target.value } : { value: e.target.value })
            }
          />
          <Input
            inputSize="sm"
            mono
            className="w-1/5"
            placeholder="content-type"
            value={part.content_type ?? ''}
            onChange={(e) => update(i, { content_type: e.target.value || undefined })}
          />
          <IconButton
            label={t('common.delete')}
            size="sm"
            onClick={() => onChange(parts.filter((_, x) => x !== i))}
          >
            <Trash2 size={13} />
          </IconButton>
        </div>
      ))}
      <div>
        <Button
          variant="ghost"
          size="sm"
          icon={<Plus size={12} />}
          onClick={() => onChange([...parts, { name: '', kind: 'text', value: '' }])}
        >
          {t('common.add')}
        </Button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------

type AuthKind = 'inherit' | 'none' | 'basic' | 'bearer' | 'api_key' | 'digest' | 'oauth2'

function defaultAuth(kind: AuthKind): AuthDto | undefined {
  switch (kind) {
    case 'inherit':
      return undefined
    case 'none':
      return { type: 'none' }
    case 'basic':
      return { type: 'basic', username: '', password: '' }
    case 'bearer':
      return { type: 'bearer', token: '' }
    case 'api_key':
      return { type: 'api_key', key: '', value: '' }
    case 'digest':
      return { type: 'digest', username: '', password: '' }
    case 'oauth2':
      return { type: 'oauth2', grant: 'client_credentials', token_url: '', client_id: '' }
  }
}

export function AuthTab({ tabId, request }: TabProps) {
  const auth = request.auth
  const kind: AuthKind = auth?.type ?? 'inherit'
  const setAuth = (a: AuthDto | undefined) => editTab(tabId, (d) => ({ ...d, auth: a }))
  const patch = (p: Record<string, unknown>) => setAuth({ ...(auth as AuthDto), ...p } as AuthDto)

  return (
    <div className="flex h-full flex-col gap-2 overflow-y-auto p-3">
      <Row label="Type">
        <Select
          size="sm"
          ariaLabel="Auth type"
          value={kind}
          onChange={(k) => setAuth(defaultAuth(k))}
          options={[
            { value: 'inherit', label: 'Inherit' },
            { value: 'none', label: 'None' },
            { value: 'basic', label: 'Basic' },
            { value: 'bearer', label: 'Bearer' },
            { value: 'api_key', label: 'API key' },
            { value: 'digest', label: 'Digest' },
            { value: 'oauth2', label: 'OAuth 2.0' },
          ]}
        />
      </Row>

      {auth?.type === 'basic' || auth?.type === 'digest' ? (
        <>
          <Row label="Username">
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              value={auth.username}
              onChange={(e) => patch({ username: e.target.value })}
            />
          </Row>
          <Row label="Password">
            <Input
              inputSize="sm"
              mono
              type="password"
              className="flex-1"
              value={auth.password}
              onChange={(e) => patch({ password: e.target.value })}
            />
          </Row>
        </>
      ) : auth?.type === 'bearer' ? (
        <Row label="Token">
          <Input
            inputSize="sm"
            mono
            className="flex-1"
            placeholder="{{access_token}}"
            value={auth.token}
            onChange={(e) => patch({ token: e.target.value })}
          />
        </Row>
      ) : auth?.type === 'api_key' ? (
        <>
          <Row label="Key">
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              placeholder="X-Api-Key"
              value={auth.key}
              onChange={(e) => patch({ key: e.target.value })}
            />
          </Row>
          <Row label="Value">
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              value={auth.value}
              onChange={(e) => patch({ value: e.target.value })}
            />
          </Row>
          <Row label="Placement">
            <Select
              size="sm"
              ariaLabel="Placement"
              value={auth.placement ?? 'header'}
              onChange={(placement) => patch({ placement })}
              options={[
                { value: 'header', label: 'Header' },
                { value: 'query', label: 'Query' },
              ]}
            />
          </Row>
        </>
      ) : auth?.type === 'oauth2' ? (
        <>
          <Row label="Grant">
            <Select
              size="sm"
              ariaLabel="Grant"
              value={auth.grant}
              onChange={(grant) => patch({ grant })}
              options={[
                { value: 'client_credentials', label: 'Client credentials' },
                { value: 'password', label: 'Password' },
              ]}
            />
          </Row>
          <Row label="Token URL">
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              value={auth.token_url}
              onChange={(e) => patch({ token_url: e.target.value })}
            />
          </Row>
          <Row label="Client ID">
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              value={auth.client_id}
              onChange={(e) => patch({ client_id: e.target.value })}
            />
          </Row>
          <Row label="Client secret">
            <Input
              inputSize="sm"
              mono
              type="password"
              className="flex-1"
              placeholder="{{oauth_secret}}"
              value={auth.client_secret ?? ''}
              onChange={(e) => patch({ client_secret: e.target.value || undefined })}
            />
          </Row>
          {auth.grant === 'password' ? (
            <>
              <Row label="Username">
                <Input
                  inputSize="sm"
                  mono
                  className="flex-1"
                  value={auth.username ?? ''}
                  onChange={(e) => patch({ username: e.target.value || undefined })}
                />
              </Row>
              <Row label="Password">
                <Input
                  inputSize="sm"
                  mono
                  type="password"
                  className="flex-1"
                  value={auth.password ?? ''}
                  onChange={(e) => patch({ password: e.target.value || undefined })}
                />
              </Row>
            </>
          ) : null}
          <Row label="Scopes">
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              placeholder="read write"
              value={(auth.scopes ?? []).join(' ')}
              onChange={(e) => patch({ scopes: e.target.value.split(/\s+/).filter(Boolean) })}
            />
          </Row>
          <Row label="Client auth">
            <Select
              size="sm"
              ariaLabel="Client auth"
              value={auth.client_auth ?? 'basic_header'}
              onChange={(client_auth) => patch({ client_auth })}
              options={[
                { value: 'basic_header', label: 'Basic header' },
                { value: 'body', label: 'In body' },
              ]}
            />
          </Row>
          <Row label="Cache token">
            <Switch
              checked={auth.cache_token !== false}
              onCheckedChange={(v) => patch({ cache_token: v ? undefined : false })}
              ariaLabel="Cache token"
            />
          </Row>
        </>
      ) : (
        <p className="text-xs text-muted">
          {kind === 'inherit' ? 'Uses the auth of the closest folder/collection.' : 'No authentication.'}
        </p>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------

export function ScriptsTab({ tabId, collectionId, request }: TabProps) {
  const scripts = request.scripts ?? {}
  return (
    <div className="grid h-full grid-rows-2">
      <div className="flex min-h-0 flex-col">
        <SectionLabel text="Pre-request" />
        <CodeEditor
          className="min-h-0 flex-1"
          language="javascript"
          collectionId={collectionId}
          value={scripts.pre_request ?? ''}
          onChange={(v) =>
            editTab(tabId, (d) => ({
              ...d,
              scripts: { ...d.scripts, pre_request: v || undefined },
            }))
          }
          placeholder="vars.set('token', 'abc'); req.setHeader('X-Nonce', vars.get('token'));"
          ariaLabel="Pre-request script"
        />
      </div>
      <div className="flex min-h-0 flex-col border-t border-subtle">
        <SectionLabel text="Post-response" />
        <CodeEditor
          className="min-h-0 flex-1"
          language="javascript"
          collectionId={collectionId}
          value={scripts.post_response ?? ''}
          onChange={(v) =>
            editTab(tabId, (d) => ({
              ...d,
              scripts: { ...d.scripts, post_response: v || undefined },
            }))
          }
          placeholder="test('ok', () => { expect(res.status).toBe(200); });"
          ariaLabel="Post-response script"
        />
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------

const ASSERT_OPS: AssertOp[] = [
  'eq',
  'neq',
  'gt',
  'gte',
  'lt',
  'lte',
  'contains',
  'notContains',
  'matches',
  'notMatches',
  'isDefined',
  'isUndefined',
  'isNull',
  'isNotNull',
  'in',
  'notIn',
  'length',
]

const VALUELESS: AssertOp[] = ['isDefined', 'isUndefined', 'isNull', 'isNotNull']

function parseAssertValue(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return text
  }
}

function renderAssertValue(value: unknown): string {
  if (value === undefined) return ''
  if (typeof value === 'string') return value
  return JSON.stringify(value)
}

export function TestsTab({ tabId, request }: TabProps) {
  const t = useT()
  const asserts = request.tests?.asserts ?? []
  const setAsserts = (a: AssertDto[]) => editTab(tabId, (d) => ({ ...d, tests: { ...d.tests, asserts: a } }))
  const update = (i: number, patch: Partial<AssertDto>) =>
    setAsserts(asserts.map((a, idx) => (idx === i ? { ...a, ...patch } : a)))

  return (
    <div className="flex h-full flex-col gap-1 overflow-y-auto p-2">
      {asserts.map((assert, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: rows are positional
        <div key={i} className="flex items-center gap-1.5">
          <Checkbox
            checked={assert.enabled !== false}
            onCheckedChange={(v) => update(i, { enabled: v ? undefined : false })}
            ariaLabel={t('common.enabled')}
          />
          <Input
            inputSize="sm"
            mono
            className="w-2/5"
            placeholder="res.body.id"
            value={assert.expr}
            onChange={(e) => update(i, { expr: e.target.value })}
          />
          <Select
            size="sm"
            ariaLabel="Operator"
            value={assert.op}
            onChange={(op) => update(i, { op })}
            options={ASSERT_OPS.map((op) => ({ value: op, label: op }))}
          />
          {!VALUELESS.includes(assert.op) ? (
            <Input
              inputSize="sm"
              mono
              className="flex-1"
              placeholder='201, "text", [1,2]'
              value={renderAssertValue(assert.value)}
              onChange={(e) => update(i, { value: parseAssertValue(e.target.value) })}
            />
          ) : (
            <div className="flex-1" />
          )}
          <IconButton
            label={t('common.delete')}
            size="sm"
            onClick={() => setAsserts(asserts.filter((_, x) => x !== i))}
          >
            <Trash2 size={13} />
          </IconButton>
        </div>
      ))}
      <div>
        <Button
          variant="ghost"
          size="sm"
          icon={<Plus size={12} />}
          onClick={() => setAsserts([...asserts, { expr: 'res.status', op: 'eq', value: 200 }])}
        >
          {t('common.add')}
        </Button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------

export function DocsTab({ tabId, collectionId, request }: TabProps) {
  return (
    <CodeEditor
      className="h-full"
      collectionId={collectionId}
      value={request.docs?.content ?? ''}
      onChange={(v) => editTab(tabId, (d) => ({ ...d, docs: v ? { content: v } : undefined }))}
      placeholder="# Notes (markdown)"
      ariaLabel="Request docs"
    />
  )
}

// ---------------------------------------------------------------------------

export function OptionsTab({ tabId, request }: TabProps) {
  const t = useT()
  const options = request.options ?? {}
  const patch = (p: Partial<NonNullable<RequestFileDto['options']>>) =>
    editTab(tabId, (d) => ({ ...d, options: { ...d.options, ...p } }))

  return (
    <div className="flex h-full flex-col gap-2.5 overflow-y-auto p-3">
      <Row label={t('settings.timeout')}>
        <Input
          inputSize="sm"
          mono
          className="w-32"
          type="number"
          placeholder="30000"
          value={options.timeout_ms ?? ''}
          onChange={(e) => patch({ timeout_ms: e.target.value ? Number(e.target.value) : undefined })}
        />
      </Row>
      <Row label={t('settings.followRedirects')}>
        <TriState
          value={options.follow_redirects}
          onChange={(follow_redirects) => patch({ follow_redirects })}
        />
      </Row>
      <Row label={t('settings.maxRedirects')}>
        <Input
          inputSize="sm"
          mono
          className="w-32"
          type="number"
          placeholder="10"
          value={options.max_redirects ?? ''}
          onChange={(e) => patch({ max_redirects: e.target.value ? Number(e.target.value) : undefined })}
        />
      </Row>
      <Row label={t('settings.sslVerify')}>
        <TriState value={options.ssl_verify} onChange={(ssl_verify) => patch({ ssl_verify })} />
      </Row>
    </div>
  )
}

function TriState({
  value,
  onChange,
}: {
  value: boolean | undefined
  onChange: (v: boolean | undefined) => void
}) {
  const current = value === undefined ? 'default' : value ? 'on' : 'off'
  return (
    <Select
      size="sm"
      ariaLabel="Override"
      value={current}
      onChange={(v) => onChange(v === 'default' ? undefined : v === 'on')}
      options={[
        { value: 'default', label: 'Default' },
        { value: 'on', label: 'On' },
        { value: 'off', label: 'Off' },
      ]}
    />
  )
}
