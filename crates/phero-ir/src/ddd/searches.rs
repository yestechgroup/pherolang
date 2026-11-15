use serde::{Deserialize, Serialize};

use crate::ddd::CapabilityRef;
use crate::wire::is_false;
use crate::TypeRef;

/// A search definition designed over one entity: the indexed full-text
/// fields, the structured filter and sort keys, the computed document
/// projection, and the ranking/analyzer/pagination knobs.
///
/// Like [`Design::class`](crate::ddd::Design::class), [`SearchDef::entity`]
/// loosely references a
/// `.mox` class (unqualified or package-qualified at this layer);
/// resolution against the domain [`crate::Model`] is a driver/consumer
/// concern, never a wire concern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchDef {
    /// Search name, unique within its module (not validated at the wire
    /// layer, consistent with the other artifacts).
    pub name: String,
    /// Human-readable description of what the search finds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The referenced entity class whose instances are searched.
    pub entity: String,
    /// The resolved identity of [`Self::entity`] (DD1, pherolang#298): the
    /// searched class, when the domain model was supplied at compile time.
    /// Additive; absent when unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_ref: Option<TypeRef>,
    /// Full-text fields, in declaration order: the indexed prose a query's
    /// terms match against, ranked by relevance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<SearchField>,
    /// Structured filter keys, in declaration order: exact-match facets a
    /// query may constrain (never free-text searched).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<SearchFilter>,
    /// Structured sort keys, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SearchSort>,
    /// Computed projection entries of the search document, in declaration
    /// order: named values derived from the matched instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub document: Vec<DocumentField>,
    /// The relevance ranking strategy; `None` lets the consumer choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranking: Option<RankingStrategy>,
    /// The default analyzer applied to the indexed text; per-field
    /// overrides live on [`SearchField::analyzer`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzer: Option<String>,
    /// Pagination knobs; `None` lets the consumer choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
    /// Actor capability names guarding the search (the
    /// [`crate::ActorModel`] dimension); loosely coupled, not validated at
    /// the wire layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// The resolved identities of [`Self::capabilities`] (DD1,
    /// pherolang#298), aligning index-for-index with it: entry `i` is the
    /// [`CapabilityRef`] the compiler resolved `capabilities[i]` to.
    /// [`CapabilityRef::declared_in`] is `None` when no `.actor` artifact was
    /// supplied at compile time. Additive; omitted when the capability list
    /// is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_refs: Vec<CapabilityRef>,
}

impl SearchDef {
    /// Creates a search over `entity` with no resolved entity or capability
    /// references, no text fields, filters, sort keys, document projection,
    /// ranking, analyzer, pagination, or capabilities.
    pub fn new(name: impl Into<String>, entity: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            entity: entity.into(),
            entity_ref: None,
            text: Vec::new(),
            filters: Vec::new(),
            sort: Vec::new(),
            document: Vec::new(),
            ranking: None,
            analyzer: None,
            pagination: None,
            capabilities: Vec::new(),
            capability_refs: Vec::new(),
        }
    }

    /// Chainable setter for the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Chainable setter for the full-text fields.
    pub fn with_text(mut self, text: Vec<SearchField>) -> Self {
        self.text = text;
        self
    }

    /// Chainable setter for the filter keys.
    pub fn with_filters(mut self, filters: Vec<SearchFilter>) -> Self {
        self.filters = filters;
        self
    }

    /// Chainable setter for the sort keys.
    pub fn with_sort(mut self, sort: Vec<SearchSort>) -> Self {
        self.sort = sort;
        self
    }

    /// Chainable setter for the document projection.
    pub fn with_document(mut self, document: Vec<DocumentField>) -> Self {
        self.document = document;
        self
    }

    /// Chainable setter for the ranking strategy.
    pub fn with_ranking(mut self, ranking: RankingStrategy) -> Self {
        self.ranking = Some(ranking);
        self
    }

    /// Chainable setter for the default analyzer.
    pub fn with_analyzer(mut self, analyzer: impl Into<String>) -> Self {
        self.analyzer = Some(analyzer.into());
        self
    }

    /// Chainable setter for the pagination knobs.
    pub fn with_pagination(mut self, pagination: Pagination) -> Self {
        self.pagination = Some(pagination);
        self
    }

    /// Chainable setter for the guarding capabilities.
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// A full-text field of a [`SearchDef`]: the indexed prose of one entity
/// property, optionally weighted and analyzed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchField {
    /// The entity property whose text is indexed.
    pub property: String,
    /// The resolved feature type (DD1, pherolang#298): the declared type of
    /// [`Self::property`] on the search's entity, when the domain model was
    /// supplied at compile time. Additive; absent when unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
    /// Relevance boost multiplier for matches in this field; `None` uses
    /// the consumer's default weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boost: Option<Boost>,
    /// Analyzer override for this field; `None` uses the search's default
    /// ([`SearchDef::analyzer`], else the consumer's default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzer: Option<String>,
}

