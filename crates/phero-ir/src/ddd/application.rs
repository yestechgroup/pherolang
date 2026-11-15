use serde::{Deserialize, Serialize};

use crate::ddd::{Design, SearchDef, Service};

/// The designed application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    /// Application name.
    pub name: String,
    /// The default domain package name for unqualified references (e.g.
    /// `"nz.example.library"`), when the design declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

impl Application {
    /// Creates an application without a base package.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base: None,
        }
    }
}

/// A module of the application: a cohesive slice of services, designed
/// classes, and search definitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    /// Module name.
    pub name: String,
    /// Application services declared in this module, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<Service>,
    /// Class designs declared in this module, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub designs: Vec<Design>,
    /// Search definitions declared in this module, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub searches: Vec<SearchDef>,
}

impl Module {
    /// Creates an empty module.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            services: Vec::new(),
            designs: Vec::new(),
            searches: Vec::new(),
        }
    }
}
