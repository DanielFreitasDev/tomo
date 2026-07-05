import { describe, expect, it } from 'vitest'
import { translate } from './index'
import { en } from './messages/en'
import { ptBR } from './messages/pt-BR'

describe('i18n', () => {
  it('pt-BR covers every key (also compile-enforced)', () => {
    for (const key of Object.keys(en)) {
      expect(ptBR[key as keyof typeof en], `missing pt-BR for ${key}`).toBeTruthy()
    }
  })

  it('interpolates {params} and leaves unknown params visible', () => {
    expect(translate('en', 'tabs.confirmClose.title', { name: 'login.toml' })).toBe(
      'Save changes to “login.toml”?',
    )
    expect(translate('pt-BR', 'tabs.confirmClose.title', { name: 'x' })).toContain('“x”')
    expect(translate('en', 'tabs.confirmClose.title')).toContain('{name}')
  })
})
