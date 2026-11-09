//! Package activation on this edge (#89): `health-activation`, run after each sync round with
//! the scope chain from `HEALTH_DISTRICT`, `HEALTH_FACILITY` and `HEALTH_PROGRAMS`, against the
//! national root installed at provisioning (`root.pem` in `HEALTH_PKI_DIR`).

pub use health_activation::{Activator, Outcome, Report, activate};
