# The Tomo TOML format

Tomo stores collections as plain folders of TOML files. They're meant to be
read, hand-edited and diffed in git — that's the whole point. Tomo edits them
**surgically**: your comments, key order and whitespace survive every save.

```
my-api/
├── bruno.json — no. there is no manifest lock-in here.
├── collection.toml         # collection manifest + shared defaults/auth/vars/scripts
├── .gitignore              # managed block ignores secrets.toml + .env
├── secrets.toml            # git-ignored secret values (created on demand)
├── environments/
│   ├── dev.toml
│   └── prod.toml
├── health.toml             # a request at the root
└── users/
    ├── folder.toml         # folder metadata + shared defaults
    ├── list-users.toml
    └── create-user.toml
```

## Design choices

- **Arrays of inline tables, one item per line.** Headers, params, form fields
  and asserts read top-to-bottom the way you scan them, and every add/remove/
  toggle is a one-line diff. `enabled = false` keeps a row without sending it.
- **`seq` decides order.** Each file's `[meta]` carries an integer `seq`; there
  is no central ordering manifest to conflict on merges. Files without `seq`
  sort last, by filename.
- **`'''` multiline strings** hold JSON, GraphQL and scripts with zero escaping.
- **Absent `[auth]` = inherit** from the nearest folder/collection. `type =
  "none"` opts out explicitly.
- **One request per `<slug>.toml`.** The display name in `[meta]` is what you
  see; the slug is just a filename.

## Request file

```toml
[meta]
name = "Create user"
seq = 3

[http]
method = "POST"                       # any verb, custom allowed
url = "{{base_url}}/api/v2/users/:id/posts"
headers = [
  { name = "Content-Type", value = "application/json" },
  { name = "X-Debug", value = "1", enabled = false },   # kept, not sent
]
query = [ { name = "notify", value = "true" } ]
path  = [ { name = "id", value = "{{user_id}}" } ]      # fills the :id segment

[auth]                                # omit the table entirely to inherit
type = "bearer"
token = "{{access_token}}"

[body]
type = "json"                         # json|text|xml|form_urlencoded|multipart_form|binary|graphql
content = '''
{ "name": "Ada Lovelace" }
'''

[vars]                                # request-scoped variables
user_id = "42"

[scripts]
pre_request = '''
vars.set("nonce", $uuid());
req.setHeader("X-Nonce", vars.get("nonce"));
'''
post_response = '''
vars.set("created_id", res.body.id);
test("created", () => { expect(res.status).toBe(201); });
'''

[tests]
asserts = [
  { expr = "res.status", op = "eq", value = 201 },
  { expr = "res.body.id", op = "isDefined" },
  { expr = "res.headers.content-type", op = "matches", value = "^application/json" },
]

[options]                             # per-request overrides, all optional
timeout_ms = 10000
follow_redirects = false

[docs]
content = '''# Create user
Creates a user and returns `201`.'''
```

### Body variants

```toml
[body]
type = "form_urlencoded"
fields = [ { name = "grant_type", value = "password" } ]

[body]
type = "multipart_form"
parts = [
  { name = "meta", kind = "text", value = '{"v":1}', content_type = "application/json" },
  { name = "avatar", kind = "file", path = "assets/avatar.png", content_type = "image/png" },
]

[body]
type = "binary"
path = "payloads/firmware.bin"        # relative to the collection root
content_type = "application/octet-stream"

[body]
type = "graphql"
query = '''query User($id: ID!) { user(id: $id) { name } }'''
variables = '''{ "id": "{{user_id}}" }'''
```

### Auth variants

`none`, `inherit`, `basic`, `bearer`, `api_key` (header/query placement),
`digest`, and `oauth2` (client_credentials / password grants, token caching):

```toml
[auth]
type = "oauth2"
grant = "client_credentials"
token_url = "{{base_url}}/oauth/token"
client_id = "{{oauth_client_id}}"
client_secret = "{{oauth_client_secret}}"   # keep the value in secrets.toml
scopes = ["read", "write"]
client_auth = "basic_header"                 # or "body"
cache_token = true
```

## Assert operators

`eq neq gt gte lt lte contains notContains matches notMatches isDefined
isUndefined isNull isNotNull in notIn length`. Selectors: `res.status`,
`res.statusText`, `res.responseTime`, `res.size`, `res.headers.<name>`,
`res.body[.dot.path][index]`.

## collection.toml

```toml
[meta]
name = "Acme API"
format = 1

[defaults]
headers = [ { name = "User-Agent", value = "tomo/0.1" } ]

[auth]
type = "bearer"
token = "{{access_token}}"

[vars]
base_url = "https://api.acme.test"

[scripts]
pre_request = '''// runs before every request'''

[[tls.client_certs]]
host = "mtls.acme.test"
cert = "certs/client.pem"
key = "certs/client-key.pem"
```

## folder.toml

Same shape as `collection.toml` (defaults/auth/vars/scripts) plus a `seq`:

```toml
[meta]
name = "Users"
seq = 2
```

## environments/&lt;name&gt;.toml

`meta.secrets` lists variable **names** whose values live outside git.

```toml
[meta]
name = "Development"
secrets = ["api_key", "oauth_client_secret"]

[vars]
base_url = "https://dev.api.acme.test"
```

## secrets.toml (git-ignored)

Created on demand; the managed `.gitignore` block is written **before** this
file ever touches disk.

```toml
[collection]
admin_pw = "hunter2"

[environments.dev]
api_key = "sk-dev-123"
```

Secret resolution for a name in `meta.secrets`:
`secrets.toml [environments.<env>]` → `secrets.toml [collection]` → `.env` →
process env → empty (with a warning).

## Variables

`{{var}}` with dot/index paths (`{{user.address[0].street}}`). Precedence,
highest first: **runtime > request > folder (inner→outer) > environment (+
secrets) > collection > process env / .env**.

Dynamic variables, fresh per use: `{{$uuid}}`, `{{$timestamp}}`,
`{{$isoTimestamp}}`, `{{$randomInt}}`.

## Scripting API

Pre-request and post-response scripts run in a sandboxed JS engine.

- `req.url` / `req.method` / `req.headers.get|set|remove` / `req.body`
- `res.status` / `res.statusText` / `res.headers` / `res.body` / `res.responseTime`
- `vars.get|set|has|delete(name)` (runtime scope) · `env.name()` / `env.get(name)`
- `console.log|info|warn|error|debug` (streamed to the console panel)
- `expect(x).toBe|toEqual|toContain|toMatch|toBeDefined|…` · `test(name, fn)`
