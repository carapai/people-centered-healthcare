# Access

Offline administration on a facility edge (offline-auth.md § Offline user and role administration; #75, part 3c). It is for a facility admin, whose snapshot gives `access.administer-local`.

- **`AccessPage`** (`/access`, linked from home on an edge) has three sections:
  - **Roles granted here:** the unexpired local grants, and a form that grants a role for 1 to 7 days. The roles offered are the edge's `GET /api/access/roles`: the country's locally grantable roles that carry no never-offline permission.
  - **Account for new staff:** a provisional account with a temporary password, a role and a duration. Its user replaces the password at their first sign-in.
  - **Access revoked here:** the revocations, and a form that revokes a user's access at once, after a confirming click.
- **Refusals:**
  - A 403 on the roles list means the user is not an admin. It is shown as "your role does not include administering access here", not as an error.
  - Other refusals show the edge's explanation, which says, for example, that the user is unknown or the username is taken.
- **The first sign-in** of a provisional account is on the sign-in page (`routes/sign-in.tsx`). The server answers `error=change_password`, and the page asks for the temporary password and a new one twice. It posts them through `SignIn.changePassword` to `/auth/password`, then signs in with the new password through the ordinary form post to `/authorize`.
- **`api.ts`** holds the queries and the three posts. `test/access.test.ts` covers them against a fake transport, and `e2e/edge/access.spec.ts` covers the whole flow on a real edge.
