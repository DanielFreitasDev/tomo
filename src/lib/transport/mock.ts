/**
 * In-memory transport for browser dev and Playwright e2e: a fixture workspace
 * (including a 1000-node folder for virtualization tests), scripted HTTP
 * responses with fake latency, and localStorage-backed settings.
 */
import {
  type CollectionTreeDto,
  type Commands,
  defaultSettings,
  type EnvironmentDto,
  type Events,
  type RequestFileDto,
  type ResponseMetaDto,
  type SecretsDto,
  type Transport,
  TransportError,
} from './contract'

interface MockFile {
  request: RequestFileDto
  hash: number
}

interface MockCollection {
  id: string
  name: string
  path: string
  files: Map<string, MockFile> // rel -> file
  folders: Map<string, { name: string; seq?: number }> // rel -> meta
  environments: Map<string, EnvironmentDto>
  secrets: SecretsDto
  selectedEnv?: string
}

type Listener = (payload: unknown) => void

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

function makeRequest(name: string, method = 'GET', url = 'https://httpbin.org/get'): RequestFileDto {
  return { meta: { name }, http: { method, url } }
}

export function createMockTransport(): Transport {
  const listeners = new Map<string, Set<Listener>>()
  const collections = new Map<string, MockCollection>()
  const bodies = new Map<string, Uint8Array>()
  const inflight = new Map<string, { cancelled: boolean }>()
  let hashCounter = 1

  const emit = <E extends keyof Events>(event: E, payload: Events[E]) => {
    for (const l of listeners.get(event) ?? []) l(payload)
  }

  // ---- fixture workspace --------------------------------------------------
  const fixture = (): MockCollection => {
    const files = new Map<string, MockFile>()
    const folders = new Map<string, { name: string; seq?: number }>()

    files.set('health.toml', {
      request: makeRequest('Health check', 'GET', '{{base_url}}/status/200'),
      hash: hashCounter++,
    })
    folders.set('users', { name: 'Users', seq: 1 })
    files.set('users/list-users.toml', {
      request: {
        ...makeRequest('List users', 'GET', '{{base_url}}/anything/users'),
        meta: { name: 'List users', seq: 1 },
      },
      hash: hashCounter++,
    })
    files.set('users/create-user.toml', {
      request: {
        meta: { name: 'Create user', seq: 2 },
        http: {
          method: 'POST',
          url: '{{base_url}}/anything/users',
          headers: [{ name: 'X-Trace', value: '{{$uuid}}' }],
        },
        body: { type: 'json', content: '{\n  "name": "Ada"\n}\n' },
        tests: { asserts: [{ expr: 'res.status', op: 'eq', value: 200 }] },
      },
      hash: hashCounter++,
    })

    // big folder to exercise virtualization
    folders.set('generated', { name: 'Generated (1000)', seq: 99 })
    for (let i = 0; i < 1000; i++) {
      const n = String(i).padStart(4, '0')
      files.set(`generated/req-${n}.toml`, {
        request: {
          meta: { name: `Generated ${n}`, seq: i },
          http: { method: i % 5 === 0 ? 'POST' : 'GET', url: `https://api.test/items/${i}` },
        },
        hash: hashCounter++,
      })
    }

    const environments = new Map<string, EnvironmentDto>()
    environments.set('dev', {
      meta: { name: 'dev', secrets: ['api_key'] },
      vars: { base_url: 'https://httpbin.org', who: 'dev' },
    })
    environments.set('prod', {
      meta: { name: 'prod', secrets: ['api_key'] },
      vars: { base_url: 'https://httpbin.org', who: 'prod' },
    })

    return {
      id: '/mock/acme-api',
      name: 'Acme API',
      path: '/mock/acme-api',
      files,
      folders,
      environments,
      secrets: { collection: {}, environments: { dev: { api_key: 'sk-dev-123' } } },
      selectedEnv: 'dev',
    }
  }

  const treeOf = (c: MockCollection): CollectionTreeDto => {
    type N = CollectionTreeDto['nodes'][number]
    const roots: N[] = []
    const folderNodes = new Map<string, N>()

    for (const [rel, meta] of c.folders) {
      folderNodes.set(rel, { kind: 'folder', rel, name: meta.name, seq: meta.seq, children: [] })
    }
    const parentOf = (rel: string): N[] => {
      const idx = rel.lastIndexOf('/')
      if (idx === -1) return roots
      const parent = folderNodes.get(rel.slice(0, idx))
      return parent?.children ?? roots
    }
    for (const [rel, node] of folderNodes) parentOf(rel).push(node)
    for (const [rel, file] of c.files) {
      parentOf(rel).push({
        kind: 'request',
        rel,
        name: file.request.meta.name,
        seq: file.request.meta.seq,
        method: file.request.http.method,
      })
    }
    const sortRec = (nodes: N[]) => {
      nodes.sort(
        (a, b) =>
          (a.seq ?? Number.MAX_SAFE_INTEGER) - (b.seq ?? Number.MAX_SAFE_INTEGER) ||
          a.rel.localeCompare(b.rel),
      )
      for (const n of nodes) if (n.children) sortRec(n.children)
    }
    sortRec(roots)
    return {
      id: c.id,
      name: c.name,
      path: c.path,
      nodes: roots,
      invalid: [],
      environments: [...c.environments.keys()],
      selected_environment: c.selectedEnv,
    }
  }

  const need = (id: string): MockCollection => {
    const c = collections.get(id)
    if (!c) throw new TransportError('not_found', `collection not open: ${id}`)
    return c
  }

  const slug = (name: string) =>
    name
      .toLowerCase()
      .normalize('NFD')
      .replace(/[̀-ͯ]/g, '')
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'request'

  // ---- scripted HTTP -------------------------------------------------------
  const respond = (req: RequestFileDto): ResponseMetaDto => {
    const url = req.http.url
    const status = url.includes('/status/500') ? 500 : url.includes('/status/404') ? 404 : 200
    const bodyObj = {
      ok: status === 200,
      method: req.http.method,
      url,
      echo: req.body?.type === 'json' ? safeJson(req.body.content) : undefined,
    }
    const bytes = new TextEncoder().encode(JSON.stringify(bodyObj, null, 2))
    return {
      status,
      status_text: status === 200 ? 'OK' : status === 404 ? 'Not Found' : 'Internal Server Error',
      http_version: 'HTTP/1.1',
      headers: [
        ['content-type', 'application/json'],
        ['x-mock', 'true'],
      ],
      final_url: url,
      timing: { total_ms: 134, ttfb_ms: 90, download_ms: 44 },
      body: {
        total_size: bytes.length,
        truncated: false,
        mime: 'application/json',
        is_binary: false,
      },
      warnings: [],
      console: [],
      tests: [],
      asserts: (req.tests?.asserts ?? [])
        .filter((a) => a.enabled !== false)
        .map((a) => ({ ...a, ok: true, actual: status, expected: a.value })),
      cookies: [],
      _bytes: bytes,
    } as ResponseMetaDto & { _bytes: Uint8Array }
  }

  const safeJson = (s: string): unknown => {
    try {
      return JSON.parse(s)
    } catch {
      return s
    }
  }

  const settingsKey = 'tomo-mock-settings'
  const uiKey = (id?: string) => `tomo-mock-ui-${id ?? 'app'}`

  const handlers: { [K in keyof Commands]: (args: Commands[K]['args']) => Promise<Commands[K]['result']> } = {
    pick_collection_folder: async () => '/mock/acme-api',
    open_collection: async ({ path }) => {
      const c = [...collections.values()].find((c) => c.path === path) ?? fixture()
      collections.set(c.id, c)
      return treeOf(c)
    },
    create_collection: async ({ name }) => {
      const id = `/mock/${slug(name)}`
      const c: MockCollection = {
        id,
        name,
        path: id,
        files: new Map(),
        folders: new Map(),
        environments: new Map(),
        secrets: { collection: {}, environments: {} },
      }
      collections.set(id, c)
      return treeOf(c)
    },
    close_collection: async ({ id }) => {
      collections.delete(id)
      return null
    },
    reload_collection: async ({ id }) => treeOf(need(id)),
    list_recent_collections: async () => [{ path: '/mock/acme-api', name: 'Acme API' }],

    create_folder: async ({ id, parent_rel, name }) => {
      const c = need(id)
      const rel = parent_rel ? `${parent_rel}/${slug(name)}` : slug(name)
      c.folders.set(rel, { name })
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return rel
    },
    create_request: async ({ id, parent_rel, name }) => {
      const c = need(id)
      const base = parent_rel ? `${parent_rel}/${slug(name)}` : slug(name)
      let rel = `${base}.toml`
      let n = 2
      while (c.files.has(rel)) rel = `${base}-${n++}.toml`
      c.files.set(rel, { request: makeRequest(name, 'GET', ''), hash: hashCounter++ })
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return rel
    },
    read_request: async ({ id, rel }) => {
      const f = need(id).files.get(rel)
      if (!f) throw new TransportError('not_found', `no such request: ${rel}`)
      return { request: structuredClone(f.request), hash: String(f.hash) }
    },
    save_request: async ({ id, rel, request, base_hash }) => {
      const c = need(id)
      const f = c.files.get(rel)
      if (!f) throw new TransportError('not_found', `no such request: ${rel}`)
      if (base_hash !== undefined && base_hash !== String(f.hash)) {
        return {
          outcome: 'conflict',
          current_text: JSON.stringify(f.request, null, 2),
          current_hash: String(f.hash),
        }
      }
      f.request = structuredClone(request)
      f.hash = hashCounter++
      return { outcome: 'saved', hash: String(f.hash) }
    },
    rename_node: async ({ id, rel, new_name, kind }) => {
      const c = need(id)
      if (kind === 'request') {
        const f = c.files.get(rel)
        if (!f) throw new TransportError('not_found', rel)
        f.request.meta.name = new_name
        const dir = rel.includes('/') ? rel.slice(0, rel.lastIndexOf('/') + 1) : ''
        const newRel = `${dir}${slug(new_name)}.toml`
        if (newRel !== rel && !c.files.has(newRel)) {
          c.files.delete(rel)
          c.files.set(newRel, f)
          emit('watcher:tree-changed', { id, tree: treeOf(c) })
          return newRel
        }
        emit('watcher:tree-changed', { id, tree: treeOf(c) })
        return rel
      }
      const meta = c.folders.get(rel)
      if (!meta) throw new TransportError('not_found', rel)
      meta.name = new_name
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return rel
    },
    move_node: async ({ id, rel, new_parent_rel }) => {
      const c = need(id)
      const base = rel.split('/').pop() ?? rel
      const newRel = new_parent_rel ? `${new_parent_rel}/${base}` : base
      const f = c.files.get(rel)
      if (f) {
        c.files.delete(rel)
        c.files.set(newRel, f)
      } else if (c.folders.has(rel)) {
        // move folder + descendants
        const meta = c.folders.get(rel)
        if (!meta) throw new TransportError('not_found', rel)
        c.folders.delete(rel)
        c.folders.set(newRel, meta)
        for (const [r, file] of [...c.files]) {
          if (r.startsWith(`${rel}/`)) {
            c.files.delete(r)
            c.files.set(newRel + r.slice(rel.length), file)
          }
        }
      }
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return newRel
    },
    duplicate_request: async ({ id, rel }) => {
      const c = need(id)
      const f = c.files.get(rel)
      if (!f) throw new TransportError('not_found', rel)
      const copy = structuredClone(f.request)
      copy.meta.name = `${copy.meta.name} (copy)`
      const newRel = rel.replace(/\.toml$/, '-copy.toml')
      c.files.set(newRel, { request: copy, hash: hashCounter++ })
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return newRel
    },
    delete_node: async ({ id, rel }) => {
      const c = need(id)
      c.files.delete(rel)
      c.folders.delete(rel)
      for (const r of [...c.files.keys()]) if (r.startsWith(`${rel}/`)) c.files.delete(r)
      for (const r of [...c.folders.keys()]) if (r.startsWith(`${rel}/`)) c.folders.delete(r)
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return null
    },
    reorder_nodes: async ({ id, ordered_rels }) => {
      const c = need(id)
      ordered_rels.forEach((rel, i) => {
        const f = c.files.get(rel)
        if (f) f.request.meta.seq = i + 1
        const folder = c.folders.get(rel)
        if (folder) folder.seq = i + 1
      })
      emit('watcher:tree-changed', { id, tree: treeOf(c) })
      return null
    },

    read_environment: async ({ id, name }) => {
      const env = need(id).environments.get(name)
      if (!env) throw new TransportError('not_found', `no such environment: ${name}`)
      return structuredClone(env)
    },
    save_environment: async ({ id, name, env, previous_name }) => {
      const c = need(id)
      if (previous_name && previous_name !== name) c.environments.delete(previous_name)
      c.environments.set(name, structuredClone(env))
      return null
    },
    delete_environment: async ({ id, name }) => {
      need(id).environments.delete(name)
      return null
    },
    select_environment: async ({ id, name }) => {
      need(id).selectedEnv = name ?? undefined
      return null
    },
    read_secrets: async ({ id }) => structuredClone(need(id).secrets),
    save_secrets: async ({ id, secrets }) => {
      need(id).secrets = structuredClone(secrets)
      return null
    },

    send_request: async ({ id, rel, run_id, draft }) => {
      const c = need(id)
      const req = draft ?? c.files.get(rel)?.request
      if (!req) throw new TransportError('not_found', rel)
      inflight.set(run_id, { cancelled: false })
      emit('request:started', { run_id })

      const slow = req.http.url.includes('/delay/')
      const latency = slow ? 5_000 : 120 + Math.random() * 200
      const step = 50
      for (let waited = 0; waited < latency; waited += step) {
        await sleep(Math.min(step, latency - waited))
        if (inflight.get(run_id)?.cancelled) {
          inflight.delete(run_id)
          emit('request:cancelled', { run_id })
          throw new TransportError('cancelled', 'request cancelled')
        }
      }
      inflight.delete(run_id)

      if (req.http.url.includes('unreachable')) {
        emit('request:failed', { run_id, error: 'dns error: name not resolved' })
        throw new TransportError('http', 'dns error: name not resolved')
      }

      const meta = respond(req) as ResponseMetaDto & { _bytes: Uint8Array }
      bodies.set(run_id, meta._bytes)
      const { _bytes, ...clean } = meta
      emit('request:completed', { run_id, meta: clean })
      return clean
    },
    cancel_request: async ({ run_id }) => {
      const entry = inflight.get(run_id)
      if (entry) entry.cancelled = true
      return Boolean(entry)
    },
    get_response_body: async ({ run_id }) => bodies.get(run_id) ?? new Uint8Array(),
    save_response_body: async () => null,
    get_cookies: async () => [],
    clear_cookies: async () => null,
    get_runtime_vars: async () => ({}),
    set_runtime_var: async () => null,
    clear_runtime_vars: async () => null,

    import_curl: async ({ text }) => {
      const url = text.match(/https?:\/\/\S+/)?.[0]?.replace(/['"]/g, '') ?? ''
      const method = text.includes('-X POST') || text.includes('--data') ? 'POST' : 'GET'
      return makeRequest('Imported from curl', method, url)
    },
    export_curl: async ({ id, rel, draft }) => {
      const req = draft ?? need(id).files.get(rel)?.request
      if (!req) throw new TransportError('not_found', rel)
      return `curl -X ${req.http.method} '${req.http.url}'`
    },

    get_settings: async () => {
      const raw = localStorage.getItem(settingsKey)
      return raw ? { ...defaultSettings(), ...JSON.parse(raw) } : defaultSettings()
    },
    save_settings: async ({ settings }) => {
      localStorage.setItem(settingsKey, JSON.stringify(settings))
      return null
    },
    get_ui_state: async ({ id }) => {
      const raw = localStorage.getItem(uiKey(id))
      return raw ? JSON.parse(raw) : null
    },
    save_ui_state: async ({ id, state_json }) => {
      localStorage.setItem(uiKey(id), JSON.stringify(state_json))
      return null
    },
  }

  return {
    async invoke(cmd, args) {
      const handler = handlers[cmd]
      if (!handler) throw new TransportError('unknown_command', String(cmd))
      return handler(args) as never
    },
    listen(event, handler) {
      if (!listeners.has(event)) listeners.set(event, new Set())
      const set = listeners.get(event)
      set?.add(handler as Listener)
      return () => set?.delete(handler as Listener)
    },
  }
}
