# crates/edge

Facility Edge Node runtime crates ([crate-catalogue.md](../../docs/architecture/crate-catalogue.md)):
- [`health-edge-runtime`](health-edge-runtime/README.md): the store, the startup and stop sequence, service-manager notifications, supervised tasks and the UPS watch (#72).
- [`health-node-identity`](health-node-identity/README.md): the full-disk encryption check and provisioning gate (#73), composed in `apps/server/facility-edge`.
