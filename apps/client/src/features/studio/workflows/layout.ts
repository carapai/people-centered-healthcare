import { Graph, layout } from '@dagrejs/dagre'

import { rows, type State, type WorkflowDefinition } from './model'

// The canvas's layout (studio.md § The Workflow Studio): left to right with dagre. Each compound
// or parallel state is laid out on its own first, then sized to hold its children, then placed
// in its parent like any other node, so a group never overlaps its siblings. A parallel state's
// regions are not joined by transitions, so they stand side by side.

/** A state's box, relative to its parent's box (or to the canvas, at the top level). */
export interface Box {
  x: number
  y: number
  width: number
  height: number
}

export const LEAF_WIDTH = 200
/** Room at the top of a group for its title. */
export const GROUP_HEADER = 40
const GROUP_MARGIN = 16

/** A leaf's height: a second line for its task, gate, sub-workflow or being final. */
export const leafHeight = (state: State) => ((state.task ?? null) !== null || (state.gate ?? null) !== null || (state.invoke ?? []).length > 0 || state.kind === 'final' ? 64 : 44)

export function arrange(definition: WorkflowDefinition): Map<string, Box> {
  const boxes = new Map<string, Box>()
  const parent = new Map(rows(definition).map((r) => [r.state.id, r.parent]))
  // The child of `container` that `id` is in (or is).
  const childOf = (container: string | null, id: string): string | null => {
    let at: string | null = id
    while (at !== null) {
      const up: string | null = parent.get(at) ?? null
      if (up === container) return at
      at = up
    }
    return null
  }
  const transitions = rows(definition).flatMap(({ state }) => (state.transitions ?? []).flatMap((t) => (t.targets ?? []).map((target) => [state.id, target] as const)))

  const place = (container: State | null): { width: number; height: number } => {
    const children = container === null ? definition.statechart.states : (container.children ?? [])
    const graph = new Graph()
    const inside = container !== null
    graph.setGraph({ rankdir: 'LR', nodesep: 36, ranksep: 110, marginx: inside ? GROUP_MARGIN : 0, marginy: inside ? GROUP_MARGIN : 0 })
    graph.setDefaultEdgeLabel(() => ({}))
    for (const child of children) {
      const group = (child.children ?? []).length > 0
      const size = group ? place(child) : { width: LEAF_WIDTH, height: leafHeight(child) }
      graph.setNode(child.id, size)
    }
    if (container?.kind !== 'parallel') {
      for (const [from, to] of transitions) {
        const a = childOf(container?.id ?? null, from)
        const b = childOf(container?.id ?? null, to)
        if (a !== null && b !== null && a !== b && graph.hasNode(a) && graph.hasNode(b)) graph.setEdge(a, b)
      }
    }
    layout(graph)
    for (const child of children) {
      const node = graph.node(child.id)
      boxes.set(child.id, { x: node.x - node.width / 2, y: node.y - node.height / 2 + (inside ? GROUP_HEADER : 0), width: node.width, height: node.height })
    }
    const size = graph.graph()
    return { width: Math.max(size.width ?? 0, LEAF_WIDTH + 2 * GROUP_MARGIN), height: Math.max(size.height ?? 0, 44) + (inside ? GROUP_HEADER : 0) }
  }
  place(null)
  return boxes
}
