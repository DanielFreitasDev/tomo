import { KeyRound, Plus, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/dialog'
import { IconButton } from '@/components/ui/icon-button'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { toast } from '@/components/ui/toast'
import { useT } from '@/i18n'
import { cn } from '@/lib/cn'
import { type EnvironmentDto, transport } from '@/lib/transport'
import { useCollections } from '@/stores/collections'
import { useEnvironments } from '@/stores/environments'
import { useUi } from '@/stores/ui'

export type EnvVarType = 'string' | 'number' | 'boolean' | 'json' | 'secret'

export interface VarRow {
  name: string
  value: string
  type: EnvVarType
}

const VAR_TYPES: EnvVarType[] = ['string', 'number', 'boolean', 'json', 'secret']

export function rowsFromEnvironment(env: EnvironmentDto): VarRow[] {
  const secrets = new Set(env.meta.secrets ?? [])
  const names = new Set([...Object.keys(env.vars), ...secrets])
  return [...names].map((name) => {
    const value = env.vars[name] ?? ''
    return {
      name,
      value: valueToText(value),
      type: secrets.has(name) ? 'secret' : typeOfValue(value),
    }
  })
}

export function environmentFromRows(name: string, rows: VarRow[]): EnvironmentDto {
  const vars: Record<string, unknown> = {}
  const secrets: string[] = []
  for (const row of rows) {
    if (!row.name) continue
    if (row.type === 'secret') {
      secrets.push(row.name)
    } else {
      vars[row.name] = parseRowValue(row)
    }
  }
  return { meta: { name, secrets }, vars }
}

function typeOfValue(value: unknown): EnvVarType {
  if (typeof value === 'number') return 'number'
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'string') return 'string'
  return 'json'
}

function valueToText(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  return JSON.stringify(value, null, 2)
}

function parseRowValue(row: VarRow): unknown {
  switch (row.type) {
    case 'number': {
      const n = Number(row.value)
      return Number.isFinite(n) ? n : 0
    }
    case 'boolean':
      return row.value === 'true'
    case 'json':
      try {
        return JSON.parse(row.value)
      } catch {
        return row.value
      }
    case 'string':
    case 'secret':
      return row.value
  }
}

