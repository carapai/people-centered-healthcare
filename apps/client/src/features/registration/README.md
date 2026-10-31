# Registration

Reception registration (patient-identity.md § "Did you mean" at registration; #71).

- **`RegistrationPage`** (`/registration`):
  1. The clerk enters the patient.
  2. **Check for this patient** calls `Patient/$register` without confirmation, which writes nothing and returns the candidates.
  3. **Choosing:** the candidates are a DataTable, moved through with the arrow keys and chosen with Enter. Choosing one uses the existing record.
  4. **Registering new:** "Register as a new patient" is enabled only after the clerk ticks "this patient is none of these". The server enforces the same rule.
  5. **Too little to match on:** the clerk searches the registry by name, birth year and village first. That search needs the configured search parameters.
  6. **Done:** a registration shows the MRN and what matching did: linked, queued for review, or a new identity.
- **`patient.ts`** builds the staff-entered Patient. An age gives an estimated birth year, which matching widens by ±5 years.
- **`candidates.ts`** reads a match or search Bundle into rows.
- **Port:** writes go through the `registries` command port (`platform/registries.ts`), never `FhirClient`.
- **Tests:** `test/registration.test.ts` covers the builder, the port and the candidate rows. The keyboard scenario end to end is #228.
