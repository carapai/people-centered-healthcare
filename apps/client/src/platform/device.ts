/**
 * The device's key (lan-security.md § Device enrollment; ADR 0009): an ECDSA P-256 pair whose
 * private key the browser will not export, kept in IndexedDB. It proves the device in every
 * request to the edge (DPoP). Clearing the browser's storage loses it, and the device must enroll
 * again.
 */
export interface DeviceKeys {
  /** The key pair, once the device has one. */
  current(): CryptoKeyPair | undefined
  /** Makes a new key pair and keeps it, replacing any other. */
  create(): Promise<CryptoKeyPair>
  /** Forgets the key pair, so the device can enroll afresh (after a revocation). */
  forget(): Promise<void>
}

/** Where the key pair is kept. */
export interface KeyStorage {
  get(): Promise<CryptoKeyPair | undefined>
  put(keys: CryptoKeyPair): Promise<void>
  remove(): Promise<void>
}

/** The device's keys over `storage`, loading any it already has. */
export async function deviceKeys(storage: KeyStorage): Promise<DeviceKeys> {
  let current = await storage.get().catch(() => undefined)
  return {
    current: () => current,
    async create() {
      const keys = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign', 'verify'])
      await storage.put(keys)
      current = keys
      return keys
    },
    async forget() {
      await storage.remove()
      current = undefined
    },
  }
}

/** For tests and for runtimes with nowhere to keep a key. */
export function memoryKeyStorage(): KeyStorage {
  let kept: CryptoKeyPair | undefined
  return {
    get: () => Promise.resolve(kept),
    put: (keys) => {
      kept = keys
      return Promise.resolve()
    },
    remove: () => {
      kept = undefined
      return Promise.resolve()
    },
  }
}

const DATABASE = 'health-device'
const STORE = 'keys'
const KEY = 'device'

function database(): Promise<IDBDatabase> {
  return new Promise((done, fail) => {
    const open = indexedDB.open(DATABASE, 1)
    open.addEventListener('upgradeneeded', () => open.result.createObjectStore(STORE))
    open.addEventListener('success', () => done(open.result))
    open.addEventListener('error', () => fail(open.error))
  })
}

async function transact<T>(mode: IDBTransactionMode, work: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await database()
  try {
    return await new Promise<T>((done, fail) => {
      const request = work(db.transaction(STORE, mode).objectStore(STORE))
      request.addEventListener('success', () => done(request.result))
      request.addEventListener('error', () => fail(request.error))
    })
  } finally {
    db.close()
  }
}

/** IndexedDB, which keeps a CryptoKey as it is: the private key stays non-extractable. */
export function indexedDbKeyStorage(): KeyStorage {
  return {
    async get() {
      const kept = await transact<unknown>('readonly', (store) => store.get(KEY))
      return kept === undefined ? undefined : (kept as CryptoKeyPair)
    },
    async put(keys) {
      await transact('readwrite', (store) => store.put(keys, KEY))
    },
    async remove() {
      await transact('readwrite', (store) => store.delete(KEY))
    },
  }
}
