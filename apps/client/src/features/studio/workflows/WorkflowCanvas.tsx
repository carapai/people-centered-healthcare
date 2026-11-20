import { css, cx } from '@health/design-system/css'
import { Background, Controls, Handle, MarkerType, Position, ReactFlow, type Edge as FlowEdge, type Node, type NodeProps } from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { useMemo } from 'react'
import { useIntl } from 'react-intl'

import { workflowMessages as m } from '../../../i18n/messages'
import { arrange, LEAF_WIDTH } from './layout'
import { activityOf, edgeId, edges, rows, triggerOf, type Selection, type State, type WorkflowDefinition } from './model'

// The canvas (studio.md § The Workflow Studio): the statechart on @xyflow/react, laid out left
// to right by `layout.ts`. Compound and parallel states are group nodes holding their children;
// dragging from one state's right handle to another adds a transition. Problems are outlined in
// red. React Flow's attribution link stays (studio.md). Zooming on scroll is off, so the page
// scrolls over the canvas. Everything here can also be done from the outline and the inspector
// with the keyboard alone (WCAG 2.2 AA, 2.5.7 Dragging Movements).

interface StateData extends Record<string, unknown> {
  state: State
  flagged: boolean
  detail: string
}

const node = css({
  width: '100%',
  height: '100%',
  boxSizing: 'border-box',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: 'md',
  bg: 'bg.surface',
  color: 'fg.default',
  px: '2',
  py: '1',
  fontSize: 'xs',
  '&[data-group=true]': { bg: 'bg.subtle' },
  '&[data-kind=parallel]': { borderStyle: 'dashed' },
  '&[data-kind=final]': { borderRadius: '2xl', borderWidth: '3px', borderStyle: 'double' },
  '&[data-flagged=true]': { borderColor: 'danger.text', borderWidth: '2px' },
  '.selected > &': { outline: '3px solid', outlineColor: 'focusRing', outlineOffset: '1px' },
})

function StateNode({ data }: NodeProps<Node<StateData>>) {
  const { state } = data
  const group = (state.children ?? []).length > 0
  return (
    <div className={node} data-group={group} data-kind={state.kind} data-flagged={data.flagged}>
      <Handle type="target" position={Position.Left} />
      <div className={css({ fontWeight: 'semibold', overflowWrap: 'anywhere' })}>{state.id}</div>
      {data.detail === '' ? null : <div className={css({ color: 'fg.muted' })}>{data.detail}</div>}
      <Handle type="source" position={Position.Right} />
    </div>
  )
}

const nodeTypes = { state: StateNode }

export interface CanvasProps {
  definition: WorkflowDefinition
  selected: Selection | null
  /** The ids of the states and transitions a check is about. */
  flagged: Set<string>
  onSelect: (selection: Selection | null) => void
  /** Adds a transition; absent when the draft is read only. */
  onConnect?: ((source: string, target: string) => void) | undefined
}

export function WorkflowCanvas({ definition, selected, flagged, onSelect, onConnect }: CanvasProps) {
  const intl = useIntl()
  const boxes = useMemo(() => arrange(definition), [definition])
  const selectedId = selected === null ? null : selected.kind === 'state' ? selected.id : edgeId(selected.source, selected.index)
  const describe = (state: State) => {
    const activity = activityOf(state)
    if (activity === 'task') return intl.formatMessage(m.wsNodeTask, { role: state.task?.role ?? '', worklist: state.task?.worklist ?? '' })
    if (activity === 'gate') return intl.formatMessage(m.wsNodeGate)
    if (activity === 'invoke') return intl.formatMessage(m.wsNodeInvoke)
    if ((state.children ?? []).length > 0) return intl.formatMessage(state.kind === 'parallel' ? m.wsNodeParallel : m.wsNodeCompound)
    return state.kind === 'final' ? intl.formatMessage(m.wsNodeFinal) : ''
  }
  const nodes: Node<StateData>[] = rows(definition).map(({ state, parent, depth }) => {
    const box = boxes.get(state.id) ?? { x: 0, y: 0, width: LEAF_WIDTH, height: 44 }
    return {
      id: state.id,
      type: 'state',
      position: { x: box.x, y: box.y },
      ...(parent === null ? {} : { parentId: parent, extent: 'parent' as const }),
      style: { width: box.width, height: box.height },
      width: box.width,
      height: box.height,
      data: { state, flagged: flagged.has(state.id), detail: describe(state) },
      selected: selectedId === state.id,
      draggable: false,
      ariaLabel: intl.formatMessage(m.wsNodeLabel, { id: state.id, kind: state.kind }),
      zIndex: depth,
    }
  })
  const lines: FlowEdge[] = edges(definition).flatMap(({ source, index, transition }) => {
    const state = rows(definition).find((r) => r.state.id === source)?.state
    if (state === undefined) return []
    const trigger = triggerOf(state, transition)
    const on = trigger.kind === 'eventless' ? intl.formatMessage(m.wsEventless) : trigger.kind === 'timer' ? intl.formatMessage(m.wsAfter, { after: trigger.after }) : trigger.events.join(', ')
    const label = transition.guard === undefined || transition.guard === null || transition.guard === '' ? on : `${on} [${transition.guard}]`
    const id = edgeId(source, index)
    return (transition.targets ?? []).map((target, n) => ({
      id: n === 0 ? id : `${id}·${n}`,
      source,
      target,
      label,
      selected: selectedId === id,
      zIndex: 20,
      markerEnd: { type: MarkerType.ArrowClosed },
      className: cx(flagged.has(id) ? css({ '& path': { stroke: 'danger.text!' } }) : undefined, trigger.kind === 'timer' ? css({ '& path': { strokeDasharray: '6 4' } }) : undefined),
      ariaLabel: intl.formatMessage(m.wsEdgeLabel, { source, target, on }),
    }))
  })
  const choose = (id: string) => {
    const at = id.split('·')[0] ?? id
    const edge = edges(definition).find((e) => edgeId(e.source, e.index) === at)
    onSelect(edge === undefined ? { kind: 'state', id } : { kind: 'transition', source: edge.source, index: edge.index })
  }
  return (
    <div className={css({ height: '36rem', width: '100%', border: '1px solid', borderColor: 'border', borderRadius: 'md' })}>
      <ReactFlow
        nodes={nodes}
        edges={lines}
        nodeTypes={nodeTypes}
        fitView
        forceColorMode="light"
        // Selecting by click or by Enter on a focused node or edge.
        onNodesChange={(changes) => {
          for (const change of changes.getByType('select')) if (change.selected) choose(change.id)
        }}
        onEdgesChange={(changes) => {
          for (const change of changes.getByType('select')) if (change.selected) choose(change.id)
        }}
        onPaneClick={() => onSelect(null)}
        onConnect={(connection) => onConnect?.(connection.source, connection.target)}
        nodesConnectable={onConnect !== undefined}
        nodesDraggable={false}
        elementsSelectable
        minZoom={0.2}
        zoomOnScroll={false}
        preventScrolling={false}
        aria-label={intl.formatMessage(m.wsCanvas)}
      >
        <Background />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  )
}
