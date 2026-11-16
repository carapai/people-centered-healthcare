// PROTOTYPE (#481), throwaway: the statechart on @xyflow/react 13, laid out with @dagrejs/dagre.
// Compound and parallel states are group nodes; each container is laid out on its own (parallel
// regions side by side), then sized to fit, then laid out in its parent.
import { Graph, layout } from '@dagrejs/dagre'
import { Background, Controls, Handle, MarkerType, Position, ReactFlow, type Edge, type Node, type NodeProps } from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { useMemo } from 'react'

import { guardText, type Definition, type State } from './workflowData'

type Box = { x: number; y: number; w: number; h: number }
type StateData = { state: State; active: boolean; selected: boolean; problem: boolean }

const LEAF_W = 190
const leafHeight = (st: State) => (st.task || st.invoke || st.gate || st.kind === 'final' ? 64 : 44)

function arrange(def: Definition): Map<string, Box> {
  const boxes = new Map<string, Box>()
  const byId = new Map(def.states.map((x) => [x.id, x]))
  const childOf = (container: string | null, id: string): string | null => {
    let at: string | null = id
    while (at !== null) {
      const parent: string | null = byId.get(at)?.parent ?? null
      if (parent === container) return at
      at = parent
    }
    return null
  }
  const place = (container: string | null): { w: number; h: number } => {
    const kids = def.states.filter((x) => x.parent === container)
    const g = new Graph()
    const parallel = container !== null && byId.get(container)?.kind === 'parallel'
    g.setGraph({ rankdir: 'LR', nodesep: 36, ranksep: 130, marginx: container === null ? 0 : 16, marginy: container === null ? 0 : 40 })
    g.setDefaultEdgeLabel(() => ({}))
    for (const k of kids) {
      const size = k.kind === 'compound' || k.kind === 'parallel' ? place(k.id) : { w: LEAF_W, h: leafHeight(k) }
      g.setNode(k.id, { width: size.w, height: size.h })
    }
    if (!parallel) {
      for (const tr of def.transitions) {
        const a = childOf(container, tr.from)
        const b = childOf(container, tr.to)
        if (a && b && a !== b) g.setEdge(a, b)
      }
    }
    layout(g)
    for (const k of kids) {
      const n = g.node(k.id)
      const prior = boxes.get(k.id)
      boxes.set(k.id, { x: n.x - n.width / 2, y: n.y - n.height / 2, w: prior?.w ?? n.width, h: prior?.h ?? n.height })
    }
    const graph = g.graph()
    return { w: Math.max(graph.width ?? 0, 220), h: Math.max(graph.height ?? 0, 80) }
  }
  place(null)
  return boxes
}

function StateNode({ data }: NodeProps<Node<StateData>>) {
  const st = data.state
  const container = st.kind === 'compound' || st.kind === 'parallel'
  const ring = data.active ? '3px solid var(--colors-accent-default, #2563eb)' : data.selected ? '2px solid var(--colors-fg-default, #111)' : data.problem ? '2px solid var(--colors-danger-text, #b91c1c)' : '1px solid var(--colors-border, #ccc)'
  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        border: ring,
        borderStyle: st.kind === 'parallel' && !data.active && !data.selected ? 'dashed' : undefined,
        borderRadius: st.kind === 'final' ? 22 : 8,
        background: container ? 'color-mix(in srgb, var(--colors-bg-subtle, #f4f4f5) 70%, transparent)' : data.active ? 'var(--colors-bg-selected, #dbeafe)' : 'var(--colors-bg-surface, #fff)',
        padding: '4px 8px',
        fontSize: 12,
        color: 'var(--colors-fg-default, #111)',
        boxSizing: 'border-box',
        outline: st.kind === 'final' ? '1px solid var(--colors-border, #ccc)' : undefined,
        outlineOffset: st.kind === 'final' ? 3 : undefined,
      }}
    >
      <Handle type="target" position={Position.Left} />
      <div style={{ fontWeight: 600 }}>
        {st.label} {container ? <span style={{ fontWeight: 400, opacity: 0.7 }}>({st.kind === 'parallel' ? 'parallel: all regions at once' : 'compound'})</span> : null}
      </div>
      {st.task ? <div style={{ opacity: 0.75 }}>Task · {st.task.role} · {st.task.worklist}</div> : null}
      {st.gate ? <div style={{ opacity: 0.75 }}>Gate · {st.gate}</div> : null}
      {st.invoke ? <div style={{ opacity: 0.75 }}>Invokes {st.invoke.split('/').pop()}</div> : null}
      {st.kind === 'final' ? <div style={{ opacity: 0.75 }}>final</div> : null}
      <Handle type="source" position={Position.Right} />
    </div>
  )
}

