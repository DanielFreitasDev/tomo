/**
 * The single source of truth for the Rust boundary. DTO field names mirror
 * serde's snake_case wire format exactly. `tauri.ts` maps 1:1 onto invoke();
 * `mock/` implements the same surface in-memory for browser dev and e2e.
 */

// ---------------------------------------------------------------------------
// model DTOs (mirror crates/core/src/model)
// ---------------------------------------------------------------------------

export interface Pair {
  name: string
  value: string
  enabled?: boolean // absent = true
}

export type BodyDto =
  | { type: 'json'; content: string }
  | { type: 'text'; content: string }
  | { type: 'xml'; content: string }
  | { type: 'form_urlencoded'; fields: Pair[] }
  | { type: 'multipart_form'; parts: MultipartPartDto[] }
  | { type: 'binary'; path: string; content_type?: string }
  | { type: 'graphql'; query: string; variables?: string }

export interface MultipartPartDto {
  name: string
  kind: 'text' | 'file'
  value?: string
  path?: string
  content_type?: string
  enabled?: boolean
}

export type AuthDto =
  | { type: 'none' }
  | { type: 'inherit' }
  | { type: 'basic'; username: string; password: string }
  | { type: 'bearer'; token: string }
  | { type: 'api_key'; key: string; value: string; placement?: 'header' | 'query' }
  | { type: 'digest'; username: string; password: string }
  | {
      type: 'oauth2'
      grant: 'client_credentials' | 'password'
      token_url: string
      client_id: string
      client_secret?: string
      username?: string
      password?: string
      scopes?: string[]
      client_auth?: 'basic_header' | 'body'
      cache_token?: boolean
    }

export type AssertOp =
  | 'eq'
  | 'neq'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'contains'
  | 'notContains'
  | 'matches'
  | 'notMatches'
  | 'isDefined'
  | 'isUndefined'
  | 'isNull'
  | 'isNotNull'
  | 'in'
  | 'notIn'
  | 'length'

export interface AssertDto {
  expr: string
  op: AssertOp
  value?: unknown
  enabled?: boolean
}

export interface RequestFileDto {
  meta: { name: string; seq?: number }
  http: {
    method: string
    url: string
    headers?: Pair[]
    query?: Pair[]
    path?: Pair[]
  }
  auth?: AuthDto
  body?: BodyDto
  vars?: Record<string, unknown>
  scripts?: { pre_request?: string; post_response?: string }
  tests?: { asserts?: AssertDto[] }
  options?: {
    timeout_ms?: number
    follow_redirects?: boolean
    max_redirects?: number
    ssl_verify?: boolean
  }
  docs?: { content?: string }
}

export interface TreeNodeDto {
  kind: 'folder' | 'request'
  rel: string
  name: string
  seq?: number
  method?: string
  children?: TreeNodeDto[]
}

export interface CollectionTreeDto {
  id: string // collection root path
  name: string
  path: string
  nodes: TreeNodeDto[]
  invalid: { rel: string; error: string }[]
  environments: string[]
  selected_environment?: string
}

export interface EnvironmentDto {
  meta: { name: string; secrets?: string[] }
  vars: Record<string, unknown>
}

export interface SecretsDto {
  collection: Record<string, string>
  environments: Record<string, Record<string, string>>
}

export interface SettingsDto {
  theme: 'light' | 'dark' | 'system'
  locale?: 'en' | 'pt-BR'
  ui_font_size?: number
  editor_font_size?: number
  network: {
    timeout_ms: number
    follow_redirects: boolean
    max_redirects: number
    ssl_verify: boolean
    response_cap_bytes: number
    proxy: { mode: 'off' | 'system' | 'manual'; url?: string }
  }
}

export const defaultSettings = (): SettingsDto => ({
  theme: 'system',
  network: {
    timeout_ms: 30_000,
    follow_redirects: true,
    max_redirects: 10,
    ssl_verify: true,
    response_cap_bytes: 10 * 1024 * 1024,
    proxy: { mode: 'system' },
  },
})

export interface ResponseMetaDto {
  status: number
  status_text: string
  http_version: string
  headers: [string, string][]
  final_url: string
  timing: { total_ms: number; ttfb_ms: number; download_ms: number }
  body: {
    total_size: number
    /** Bytes available through get_response_body; may be smaller than total_size. */
    preview_size: number
    truncated: boolean
    has_spill: boolean
    can_download_full: boolean
    mime?: string
    charset?: string
    is_binary: boolean
  }
  warnings: { kind: string; name: string }[]
  console: { level: string; message: string }[]
  tests: { name: string; ok: boolean; message?: string }[]
  asserts: {
    expr: string
    op: AssertOp
    expected?: unknown
    actual?: unknown
    ok: boolean
    message?: string
  }[]
  script_error?: string
  cookies: CookieDto[]
}

