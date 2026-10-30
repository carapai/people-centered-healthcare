/** The FHIR base URLs of the two nodes the contract suite runs against. */
export interface ContractServers {
  central: string
  edge: string
}

declare module 'vitest' {
  export interface ProvidedContext {
    servers: ContractServers
  }
}
