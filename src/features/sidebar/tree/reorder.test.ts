import { describe, expect, it } from 'vitest'
import type { TreeNodeDto } from '@/lib/transport'
import { orderedRelsAfterDrop } from './reorder'

const tree: TreeNodeDto[] = [
  { kind: 'request', rel: 'a.toml', name: 'A', method: 'GET' },
  {
    kind: 'folder',
    rel: 'users',
    name: 'Users',
    children: [
      { kind: 'request', rel: 'users/list.toml', name: 'List', method: 'GET' },
      { kind: 'request', rel: 'users/create.toml', name: 'Create', method: 'POST' },
    ],
  },
  { kind: 'request', rel: 'z.toml', name: 'Z', method: 'GET' },
]

describe('tree reorder helpers', () => {
  it('moves a root sibling after another root sibling', () => {
    expect(orderedRelsAfterDrop(tree, 'a.toml', 'a.toml', 'z.toml', 'after')).toEqual([
      'users',
      'z.toml',
      'a.toml',
    ])
  })

  it('returns destination folder order when dropping into a folder', () => {
    expect(orderedRelsAfterDrop(tree, 'a.toml', 'users/a.toml', 'users', 'into')).toEqual([
      'users/list.toml',
      'users/create.toml',
      'users/a.toml',
    ])
  })
})
