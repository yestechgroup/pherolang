//! The `.rosetta` semantic walker (Rune, RN16).
//!
//! The Rune AST records element and member spans but stores names as text, so
//! each name is recovered as the identifier token with that text inside the
//! owning node's span: the parser's own token, never a second scan. Token
//! lookups binary-search the sorted token table, because CDM-sized files hold
//! tens of thousands of tokens.

use phero_syntax::rune::ast::{
    AnnotationRef, AttributeDef, ConditionDef, Element, SourceUnit, SpannedQName, TypeCall,
};
use phero_syntax::Span;

use crate::class::{HighlightClass, HighlightModifier, HighlightModifiers, HighlightSpan};
use crate::engine::lexical::{LexToken, LexTokenKind};
use crate::engine::semantic::Ctx;

/// Walks a (possibly partial) `.rosetta` unit.
pub(crate) fn walk(ctx: &mut Ctx, unit: &SourceUnit) {
    let mut walker = Walker { ctx };
    let namespace = unit.namespace.span;
    walker.qname_in(
        namespace,
        &unit.namespace.name.0,
        1,
        HighlightClass::Namespace,
    );
    for import in &unit.imports {
        walker.qname_in(
            import.span,
            &import.namespace.0,
            1,
            HighlightClass::Namespace,
        );
    }
    for configuration in &unit.configurations {
        walker.spanned(&configuration.root, HighlightClass::Type);
    }
    for element in &unit.elements {
        walker.element(element);
    }
}

struct Walker<'c, 'a> {
    ctx: &'c mut Ctx<'a>,
}

impl Walker<'_, '_> {
    fn element(&mut self, element: &Element) {
        let span = element.span();
        match element {
            Element::Data(data) | Element::Choice(data) => {
                self.decl(span, &data.name, HighlightClass::Type);
                if let Some(super_type) = &data.super_type {
                    self.spanned(super_type, HighlightClass::Type);
                }
                self.annotations(&data.annotations);
                for attribute in &data.attributes {
                    self.attribute(attribute, HighlightClass::Property);
                }
                self.conditions(&data.conditions);
            }
            Element::Enumeration(enumeration) => {
                self.decl(span, &enumeration.name, HighlightClass::Type);
                if let Some(super_type) = &enumeration.super_type {
                    self.spanned(super_type, HighlightClass::Type);
                }
                self.annotations(&enumeration.annotations);
                for value in &enumeration.values {
                    self.decl_at(value.span, &value.name, HighlightClass::Constant);
                    self.annotations(&value.annotations);
                }
            }
            Element::Annotation(annotation) => {
                self.decl(span, &annotation.name, HighlightClass::Annotation);
                for attribute in &annotation.attributes {
                    self.attribute(attribute, HighlightClass::Property);
                }
            }
            Element::TypeAlias(alias) => {
                self.decl(span, &alias.name, HighlightClass::Type);
                for parameter in &alias.parameters {
                    self.decl_at(parameter.span, &parameter.name, HighlightClass::Parameter);
                    self.type_call(&parameter.type_call);
                }
                self.type_call(&alias.type_call);
                self.annotations(&alias.annotations);
                self.conditions(&alias.conditions);
            }
            Element::BasicType(basic) => {
                self.decl(span, &basic.name, HighlightClass::Type);
                for parameter in &basic.parameters {
                    self.decl_at(parameter.span, &parameter.name, HighlightClass::Parameter);
                    self.type_call(&parameter.type_call);
                }
            }
            Element::RecordType(record) => {
                self.decl(span, &record.name, HighlightClass::Type);
                for feature in &record.features {
                    self.type_call(&feature.type_call);
                }
            }
            Element::LibraryFunction(function) => {
                self.decl(span, &function.name, HighlightClass::Function);
                for (_, type_call, _) in &function.parameters {
                    self.type_call(type_call);
                }
                self.type_call(&function.return_type);
            }
            Element::Function(function) => {
                self.decl(span, &function.name, HighlightClass::Function);
                if let Some(super_function) = &function.super_function {
                    self.spanned(super_function, HighlightClass::Function);
                }
                self.annotations(&function.annotations);
                for input in &function.inputs {
                    self.attribute(input, HighlightClass::Parameter);
                }
                if let Some(output) = &function.output {
                    self.attribute(output, HighlightClass::Parameter);
                }
                self.conditions(&function.conditions);
                self.conditions(&function.post_conditions);
            }
            Element::Rule(rule) => {
                self.decl(span, &rule.name, HighlightClass::Function);
                if let Some(input) = &rule.input {
                    self.type_call(input);
                }
            }
            Element::Report(report) => {
                self.spanned(&report.body, HighlightClass::Type);
                for corpus in &report.corpora {
                    self.spanned(corpus, HighlightClass::Type);
                }
                self.type_call(&report.input_type);
                for rule in &report.eligibility_rules {
                    self.spanned(rule, HighlightClass::Function);
                }
                self.spanned(&report.report_type, HighlightClass::Type);
                if let Some(source) = &report.rule_source {
                    self.spanned(source, HighlightClass::Type);
                }
            }
            Element::RuleSource(source) => self.decl(span, &source.name, HighlightClass::Type),
            Element::Schema(schema) => self.decl(span, &schema.name, HighlightClass::Type),
            Element::Body(body) => self.decl(span, &body.name, HighlightClass::Type),
            Element::Corpus(corpus) => self.decl(span, &corpus.name, HighlightClass::Type),
            Element::Segment(segment) => self.decl(span, &segment.name, HighlightClass::Type),
            Element::MetaType(meta) => self.decl(span, &meta.name, HighlightClass::Type),
        }
    }

