import { KeyRound, Plus, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/dialog'
import { IconButton } from '@/components/ui/icon-button'
import { Input } from '@/components/ui/input'
import { toast } from '@/components/ui/toast'
import { useT } from '@/i18n'
import { cn } from '@/lib/cn'
import { type EnvironmentDto, transport } from '@/lib/transport'
import { useCollections } from '@/stores/collections'
import { useEnvironments } from '@/stores/environments'
import { useUi } from '@/stores/ui'

interface VarRow {
  name: string
  value: string
  secret: boolean
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
        const secrets = new Set(env.meta.secrets ?? [])
        setRows(
          Object.entries(env.vars).map(([name, v]) => ({
            name,
            value: typeof v === 'string' ? v : JSON.stringify(v),
            secret: secrets.has(name),
          })),
        )
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
    const vars: Record<string, unknown> = {}
    const secrets: string[] = []
    const secretVals: Record<string, string> = {}
    for (const row of rows) {
      if (!row.name) continue
      if (row.secret) {
        secrets.push(row.name)
        if (secretValues[row.name]) secretVals[row.name] = secretValues[row.name] ?? ''
      } else {
        vars[row.name] = row.value
      }
    }
    const env: EnvironmentDto = { meta: { name: active, secrets }, vars }
    await transport().invoke('save_environment', { id: collectionId, name: active, env })
    setEnvStore(collectionId, env)
    if (Object.keys(secretVals).length > 0) {
      const current = await transport().invoke('read_secrets', { id: collectionId })
      current.environments[active] = { ...(current.environments[active] ?? {}), ...secretVals }
      await transport().invoke('save_secrets', { id: collectionId, secrets: current })
    }
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
                    <Input
                      inputSize="sm"
                      mono
                      className="flex-1"
                      type={row.secret ? 'password' : 'text'}
                      placeholder={row.secret ? '•••• (stored in secrets.toml)' : 'value'}
                      value={row.secret ? (secretValues[row.name] ?? '') : row.value}
                      onChange={(e) => {
                        if (row.secret) setSecretValues({ ...secretValues, [row.name]: e.target.value })
                        else setRows(rows.map((r, x) => (x === i ? { ...r, value: e.target.value } : r)))
                      }}
                    />
                    <IconButton
                      label="Secret"
                      size="sm"
                      variant={row.secret ? 'soft' : 'ghost'}
                      onClick={() => setRows(rows.map((r, x) => (x === i ? { ...r, secret: !r.secret } : r)))}
                    >
                      <KeyRound size={13} />
                    </IconButton>
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
                    onClick={() => setRows([...rows, { name: '', value: '', secret: false }])}
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
