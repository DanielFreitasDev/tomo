import { expect, it } from 'vitest'
import { setTransport, type Transport } from '@/lib/transport'
import { useSettings } from './settings'

it('boots with defaults when get_settings fails, never hanging on a blank screen', async () => {
  // a corrupt/unreadable settings file must not brick the app: load() must
  // still flip `loaded` so App renders, falling back to in-store defaults.
  setTransport({
    invoke: () => Promise.reject(new Error('corrupt settings.toml')),
    listen: () => () => {},
  } as unknown as Transport)

  useSettings.setState({ loaded: false })
  await useSettings.getState().load()

  expect(useSettings.getState().loaded).toBe(true)
})
