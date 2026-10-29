//! FHIR search contracts (fhir-r5-model.md § Search parameters and indexed projections;
//! persistence.md § Search index tables). Pure: no I/O.
//!
//! - [`Catalog`] compiles SearchParameter resources; [`Registry`] holds the parameters this node
//!   supports and whether each is searchable yet.
//! - [`extract`] turns a resource into normalized [`IndexEntry`] values at write time.
//! - [`parse_query`] turns request parameters into a [`SearchQuery`] of [`Clause`]s, strictly.
//! - [`SearchPolicy`] is where the policy decision point restricts every search.
//!
//! The SurrealQL translation lives in `health-surrealdb`.

mod extract;
pub mod normalize;
mod path;
mod policy;
mod query;
mod registry;

pub use extract::{Extraction, IndexEntry, IndexValue, Unindexable, extract};
pub use path::{Node, PathError, SearchPath};
pub use policy::{SearchPolicy, Unrestricted};
pub use query::{
    Clause, Handling, MAX_COUNT, Prefix, SearchError, SearchQuery, SearchValue, StringMatch, Test,
    parse_query,
};
pub use registry::{
    Catalog, Component, NotInCatalog, ParamState, ParamType, Registry, Rejected, SearchParameter,
    Supported,
};
