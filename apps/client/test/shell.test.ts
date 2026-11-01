import { expect, test } from 'vitest'

import { VersionMismatch, checkVersion, registerShellWorker } from '../src/platform/shell'

function reloader(reloaded: boolean) {
  const calls: string[] = []
  return { calls, reloaded: () => reloaded, reload: () => void calls.push('reload') }
}

test('a node that names no build, or this one, passes the check', () => {
  const page = reloader(false)
  expect(checkVersion(undefined, 'abc', page)).toBe(true)
  expect(checkVersion('abc', 'abc', page)).toBe(true)
  expect(page.calls).toEqual([])
})

test('a shell older than its edge reloads once', () => {
  const page = reloader(false)
  expect(checkVersion('new', 'old', page)).toBe(false)
  expect(page.calls).toEqual(['reload'])
})

test('a mismatch that survives a reload is an error, not a reload loop', () => {
  const page = reloader(true)
  expect(() => checkVersion('new', 'old', page)).toThrow(VersionMismatch)
  expect(page.calls).toEqual([])
})

test('the shell worker registers at the root, and is skipped without service workers', () => {
  const registered: string[] = []
  const serviceWorker = {
    register: (url: string) => {
      registered.push(url)
      return Promise.resolve()
    },
  } as unknown as ServiceWorkerContainer
  registerShellWorker({ serviceWorker })
  registerShellWorker({})
  expect(registered).toEqual(['/sw.js'])
})
