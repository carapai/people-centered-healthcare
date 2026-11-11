/** The nodes the contract suite runs against: central, an edge, and a desktop over IPC. */
export interface ContractServers {
  central: string
  edge: string
  /** The desktop node: its FHIR base over HTTP (for fixtures) and its IPC endpoint. */
  desktop: { base: string; ipc: string }
}

declare module 'vitest' {
  export interface ProvidedContext {
    servers: ContractServers
  }
}
