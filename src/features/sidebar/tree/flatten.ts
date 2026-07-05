import type { TreeNodeDto } from '@/lib/transport'

export interface VisibleRow {
  rel: string
  kind: 'folder' | 'request'
  name: string
  method?: string
  depth: number
  hasChildren: boolean
  isExpanded: boolean
  parentRel: string
  /** position info for ARIA */
  posinset: number
  setsize: number
}

/** Flatten only the expanded branches into rows for virtualization. */
export function flattenVisible(
  nodes: TreeNodeDto[],
  expanded: Record<string, true>,
  filter: string,
): VisibleRow[] {
  const rows: VisibleRow[] = []
  const query = filter.trim().toLowerCase()

  const matches = (node: TreeNodeDto): boolean => {
    if (!query) return true
    if (node.name.toLowerCase().includes(query)) return true
    return (node.children ?? []).some(matches)
  }

  const walk = (list: TreeNodeDto[], depth: number, parentRel: string) => {
    const visible = query ? list.filter(matches) : list
    visible.forEach((node, i) => {
      const isFolder = node.kind === 'folder'
      // filtering force-expands matched folders so hits are visible
      const isExpanded = isFolder && (query ? true : Boolean(expanded[node.rel]))
      rows.push({
        rel: node.rel,
        kind: node.kind,
        name: node.name,
        method: node.method,
        depth,
        hasChildren: isFolder,
        isExpanded,
        parentRel,
        posinset: i + 1,
        setsize: visible.length,
      })
      if (isFolder && isExpanded && node.children) {
        walk(node.children, depth + 1, node.rel)
      }
    })
  }

  walk(nodes, 0, '')
  return rows
}