export interface CookieDto {
  domain: string
  path: string
  name: string
  value: string
  secure: boolean
  http_only: boolean
  expires?: string
}

export interface SaveResultDto {
  outcome: 'saved' | 'conflict'
  hash?: string
  current_text?: string
  current_hash?: string
}

export interface RecentCollectionDto {
  path: string
  name: string
}

// ---------------------------------------------------------------------------
// command + event map
// ---------------------------------------------------------------------------

export interface Commands {
  // collections
  pick_collection_folder: { args: Record<string, never>; result: string | null }
  pick_save_file: { args: { default_name?: string }; result: string | null }
  open_collection: { args: { path: string }; result: CollectionTreeDto }
  create_collection: { args: { parent_dir: string; name: string }; result: CollectionTreeDto }
  close_collection: { args: { id: string }; result: null }
  reload_collection: { args: { id: string }; result: CollectionTreeDto }
  list_recent_collections: { args: Record<string, never>; result: RecentCollectionDto[] }

  // nodes
  create_folder: { args: { id: string; parent_rel: string; name: string }; result: string }
  create_request: { args: { id: string; parent_rel: string; name: string }; result: string }
  read_request: { args: { id: string; rel: string }; result: { request: RequestFileDto; hash: string } }
  save_request: {
    args: { id: string; rel: string; request: RequestFileDto; base_hash?: string }
    result: SaveResultDto
  }
  rename_node: {
    args: { id: string; rel: string; new_name: string; kind: 'folder' | 'request' }
    result: string
  }
  move_node: { args: { id: string; rel: string; new_parent_rel: string }; result: string }
  duplicate_request: { args: { id: string; rel: string }; result: string }
  delete_node: { args: { id: string; rel: string }; result: null }
  reorder_nodes: { args: { id: string; ordered_rels: string[] }; result: null }

  // environments & secrets
  read_environment: { args: { id: string; name: string }; result: EnvironmentDto }
  save_environment: {
    args: { id: string; name: string; env: EnvironmentDto; previous_name?: string }
    result: null
  }
  delete_environment: { args: { id: string; name: string }; result: null }
  select_environment: { args: { id: string; name?: string }; result: null }
  read_secrets: { args: { id: string }; result: SecretsDto }
  save_secrets: { args: { id: string; secrets: SecretsDto }; result: null }

  // http
  send_request: {
    args: { id: string; rel: string; run_id: string; draft?: RequestFileDto; env?: string }
    result: ResponseMetaDto
  }
  cancel_request: { args: { run_id: string }; result: boolean }
  /** Returns preview bytes only; large full bodies must use save_response_body. */
  get_response_body: { args: { run_id: string }; result: Uint8Array }
  /** Saves the complete captured body, including spill files, while the run is cached. */
  save_response_body: { args: { run_id: string; dest: string }; result: null }
  get_cookies: { args: { id: string }; result: CookieDto[] }
  clear_cookies: { args: { id: string; domain?: string }; result: null }
  get_runtime_vars: { args: { id: string }; result: Record<string, unknown> }
  set_runtime_var: { args: { id: string; key: string; value: unknown }; result: null }
  clear_runtime_vars: { args: { id: string }; result: null }

  // curl
  import_curl: { args: { text: string }; result: RequestFileDto }
  export_curl: {
    args: { id: string; rel: string; draft?: RequestFileDto; interpolated: boolean }
    result: string
  }

  // settings / state
  get_settings: { args: Record<string, never>; result: SettingsDto }
  save_settings: { args: { settings: SettingsDto }; result: null }
  get_ui_state: { args: { id?: string }; result: unknown }
  save_ui_state: { args: { id?: string; state_json: unknown }; result: null }
}

export interface Events {
  'request:started': { run_id: string }
  'request:completed': { run_id: string; meta: ResponseMetaDto }
  'request:failed': { run_id: string; error: string }
  'request:cancelled': { run_id: string }
  'script:console': { run_id: string; level: string; message: string }
  'watcher:tree-changed': { id: string; tree: CollectionTreeDto }
  'watcher:file-changed': { id: string; rel: string; hash: string; request?: RequestFileDto }
}

export interface Transport {
  invoke<K extends keyof Commands>(cmd: K, args: Commands[K]['args']): Promise<Commands[K]['result']>
  listen<E extends keyof Events>(event: E, handler: (payload: Events[E]) => void): () => void
}

export class TransportError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.code = code
  }
}
