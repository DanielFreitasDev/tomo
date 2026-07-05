/** Dev-only component gallery (#/gallery) — visual snapshots + axe target. */
import { Folder, Plus, Search, Send } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ConfirmDialog, Modal } from '@/components/ui/dialog'
import { EmptyState } from '@/components/ui/empty-state'
import { IconButton } from '@/components/ui/icon-button'
import { Input } from '@/components/ui/input'
import { Kbd } from '@/components/ui/kbd'
import { ContextMenu, Dropdown } from '@/components/ui/menu'
import { MethodBadge } from '@/components/ui/method-badge'
import { Select } from '@/components/ui/select'
import { Spinner } from '@/components/ui/spinner'
import { StatusPill } from '@/components/ui/status-pill'
import { Checkbox, Switch } from '@/components/ui/switch'
import { Segmented, TabPanel, UnderlineTabs } from '@/components/ui/tabs'
import { ToastViewport, toast } from '@/components/ui/toast'
import { applyThemeToDocument } from '@/stores/settings'

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-2">
      <h2 className="text-xs font-semibold uppercase tracking-wide text-muted">{title}</h2>
      <div className="flex flex-wrap items-center gap-3 rounded-lg border border-subtle bg-surface p-4">
        {children}
      </div>
    </section>
  )
}

