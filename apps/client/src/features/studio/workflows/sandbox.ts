import type { StudioSandbox, StudioSandboxMove, StudioSandboxStep } from './api'

// The Workflow Studio's sandbox (studio.md § The Workflow Studio; #499). The browser holds the
// run as the author's guard answers, gate outcomes and the moves so far, and posts it whole on
// every change: central replays it from the start in the engine's simulation and answers every
// step. Nothing here reads the statechart, so the browser never decides a transition: undo drops
// the last move, restart drops them all, and what is active comes from the server's last step.

/** A sandbox run as the browser holds it. */
export interface Run {
  /** The author's answer to each guard, by its expression. */
  guards: Record<string, boolean>
  /** The outcome of each Financial Gate, by canonical URL; an unanswered gate defers. */
  gates: Record<string, string>
  /** The moves so far, in order. */
  moves: StudioSandboxMove[]
}

export type Outcome = 'pass' | 'defer' | 'block'

export const OUTCOMES: readonly Outcome[] = ['pass', 'defer', 'block']

export const emptyRun = (): Run => ({ guards: {}, gates: {}, moves: [] })

/** Sends `event` next. */
export const send = (run: Run, event: string): Run => ({ ...run, moves: [...run.moves, { event }] })

/** Waits `by` (ISO 8601) next. */
export const wait = (run: Run, by: string): Run => ({ ...run, moves: [...run.moves, { wait: by }] })

/** Takes back the last move. */
export const undo = (run: Run): Run => ({ ...run, moves: run.moves.slice(0, -1) })

/** Starts again, keeping the answers. */
export const restart = (run: Run): Run => ({ ...run, moves: [] })

/** Answers `guard` (`null` takes the answer back). */
export function answer(run: Run, guard: string, holds: boolean | null): Run {
  const guards = { ...run.guards }
  if (holds === null) delete guards[guard]
  else guards[guard] = holds
  return { ...run, guards }
}

/** Sets the outcome of `gate` (`null` takes it back: it defers). */
export function setGate(run: Run, gate: string, outcome: Outcome | null): Run {
  const gates = { ...run.gates }
  if (outcome === null) delete gates[gate]
  else gates[gate] = outcome
  return { ...run, gates }
}

/** How many of the run's moves the server did not take: the first of them needs an answer or
 * was refused, and the rest wait behind it. */
export const untaken = (run: Run, played: StudioSandbox): number => run.moves.length - Math.max(0, played.steps.length - 1)

/** What holds now: the server's last step. */
export const current = (played: StudioSandbox | undefined): StudioSandboxStep | undefined => played?.steps.at(-1)

/** The states active now, to light on the canvas and in the outline. */
export const activeStates = (played: StudioSandbox | undefined): Set<string> => new Set(current(played)?.configuration ?? [])

/** What a move does, for the history. */
export const moveText = (move: StudioSandboxMove): { kind: 'event'; event: string } | { kind: 'wait'; wait: string } =>
  move.event === null || move.event === undefined ? { kind: 'wait', wait: move.wait ?? '' } : { kind: 'event', event: move.event }

/** The path a scenario named `name` is saved at in the package's `tests/` folder, not one of
 * `taken`. */
export function scenarioPath(name: string, taken: Set<string>): string {
  const slug =
    name
      .toLowerCase()
      .replaceAll(/[^a-z0-9]+/g, '-')
      .replaceAll(/^-|-$/g, '') || 'scenario'
  let path = `tests/${slug}.json`
  for (let n = 2; taken.has(path); n++) path = `tests/${slug}-${n}.json`
  return path
}

/** The run as the `workflow-scenario` file the server wrote for it, named `name`. Only the name
 * changes: the steps and what each expects are the server's. */
export function scenarioFile(played: StudioSandbox, name: string): string {
  const file = JSON.parse(played.scenario) as Record<string, unknown>
  return JSON.stringify({ ...file, name: name.trim() }, null, 2)
}