impl SearchField {
    /// Creates a field with no resolved feature type, no boost, and no
    /// analyzer override.
    pub fn new(property: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            type_ref: None,
            boost: None,
            analyzer: None,
        }
    }

    /// Chainable setter for the relevance boost.
    pub fn with_boost(mut self, boost: impl Into<Boost>) -> Self {
        self.boost = Some(boost.into());
        self
    }

    /// Chainable setter for the analyzer override.
    pub fn with_analyzer(mut self, analyzer: impl Into<String>) -> Self {
        self.analyzer = Some(analyzer.into());
        self
    }
}

crate::wire::total_order_float! {
    /// The relevance-boost weight of a [`SearchField`] (`boost <n>` in the
    /// source): an ordered, equality-preserving wrapper around `f32`, so the
    /// artifact chain derives [`Eq`] end to end (wire contract rule 14).
    ///
    /// # Wire layout
    ///
    /// `#[serde(transparent)]`: a `Boost` serializes and deserializes exactly
    /// as the bare JSON number it wraps — artifacts are byte-identical to the
    /// raw-`f32` encoding.
    ///
    /// # Equality and hashing
    ///
    /// [`PartialEq`]/[`Eq`] follow `f32::total_cmp` semantics: equality is
    /// bit-pattern equality, which makes it reflexive even for `NaN` (a `NaN`
    /// equals only another `NaN` with the same bit pattern) and distinguishes
    /// `-0.0` from `0.0` (both deliberate). [`Hash`] hashes the raw bit
    /// pattern, consistent with that equality.
    ///
    /// # Ordering
    ///
    /// [`Ord`]/[`PartialOrd`] are `f32::total_cmp`: a genuine total order
    /// (`-0.0 < 0.0`, every `NaN` above [`f32::INFINITY`]), so boosts can be
    /// sorted and compared like any other key.
    ///
    /// # Where `NaN`/`-0.0` can come from
    ///
    /// The `.ddd` grammar accepts only integer boosts (`boost 3`) and JSON
    /// cannot encode `NaN`, so neither source nor wire artifacts produce one
    /// today; the `NaN`/`-0.0` behavior above is the type's deliberate
    /// contract for programmatic construction and any future float-tolerant
    /// syntax, not dead-letter handling.
    Boost(f32)
}

/// A structured filter key of a [`SearchDef`]: an exact-match facet a query
/// may constrain. A struct, not a bare string, so the wire format can grow
/// per-filter knobs without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilter {
    /// The entity property the filter constrains.
    pub property: String,
    /// The resolved feature type (DD1, pherolang#298): the declared type of
    /// [`Self::property`] on the search's entity, when the domain model was
    /// supplied at compile time. Additive; absent when unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
}

impl SearchFilter {
    /// Creates a filter key with no resolved feature type.
    pub fn new(property: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            type_ref: None,
        }
    }
}

/// A structured sort key of a [`SearchDef`]. A struct, not a bare string,
/// so the wire format can grow per-key knobs without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSort {
    /// The entity property the sort orders by.
    pub property: String,
    /// The resolved feature type (DD1, pherolang#298): the declared type of
    /// [`Self::property`] on the search's entity, when the domain model was
    /// supplied at compile time. Additive; absent when unresolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_ref: Option<TypeRef>,
}

impl SearchSort {
    /// Creates a sort key with no resolved feature type.
    pub fn new(property: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            type_ref: None,
        }
    }
}

/// A computed projection entry of a [`SearchDef`]'s document: one named
/// value of the search result, derived from the matched instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentField {
    /// The entry's name in the search document.
    pub name: String,
    /// The verbatim expression source computing the value (the same
    /// convention as a domain feature's `expr` body); typed against the
    /// entity by the consumer, never at the wire layer.
    pub expr: String,
}

impl DocumentField {
    /// Creates a document field from its name and expression source.
    pub fn new(name: impl Into<String>, expr: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            expr: expr.into(),
        }
    }
}

/// The relevance ranking strategy of a [`SearchDef`].
///
/// Adjacently tagged (the crate's `{"type": ..., "value": ...}` rule):
/// unit variants serialize without content — exactly `{"type": "bm25"}`,
/// `{"type": "tfIdf"}`, `{"type": "exact"}` — and the newtype variant as
/// `{"type": "custom", "value": "..."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum RankingStrategy {
    /// BM25 relevance scoring.
    Bm25,
    /// TF-IDF relevance scoring.
    TfIdf,
    /// Exact-match filtering only; no relevance ranking.
    Exact,
    /// A consumer-provided strategy, referenced by name — loosely coupled,
    /// like every cross-artifact reference in this module.
    Custom(String),
}

/// The pagination knobs of a [`SearchDef`]. A fully-default value
/// serializes as `{}` (the field itself is optional on the search).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// The default page size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// The maximum page size a query may request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_limit: Option<u32>,
    /// The search pages by cursor (keyset) rather than by offset.
    #[serde(default, skip_serializing_if = "is_false")]
    pub cursor: bool,
}
