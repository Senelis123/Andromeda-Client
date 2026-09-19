//! Official Minecraft metadata retrieval, caching, and rule evaluation.

mod catalog;
mod metadata;
mod rules;

pub use catalog::{CatalogSource, CatalogUpdate, VersionCatalogService};
pub use metadata::*;
pub use rules::{RuleContext, RuleDecision, evaluate_rules};
