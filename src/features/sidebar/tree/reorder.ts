import type { TreeNodeDto } from '@/lib/transport'

export type DropMode = 'into' | 'after'

export function parentRelOf(rel: string): string {
  const idx = rel.lastIndexOf('/')
  return idx === -1 ? '' : rel.slice(0, idx)
}

export function orderedRelsAfterDrop(
  nodes: TreeNodeDto[],
  sourceRel: string,
  movedRel: string,
  targetRel: string,
  mode: DropMode,
): string[] {
  if (mode === 'into') {
    return [...childRels(nodes, targetRel).filter((rel) => rel !== sourceRel && rel !== movedRel), movedRel]
  }

  const targetParent = parentRelOf(targetRel)
  const siblings = childRels(nodes, targetParent).filter((rel) => rel !== sourceRel && rel !== movedRel)
  const targetIndex = siblings.indexOf(targetRel)
  const insertAt = targetIndex === -1 ? siblings.length : targetIndex + 1
  return [...siblings.slice(0, insertAt), movedRel, ...siblings.slice(insertAt)]
}

function childRels(nodes: TreeNodeDto[], parentRel: string): string[] {
  const children = parentRel ? findNode(nodes, parentRel)?.children : nodes
  return (children ?? []).map((node) => node.rel)
}

function findNode(nodes: TreeNodeDto[], rel: string): TreeNodeDto | undefined {
  for (const node of nodes) {
    if (node.rel === rel) return node
    const child = node.children ? findNode(node.children, rel) : undefined
    if (child) return child
  }
  return undefined
}
