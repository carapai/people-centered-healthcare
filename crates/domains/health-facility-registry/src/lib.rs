//! The facility registry Module (ADR 0094; facility-configuration.md): the place hierarchy and
//! where a user's area comes from.
//!
//! - **Places** ([`Place`], [`Places`]): facilities, districts, regions and the one national
//!   place, each linked to the one above it (facility → district → region → national). A place's
//!   **area** is it and everything below it ([`Places::area`]); the facilities in it are what a
//!   district's or region's figures roll up over ([`Places::facilities_in`]). The reserved code
//!   [`NATIONAL`] (ADR 0056's `central`) is the national place.
//! - **Administrative areas** ([`AdministrativeAreas`]): the regions and districts the national
//!   core package ships (a `Library` of type [`AREAS_TYPE`]), and their boundaries
//!   ([`AdministrativeBoundaries`], a GeoJSON `Library` of type [`BOUNDARIES_TYPE`]; #604), which
//!   [`import_boundaries`] reads from a boundary file and checks against the hierarchy.
//! - **The NHFR import** ([`parse_nhfr_csv`], [`plan_import`]): the NHFR's facility list, read
//!   and compared with the registry before anything is written: what is new, changed or closed.
//!   A facility is linked to its district by code, and a district the core package does not hold
//!   refuses the import. A closed facility becomes inactive and is never removed. The NHFR's
//!   DHIS2 org unit ID for a facility, when the export gives one, is kept on its place.
//! - **HMIS reporting settings** ([`FacilityReporting`]; ADR 0069, #560): each facility's HMIS
//!   org unit ID and the forms reported through the platform, set at central only, the ID
//!   checked against the NHFR's ([`org_unit_flags`]).
//!
//! The registry is global and read-only on edges (write-authority.md). Places are stored as FHIR
//! Organizations (and a facility's coordinates on a Location); the application layer maps them.

mod areas;
mod boundaries;
mod import;
mod nhfr;
mod place;
mod reporting;

pub use areas::{AREAS_CONTENT, AREAS_TYPE, AdministrativeAreas, Area, District};
pub use boundaries::{
    AdministrativeBoundaries, BOUNDARIES_CONTENT, BOUNDARIES_TYPE, Boundary, BoundaryFeature,
    BoundaryProperties, Geometry, Ring, import_boundaries,
};
pub use import::{Change, ImportPlan, ImportRefused, plan_import};
pub use nhfr::{NhfrFacility, parse_nhfr_csv};
pub use place::{Coordinates, NATIONAL, Place, PlaceLevel, Places, location_id, place_id};
pub use reporting::{
    FacilityReporting, FacilityReportingStore, OrgUnitFlag, is_dhis2_uid, org_unit_flags,
};

use std::future::Future;

use health_persistence::StoreError;

/// Reads the registry's places as stored: every Organization's JSON. Implemented over a
/// transaction by the persistence adapter.
pub trait PlaceStore {
    /// Every Organization in the registry, deleted ones left out.
    fn organizations(&mut self) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;
}
