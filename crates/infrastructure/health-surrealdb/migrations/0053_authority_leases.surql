-- Authority Leases (#135; write-authority.md § Hand-over rules): `granted` rows are leases this
-- node (an edge) granted to its desktops; `held` rows are leases this node (a desktop) holds. The
-- lease itself is `data`, as `health_authority_leases::Lease` serialises it; `stores` and `state`
-- repeat it for lookups.
DEFINE TABLE authority_lease SCHEMAFULL;
DEFINE FIELD role ON authority_lease TYPE string
  ASSERT $value IN ['granted', 'held'];
DEFINE FIELD holder ON authority_lease TYPE string;
DEFINE FIELD stores ON authority_lease TYPE array<string>;
DEFINE FIELD state ON authority_lease TYPE string;
DEFINE FIELD data ON authority_lease TYPE string;
DEFINE INDEX authority_lease_stores ON authority_lease FIELDS stores;
