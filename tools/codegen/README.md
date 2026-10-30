# @health/codegen

The generators of the client's types (client-application.md § Types).

```sh
bun run generate   # central's OpenAPI document, @health/api-client and @health/fhir-types
bun run check      # generate, then fail if anything differs from the committed files (CI)
bun run test       # the OpenAPI generator's tests
```

- **`src/fhir-types.ts`** reads `health-fhir-validation`'s R5 element table and writes `packages/fhir-types/src/r5.ts`. It generates one interface per resource and data type, a nested interface per backbone element, the choice-type expansions, the `_name` elements, the `FhirResource` union and `ResourceTypeMap`.
- **`src/openapi.ts`** reads an OpenAPI document (`cargo run -p central-server -- openapi`) and writes Zod schemas, `z.infer` types and an `operations` table.
  - It accepts the JSON Schema that utoipa emits for our DTOs: objects, string enums, scalars, arrays, maps, `$ref`, `oneOf`/`anyOf` with `null`, and single-member `allOf`.
  - Anything else, a recursive schema, or an operation without an `operationId` fails the generation with its location.

Both print text rather than use the TypeScript compiler API, which TypeScript 7 does not have. Their output is deterministic.