export function Gallery() {
  const [dark, setDark] = useState(false)
  const [checked, setChecked] = useState(true)
  const [modalOpen, setModalOpen] = useState(false)
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [seg, setSeg] = useState<'pretty' | 'raw' | 'preview'>('pretty')
  const [tab, setTab] = useState<'params' | 'headers' | 'body'>('params')
  const [method, setMethod] = useState<'GET' | 'POST' | 'DELETE'>('GET')

  return (
    <div className="h-full overflow-y-auto bg-app p-6" data-testid="gallery">
      <div className="mx-auto max-w-3xl space-y-6 pb-16">
        <header className="flex items-center justify-between">
          <h1 className="font-mono text-lg font-bold text-accent-text">Tomo · gallery</h1>
          <span className="flex items-center gap-2 text-xs text-secondary">
            Dark
            <Switch
              checked={dark}
              onCheckedChange={(v) => {
                setDark(v)
                applyThemeToDocument(v ? 'dark' : 'light')
              }}
              ariaLabel="Toggle dark theme"
            />
          </span>
        </header>

        <Section title="Buttons">
          <Button variant="primary" icon={<Send size={13} />}>
            Send
          </Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="soft">Soft</Button>
          <Button variant="danger">Delete</Button>
          <Button variant="primary" loading>
            Sending
          </Button>
          <Button variant="secondary" disabled>
            Disabled
          </Button>
          <Button variant="secondary" size="sm">
            Small
          </Button>
          <IconButton label="New request">
            <Plus size={15} />
          </IconButton>
        </Section>

        <Section title="Inputs">
          <Input placeholder="https://api.example.com" className="w-64" />
          <Input
            placeholder="Search"
            prefixEl={<Search size={13} className="text-muted" />}
            className="w-48"
          />
          <Input placeholder="Invalid" invalid className="w-40" />
          <Input placeholder="mono value" mono className="w-40" />
          <Select
            value={method}
            onChange={setMethod}
            ariaLabel="Method"
            options={[
              { value: 'GET', label: <MethodBadge method="GET" block /> },
              { value: 'POST', label: <MethodBadge method="POST" block /> },
              { value: 'DELETE', label: <MethodBadge method="DELETE" block /> },
            ]}
          />
        </Section>

        <Section title="Badges & pills">
          <MethodBadge method="GET" />
          <MethodBadge method="POST" />
          <MethodBadge method="PUT" />
          <MethodBadge method="PATCH" />
          <MethodBadge method="DELETE" />
          <MethodBadge method="OPTIONS" />
          <MethodBadge method="QUERY" />
          <StatusPill status={200} statusText="OK" />
          <StatusPill status={301} statusText="Moved" />
          <StatusPill status={404} statusText="Not Found" />
          <StatusPill status={500} statusText="Server Error" />
          <Badge tone="accent">draft</Badge>
          <Badge tone="success">passed</Badge>
          <Badge tone="danger">failed</Badge>
          <Kbd>Ctrl</Kbd>
          <Kbd>⏎</Kbd>
          <Spinner />
        </Section>

        <Section title="Selection controls">
          <Checkbox checked={checked} onCheckedChange={setChecked} ariaLabel="Row enabled" />
          <Checkbox checked={false} onCheckedChange={() => {}} ariaLabel="Off" />
          <Checkbox checked={false} indeterminate onCheckedChange={() => {}} ariaLabel="Some" />
          <Switch checked={checked} onCheckedChange={setChecked} ariaLabel="Feature toggle" />
          <Segmented
            value={seg}
            onChange={setSeg}
            options={[
              { value: 'pretty', label: 'Pretty' },
              { value: 'raw', label: 'Raw' },
              { value: 'preview', label: 'Preview' },
            ]}
          />
        </Section>

        <Section title="Tabs">
          <UnderlineTabs
            className="w-full"
            value={tab}
            onChange={setTab}
            tabs={[
              { value: 'params', label: 'Params', badge: <Badge tone="accent">3</Badge> },
              { value: 'headers', label: 'Headers' },
              { value: 'body', label: 'Body' },
            ]}
          >
            <TabPanel value={tab} className="p-3 text-xs text-secondary">
              panel: {tab}
            </TabPanel>
          </UnderlineTabs>
        </Section>

        <Section title="Menus, modals, toasts">
          <Dropdown
            trigger={<Button variant="secondary">Dropdown</Button>}
            entries={[
              { label: 'New request', kbd: 'Ctrl+N', onSelect: () => {} },
              { label: 'New folder', icon: <Folder size={13} />, onSelect: () => {} },
              'separator',
              { label: 'Delete', danger: true, onSelect: () => {} },
            ]}
          />
          <ContextMenu
            entries={[
              { label: 'Rename', kbd: 'F2', onSelect: () => {} },
              { label: 'Delete', danger: true, onSelect: () => {} },
            ]}
          >
            <div className="rounded-md border border-dashed border-strong px-3 py-1.5 text-xs text-secondary">
              right-click me
            </div>
          </ContextMenu>
          <Button variant="secondary" onClick={() => setModalOpen(true)}>
            Open modal
          </Button>
          <Button variant="secondary" onClick={() => setConfirmOpen(true)}>
            Confirm dialog
          </Button>
          <Button variant="secondary" onClick={() => toast.success('Saved', 'create-user.toml')}>
            Toast
          </Button>
          <Button
            variant="secondary"
            onClick={() =>
              toast.action('warning', '“login.toml” changed on disk', {
                label: 'Reload from disk',
                onSelect: () => {},
              })
            }
          >
            Action toast
          </Button>
        </Section>

        <Section title="Empty state">
          <div className="h-40 w-full">
            <EmptyState
              icon={Send}
              title="Send a request"
              hint={
                <span>
                  Press <Kbd>Ctrl</Kbd> + <Kbd>⏎</Kbd> to send
                </span>
              }
            />
          </div>
        </Section>
      </div>

      <Modal
        open={modalOpen}
        onOpenChange={setModalOpen}
        title="Environments"
        footer={
          <>
            <Button variant="ghost" onClick={() => setModalOpen(false)}>
              Cancel
            </Button>
            <Button variant="primary" onClick={() => setModalOpen(false)}>
              Save
            </Button>
          </>
        }
      >
        <p className="text-sm text-secondary">Modal body content.</p>
      </Modal>

      <ConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        title="Save changes to “create-user.toml”?"
        body="Your changes will be lost if you don’t save them."
        confirmLabel="Save"
        cancelLabel="Cancel"
        extraAction={{ label: 'Discard', onSelect: () => {} }}
        onConfirm={() => setConfirmOpen(false)}
      />

      <ToastViewport />
    </div>
  )
}
