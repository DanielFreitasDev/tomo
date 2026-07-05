import { describe, expect, it } from 'vitest'
import { environmentFromRows, rowsFromEnvironment } from './EnvironmentsModal'

describe('environment row conversion', () => {
  it('preserves string, number, boolean, json and secret metadata', () => {
    const rows = rowsFromEnvironment({
      meta: { name: 'dev', secrets: ['api_key'] },
      vars: {
        name: 'Ada',
        retries: 3,
        enabled: true,
        payload: { nested: ['x'] },
      },
    })

    const env = environmentFromRows('dev', rows)

    expect(env.vars.name).toBe('Ada')
    expect(env.vars.retries).toBe(3)
    expect(env.vars.enabled).toBe(true)
    expect(env.vars.payload).toEqual({ nested: ['x'] })
    expect(env.meta.secrets).toContain('api_key')
    expect(env.vars).not.toHaveProperty('api_key')
  })
})
