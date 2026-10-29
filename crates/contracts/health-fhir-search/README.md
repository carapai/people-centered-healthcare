# health-fhir-search

Layer: **contracts**. Pure FHIR search contracts (fhir-r5-model.md § Search parameters and indexed projections; persistence.md § Search index tables). The SurrealQL translation lives in `health-surrealdb`.

- **`Catalog`** compiles SearchParameter resources from the R5 core package and configuration packages. Expressions use the search subset of FHIRPath (`SearchPath`): unions of paths with `ofType`/`as`, `[n]`, `first()`, `.(a | b)`, `extension('url')`, `where(resolve() is T)` and `where(field = 'literal')`. Of the 1,244 R5 core SearchParameters it compiles 1,211; the rest (11 `special`, expressions that compute such as `deceased.exists() and deceased != false`, and the core package's example) are rejected with a reason and cannot be supported.
- **`Registry`** holds the parameters this node supports per resource type, each `Active` or `Reindexing`, and the node's offset for dates without one.
- **`extract`** turns a resource into normalized `IndexEntry` values: tokens as system + code, strings case- and accent-folded (plus the exact text), dates as a half-open range at their precision, references as type + id, quantities and numbers as exact decimals (failing closed beyond 28 digits, ADR 0052), URIs as stored, and composites as tuples from the same element. Supported parameters without a value are listed for `:missing`.
- **`parse_query`** parses request parameters into a `SearchQuery`, strictly: unknown parameters, unsupported modifiers and prefixes, chains and `_has` are errors; `Handling::Lenient` only ignores unknown parameters. Result-control parameters (`_sort`, `_include`, …) are passed through for the REST layer.
- **`SearchPolicy`** is the policy decision point's hook: clauses it adds are ANDed into every search.

Not yet supported, and refused: `:text`, `:of-type`, `:identifier`, `:in`, `:not-in`, `:above`, `:below`, chaining and `_has`. Quantities are matched on their stated unit; UCUM canonicalization (`octofhir-ucum`) is not applied yet.

Set `HEALTH_FHIR_R5_PACKAGE` to an unpacked `hl7.fhir.r5.core#5.0.0/package` to compile the whole core package in the tests.
