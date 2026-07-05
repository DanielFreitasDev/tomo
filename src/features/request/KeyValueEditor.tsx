import { Plus, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { CodeEditor } from '@/components/ui/code-editor'
import { IconButton } from '@/components/ui/icon-button'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/switch'
import { useT } from '@/i18n'
import type { Pair } from '@/lib/transport'

export function KeyValueEditor({
  pairs,
  onChange,
  collectionId,
  keyPlaceholder = 'name',
  valuePlaceholder = 'value',
  addLabel,
}: {
  pairs: Pair[]
  onChange: (pairs: Pair[]) => void
  collectionId: string
  keyPlaceholder?: string
  valuePlaceholder?: string
  addLabel?: string
}) {
  const t = useT()

  const update = (index: number, patch: Partial<Pair>) => {
    onChange(pairs.map((p, i) => (i === index ? { ...p, ...patch } : p)))
  }

  return (
    <div className="flex flex-col gap-1 p-2">
      {pairs.map((pair, index) => (
        <div
          // biome-ignore lint/suspicious/noArrayIndexKey: rows are positional by design
          key={index}
          className="flex items-center gap-1.5"
        >
          <Checkbox
            checked={pair.enabled !== false}
            onCheckedChange={(enabled) => update(index, { enabled: enabled ? undefined : false })}
            ariaLabel={t('common.enabled')}
          />
          <Input
            inputSize="sm"
            mono
            className="w-2/5"
            placeholder={keyPlaceholder}
            value={pair.name}
            onChange={(e) => update(index, { name: e.target.value })}
            aria-label={t('common.name')}
          />
          <div className="min-w-0 flex-1 rounded-md border border-default bg-raised px-2 focus-within:border-(--accent)">
            <CodeEditor
              singleLine
              value={pair.value}
              onChange={(value) => update(index, { value })}
              collectionId={collectionId}
              placeholder={valuePlaceholder}
              ariaLabel={t('common.value')}
            />
          </div>
          <IconButton
            label={t('common.delete')}
            size="sm"
            onClick={() => onChange(pairs.filter((_, i) => i !== index))}
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
          onClick={() => onChange([...pairs, { name: '', value: '' }])}
        >
          {addLabel ?? t('common.add')}
        </Button>
      </div>
    </div>
  )
}
