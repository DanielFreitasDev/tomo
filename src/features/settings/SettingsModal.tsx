import { useState } from 'react'
import { Modal } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { TabPanel, UnderlineTabs } from '@/components/ui/tabs'
import { useT } from '@/i18n'
import { useSettings } from '@/stores/settings'
import { useUi } from '@/stores/ui'

type Section = 'general' | 'network' | 'advanced'

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-1.5">
      <span className="text-sm text-secondary">{label}</span>
      {children}
    </div>
  )
}

export function SettingsModal() {
  const t = useT()
  const open = useUi((s) => s.modal === 'settings')
  const closeModal = useUi((s) => s.openModal)
  const settings = useSettings()
  const [section, setSection] = useState<Section>('general')

  return (
    <Modal open={open} onOpenChange={(o) => !o && closeModal(null)} title={t('settings.title')} size="md">
      <UnderlineTabs<Section>
        value={section}
        onChange={setSection}
        tabs={[
          { value: 'general', label: t('settings.general') },
          { value: 'network', label: t('settings.network') },
          { value: 'advanced', label: t('settings.advanced') },
        ]}
      >
        <TabPanel value="general" className="pt-3">
          <Field label={t('settings.theme')}>
            <Select
              size="sm"
              ariaLabel={t('settings.theme')}
              value={settings.theme}
              onChange={(theme) => settings.update({ theme })}
              options={[
                { value: 'light', label: t('settings.theme.light') },
                { value: 'dark', label: t('settings.theme.dark') },
                { value: 'system', label: t('settings.theme.system') },
              ]}
            />
          </Field>
          <Field label={t('settings.language')}>
            <Select
              size="sm"
              ariaLabel={t('settings.language')}
              value={settings.locale ?? 'system'}
              onChange={(v) =>
                settings.update({ locale: v === 'system' ? undefined : (v as 'en' | 'pt-BR') })
              }
              options={[
                { value: 'system', label: t('settings.theme.system') },
                { value: 'en', label: 'English' },
                { value: 'pt-BR', label: 'Português (BR)' },
              ]}
            />
          </Field>
        </TabPanel>

        <TabPanel value="network" className="pt-3">
          <Field label={t('settings.timeout')}>
            <Input
              inputSize="sm"
              mono
              className="w-28"
              type="number"
              value={settings.network.timeout_ms}
              onChange={(e) =>
                settings.update({ network: { ...settings.network, timeout_ms: Number(e.target.value) } })
              }
            />
          </Field>
          <Field label={t('settings.followRedirects')}>
            <Switch
              checked={settings.network.follow_redirects}
              onCheckedChange={(v) =>
                settings.update({ network: { ...settings.network, follow_redirects: v } })
              }
              ariaLabel={t('settings.followRedirects')}
            />
          </Field>
          <Field label={t('settings.maxRedirects')}>
            <Input
              inputSize="sm"
              mono
              className="w-28"
              type="number"
              value={settings.network.max_redirects}
              onChange={(e) =>
                settings.update({ network: { ...settings.network, max_redirects: Number(e.target.value) } })
              }
            />
          </Field>
          <Field label={t('settings.sslVerify')}>
            <Switch
              checked={settings.network.ssl_verify}
              onCheckedChange={(v) => settings.update({ network: { ...settings.network, ssl_verify: v } })}
              ariaLabel={t('settings.sslVerify')}
            />
          </Field>
          <Field label={t('settings.proxy')}>
            <Select
              size="sm"
              ariaLabel={t('settings.proxy')}
              value={settings.network.proxy.mode}
              onChange={(mode) =>
                settings.update({
                  network: { ...settings.network, proxy: { ...settings.network.proxy, mode } },
                })
              }
              options={[
                { value: 'off', label: t('settings.proxy.off') },
                { value: 'system', label: t('settings.proxy.system') },
                { value: 'manual', label: t('settings.proxy.manual') },
              ]}
            />
          </Field>
          {settings.network.proxy.mode === 'manual' ? (
            <Field label="Proxy URL">
              <Input
                inputSize="sm"
                mono
                className="w-52"
                placeholder="socks5://127.0.0.1:9050"
                value={settings.network.proxy.url ?? ''}
                onChange={(e) =>
                  settings.update({
                    network: {
                      ...settings.network,
                      proxy: { ...settings.network.proxy, url: e.target.value },
                    },
                  })
                }
              />
            </Field>
          ) : null}
          <p className="pt-2 text-2xs text-muted">{t('settings.proxy.system')}</p>
        </TabPanel>

        <TabPanel value="advanced" className="pt-3">
          <Field label="Editor font size">
            <Input
              inputSize="sm"
              mono
              className="w-24"
              type="number"
              value={settings.editor_font_size ?? 13}
              onChange={(e) => settings.update({ editor_font_size: Number(e.target.value) })}
            />
          </Field>
          <Field label="Response cap (MiB)">
            <Input
              inputSize="sm"
              mono
              className="w-24"
              type="number"
              value={Math.round(settings.network.response_cap_bytes / (1024 * 1024))}
              onChange={(e) =>
                settings.update({
                  network: { ...settings.network, response_cap_bytes: Number(e.target.value) * 1024 * 1024 },
                })
              }
            />
          </Field>
        </TabPanel>
      </UnderlineTabs>
    </Modal>
  )
}