const nodeTypes = { state: StateNode }

export type CanvasProps = {
  def: Definition
  selected: string | null
  onSelect: (id: string | null) => void
  active?: string[] | undefined
  /** Transitions taken in the simulation step being shown. */
  taken?: string[] | undefined
  /** States and transitions a check complains about. */
  flagged?: string[] | undefined
  onConnect?: ((from: string, to: string) => void) | undefined
  height?: number | string
  compact?: boolean
}

export function WorkflowCanvas({ def, selected, onSelect, active = [], taken = [], flagged = [], onConnect, height = 560, compact = false }: CanvasProps) {
  const boxes = useMemo(() => arrange(def), [def])
  const nodes: Node<StateData>[] = def.states.map((st) => {
    const b = boxes.get(st.id) ?? { x: 0, y: 0, w: LEAF_W, h: 44 }
    return {
      id: st.id,
      type: 'state',
      position: { x: b.x, y: b.y },
      ...(st.parent ? { parentId: st.parent } : {}),
      style: { width: b.w, height: b.h },
      data: { state: st, active: active.includes(st.id), selected: selected === st.id, problem: flagged.includes(st.id) },
      ariaLabel: `${st.kind} state ${st.label}`,
      zIndex: st.parent ? 1 : 0,
    }
  })
  const edges: Edge[] = def.transitions.map((tr) => {
    const hot = taken.includes(tr.id)
    const label = compact ? tr.event : [tr.event === '' ? '(eventless)' : tr.event, guardText(tr.guard), tr.actions.length ? `/ ${tr.actions.length} action${tr.actions.length > 1 ? 's' : ''}` : ''].filter(Boolean).join(' ')
    return {
      id: tr.id,
      source: tr.from,
      target: tr.to,
      label,
      animated: hot,
      selected: selected === tr.id,
      zIndex: 2,
      markerEnd: { type: MarkerType.ArrowClosed },
      style: { strokeWidth: hot || selected === tr.id ? 3 : 1.5, stroke: hot ? 'var(--colors-accent-default, #2563eb)' : flagged.includes(tr.id) ? 'var(--colors-danger-text, #b91c1c)' : undefined, strokeDasharray: tr.event.startsWith('after ') ? '6 4' : undefined },
      labelStyle: { fontSize: 11 },
      labelBgPadding: [4, 2] as [number, number],
      ariaLabel: `Transition from ${tr.from} to ${tr.to} on ${tr.event || 'no event'}`,
    }
  })
  return (
    <div style={{ height, width: '100%' }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        fitView
        forceColorMode="light"
        onNodeClick={(_, n) => onSelect(n.id)}
        onEdgeClick={(_, e) => onSelect(e.id)}
        onPaneClick={() => onSelect(null)}
        onConnect={(c) => onConnect?.(c.source, c.target)}
        nodesConnectable={onConnect !== undefined}
        minZoom={0.2}
        zoomOnScroll={false}
        preventScrolling={false}
      >
        <Background />
        {compact ? null : <Controls />}
      </ReactFlow>
    </div>
  )
}
