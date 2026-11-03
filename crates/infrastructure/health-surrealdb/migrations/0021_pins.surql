-- PINs and the idle lock (offline-auth.md § Credentials, § Tokens and sessions; #75): a user's
-- PIN on one device, keyed [account, device key thumbprint], its hash sealed under the node's
-- pepper key; and each session's account and device, to find a user's session on a device when
-- they resume or switch to it. Owned by identity.
DEFINE TABLE auth_pin SCHEMAFULL;
DEFINE FIELD credential ON auth_pin TYPE string;
DEFINE FIELD failures ON auth_pin TYPE int;
DEFINE FIELD account ON auth_session TYPE option<string>;
DEFINE FIELD jkt ON auth_session TYPE option<string>;
DEFINE INDEX auth_session_device ON auth_session FIELDS account, jkt;