export function EnvironmentsModal() {
  const t = useT()
  const open = useUi((s) => s.modal === 'environments')
  const closeModal = useUi((s) => s.openModal)
  const collectionId = useCollections((s) => s.order[0])
  const info = useCollections((s) => (collectionId ? s.byId[collectionId] : undefined))
  const setEnvStore = useEnvironments((s) => s.setEnv)

  const [envs, setEnvs] = useState<string[]>([])
  const [active, setActive] = useState<string | null>(null)
  const [rows, setRows] = useState<VarRow[]>([])
  const [secretValues, setSecretValues] = useState<Record<string, string>>({})

  useEffect(() => {
    if (!open || !info) return
    setEnvs(info.environments)
    const first = info.selectedEnv ?? info.environments[0] ?? null
    setActive(first)
  }, [open, info])

  useEffect(() => {
    if (!open || !collectionId || !active) {
      setRows([])
      return
    }
    void transport()
      .invoke('read_environment', { id: collectionId, name: active })
      .then((env) => {
        setRows(rowsFromEnvironment(env))
      })
      .catch(() => setRows([]))
    void transport()
      .invoke('read_secrets', { id: collectionId })
      .then((s) => setSecretValues(s.environments[active] ?? {}))
      .catch(() => setSecretValues({}))
  }, [open, collectionId, active])

  if (!collectionId || !info) return null

  const save = async () => {
    if (!active) return
    const secretVals: Record<string, string> = {}
    for (const row of rows) {
      if (!row.name) continue
      if (row.type === 'secret') {
        secretVals[row.name] = String(secretValues[row.name] ?? '')
      }
    }
    const env = environmentFromRows(active, rows)
    await transport().invoke('save_environment', { id: collectionId, name: active, env })
    setEnvStore(collectionId, env)
    const current = await transport().invoke('read_secrets', { id: collectionId })
    current.environments[active] = secretVals
    await transport().invoke('save_secrets', { id: collectionId, secrets: current })
    toast.success(t('toast.saved'), active)
  }

  const createEnv = async () => {
    const name = `env-${envs.length + 1}`
    await transport().invoke('save_environment', {
      id: collectionId,
      name,
      env: { meta: { name }, vars: {} },
    })
    setEnvs([...envs, name])
    setActive(name)
  }

  return (
    <Modal
      open={open}
      onOpenChange={(o) => !o && closeModal(null)}
      title={t('env.title')}
      size="lg"
      footer={
        <>
          <Button variant="ghost" onClick={() => closeModal(null)}>
            {t('common.close')}
          </Button>
          <Button variant="primary" onClick={() => void save()} disabled={!active}>
            {t('common.save')}
          </Button>
        </>
      }
    >
      <div className="flex h-96 gap-4">
        <div className="flex w-44 shrink-0 flex-col gap-0.5 border-r border-subtle pr-2">
          {envs.map((name) => (
            <button
              key={name}
              type="button"
              onClick={() => setActive(name)}
              className={cn(
                'rounded-md px-2 py-1.5 text-left text-sm transition-colors',
                active === name ? 'bg-selected text-primary' : 'text-secondary hover:bg-hover',
              )}
            >
              {name}
            </button>
          ))}
          <Button variant="ghost" size="sm" icon={<Plus size={12} />} onClick={() => void createEnv()}>
            {t('env.new')}
          </Button>
        </div>

        <div className="flex min-w-0 flex-1 flex-col">
          {active ? (
            <>
              <div className="flex flex-col gap-1 overflow-y-auto">
                {rows.map((row, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: rows are positional
                  <div key={i} className="flex items-center gap-1.5">
                    <Input
                      inputSize="sm"
                      mono
                      className="w-1/3"
                      placeholder="name"
                      value={row.name}
                      onChange={(e) =>
                        setRows(rows.map((r, x) => (x === i ? { ...r, name: e.target.value } : r)))
                      }
                    />
                    <Select
                      ariaLabel="Type"
                      size="sm"
                      value={row.type}
                      onChange={(type) => {
                        setRows(
                          rows.map((r, x) => {
                            if (x !== i) return r
                            return {
                              ...r,
                              type,
                              value:
                                r.type === 'secret' && type !== 'secret'
                                  ? (secretValues[r.name] ?? r.value)
                                  : r.value,
                            }
                          }),
                        )
                        if (type === 'secret' && row.name && secretValues[row.name] === undefined) {
                          setSecretValues({ ...secretValues, [row.name]: row.value })
                        }
                      }}
                      options={VAR_TYPES.map((type) => ({ value: type, label: type }))}
                      triggerClassName="w-24"
                    />
                    {row.type === 'boolean' ? (
                      <Select
                        ariaLabel="Value"
                        size="sm"
                        value={row.value === 'true' ? 'true' : 'false'}
                        onChange={(value) => setRows(rows.map((r, x) => (x === i ? { ...r, value } : r)))}
                        options={[
                          { value: 'true', label: 'true' },
                          { value: 'false', label: 'false' },
                        ]}
                        triggerClassName="flex-1"
                      />
                    ) : (
                      <Input
                        inputSize="sm"
                        mono
                        className="flex-1"
                        type={row.type === 'secret' ? 'password' : row.type === 'number' ? 'number' : 'text'}
                        placeholder={row.type === 'secret' ? 'stored in secrets.toml' : 'value'}
                        value={row.type === 'secret' ? (secretValues[row.name] ?? '') : row.value}
                        onChange={(e) => {
                          if (row.type === 'secret') {
                            setSecretValues({ ...secretValues, [row.name]: e.target.value })
                          } else {
                            setRows(rows.map((r, x) => (x === i ? { ...r, value: e.target.value } : r)))
                          }
                        }}
                      />
                    )}
                    {row.type === 'secret' ? <KeyRound size={13} className="shrink-0 text-muted" /> : null}
                    <IconButton
                      label={t('common.delete')}
                      size="sm"
                      onClick={() => setRows(rows.filter((_, x) => x !== i))}
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
                    onClick={() => setRows([...rows, { name: '', value: '', type: 'string' }])}
                  >
                    {t('common.add')}
                  </Button>
                </div>
              </div>
              <p className="mt-auto pt-3 text-2xs text-muted">{t('env.secrets.hint')}</p>
            </>
          ) : (
            <div className="flex h-full items-center justify-center text-xs text-muted">{t('env.new')}</div>
          )}
        </div>
      </div>
    </Modal>
  )
}