    fn attribute(&mut self, attribute: &AttributeDef, class: HighlightClass) {
        self.decl_at(attribute.span, &attribute.name, class);
        self.type_call(&attribute.type_call);
        self.annotations(&attribute.annotations);
    }

    fn conditions(&mut self, conditions: &[ConditionDef]) {
        for condition in conditions {
            if let Some(name) = &condition.name {
                self.decl(condition.span, name, HighlightClass::Function);
            }
            self.annotations(&condition.annotations);
        }
    }

    fn annotations(&mut self, annotations: &[AnnotationRef]) {
        for annotation in annotations {
            self.qname_in(
                annotation.span,
                &annotation.annotation.0,
                0,
                HighlightClass::Annotation,
            );
        }
    }

    fn type_call(&mut self, type_call: &TypeCall) {
        self.qname_in(type_call.span, &type_call.name.0, 0, HighlightClass::Type);
    }

    fn spanned(&mut self, name: &SpannedQName, last: HighlightClass) {
        self.qname_in(name.span, &name.name.0, 0, last);
    }

    /// An element's declared name: the first identifier with that text after
    /// the element's leading keyword token.
    fn decl(&mut self, within: Span, name: &str, class: HighlightClass) {
        if let Some((start, end)) = self.find(within, name, 1) {
            self.push(start, end, class, true);
        }
    }

    /// A member's declared name: the member's span starts at (or just before)
    /// the name.
    fn decl_at(&mut self, within: Span, name: &str, class: HighlightClass) {
        if let Some((start, end)) = self.find(within, name, 0) {
            self.push(start, end, class, true);
        }
    }

    /// A dotted name inside `within`: the segments as consecutive identifier
    /// tokens (separated by `.`), leading segments as namespaces.
    fn qname_in(&mut self, within: Span, qname: &str, skip: usize, last: HighlightClass) {
        let segments: Vec<&str> = qname.split('.').collect();
        let Some(first) = segments.first() else {
            return;
        };
        let tokens = self.tokens_in(within);
        let Some(head) = tokens
            .iter()
            .enumerate()
            .skip(skip)
            .find(|(_, token)| ident_text(token) == Some(first))
            .map(|(index, _)| index)
        else {
            return;
        };
        let mut found = Vec::with_capacity(segments.len());
        let mut cursor = head;
        for (index, segment) in segments.iter().enumerate() {
            let Some(token) = tokens.get(cursor) else {
                return;
            };
            if ident_text(token) != Some(segment) {
                return;
            }
            found.push((token.start, token.end));
            // Skip the `.` separator to the next segment.
            cursor += if index + 1 < segments.len() { 2 } else { 1 };
        }
        let count = found.len();
        for (index, (start, end)) in found.into_iter().enumerate() {
            let class = if index + 1 == count {
                last
            } else {
                HighlightClass::Namespace
            };
            self.push(start, end, class, false);
        }
    }

    /// The first identifier token with `text` in `within`, skipping the first
    /// `skip` tokens of the span.
    fn find(&self, within: Span, text: &str, skip: usize) -> Option<(usize, usize)> {
        self.tokens_in(within)
            .iter()
            .skip(skip)
            .find(|token| ident_text(token) == Some(text))
            .map(|token| (token.start, token.end))
    }

    /// The tokens fully inside `within`, found by binary search.
    fn tokens_in(&self, within: Span) -> &[LexToken<'_>] {
        let tokens = self.ctx.tokens;
        let from = tokens.partition_point(|token| token.start < within.start);
        let to = tokens.partition_point(|token| token.end <= within.end);
        tokens.get(from..to.max(from)).unwrap_or(&[])
    }

    fn push(&mut self, start: usize, end: usize, class: HighlightClass, declaration: bool) {
        let mut modifiers = HighlightModifiers::new();
        if declaration {
            modifiers.insert(HighlightModifier::Declaration);
        }
        if start < end {
            self.ctx
                .out
                .push(HighlightSpan::with_modifiers(start, end, class, modifiers));
        }
    }
}

/// The text of an identifier token (escaped or not), else `None`.
fn ident_text<'t>(token: &'t LexToken<'_>) -> Option<&'t str> {
    match &token.kind {
        LexTokenKind::Ident { text, .. } => Some(text),
        _ => None,
    }
}
