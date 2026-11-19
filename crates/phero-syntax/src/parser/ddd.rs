//! The `.ddd` design grammar: services, designs, repositories,
//! machines, searches, modules, and the root/fragment application file
//! parser.

use std::iter::once;

use crate::ddd;
use crate::lexer::{lex_with_comments, Token};

use super::docs::attach_ddd_docs;
use super::*;
/// A contextual `.ddd` keyword: an ordinary identifier that is special only
/// in the grammar position it is parsed here in (the `schema`/`to`
/// precedent). Escaped forms (`^service`) are different token variants and
/// never match.
pub(crate) fn ddd_keyword<'src>(
    keyword: &'static str,
) -> impl Parser<'src, Tokens<'src>, Span, MoxExtra<'src>> + Clone {
    select! { Token::Ident(text) = e if text == keyword => e.span() }
}

/// An `import "<path>"` declaration of a `.ddd` file. Unlike the `.actor`
/// production this tolerates a trailing `;`, which the canonical `.ddd`
/// formatting emits on every import line.
pub(crate) fn ddd_import_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, Import, MoxExtra<'src>> + Clone {
    kw(Token::Import)
        .ignore_then(string_lit())
        .then_ignore(kw(Token::Other(';')).or_not())
        .map_with(|path, e| Import {
            path,
            alias: None,
            span: e.span(),
        })
}

/// The `capability <name> (, <name>)*` clause of a service operation. The
/// clause is committed once the `capability` keyword matched: a missing name
/// list is a syntax error (with recovery), not a failed alternative.
pub(crate) fn ddd_capabilities<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<Name>, MoxExtra<'src>> + Clone {
    kw(Token::Capability)
        .ignore_then(name().or_not())
        .then(
            kw(Token::Comma)
                .ignore_then(name())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, rest), e| (first, rest, e.span()))
        .validate(|(first, rest, span), _e, emitter| {
            if first.is_none() {
                emitter.emit(Rich::custom(
                    span,
                    "`capability` requires at least one capability name",
                ));
            }
            first.into_iter().chain(rest).collect::<Vec<_>>()
        })
}

/// The `emits <Event> (, <Event>)*` clause of a service operation: the
/// events the operation produces. The `emits` keyword is contextual (the
/// [`ddd::EMITS`] identifier, not a lexer token), so an operation may be
/// named `emits` and an emitted event may be named `emits` — the keyword
/// position is unambiguous. The clause is committed once the keyword
/// matched: a missing name list is a syntax error (with recovery), not a
/// failed alternative. Names are loosely coupled: the `.ddd` compile has
/// no `.evt` bucket, so existence/coverage resolve in the union integrity
/// pass.
pub(crate) fn ddd_emits<'src>() -> impl Parser<'src, Tokens<'src>, Vec<Name>, MoxExtra<'src>> + Clone
{
    ddd_keyword(ddd::EMITS)
        .ignore_then(name().or_not())
        .then(
            kw(Token::Comma)
                .ignore_then(name())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, rest), e| (first, rest, e.span()))
        .validate(|(first, rest, span), _e, emitter| {
            if first.is_none() {
                emitter.emit(Rich::custom(
                    span,
                    "`emits` requires at least one event name",
                ));
            }
            first.into_iter().chain(rest).collect::<Vec<_>>()
        })
}

/// The `throws <Error> (, <Error>)*` clause of a declared/delegated service
/// operation and a declared repository operation (DD7, issue
/// pherolang#304): the error vocabulary the operation can raise. The
/// `throws` keyword is contextual (the [`ddd::THROWS`] identifier, not a
/// lexer token), so an operation may be named `throws` — the keyword
/// position is unambiguous. The clause is committed once the keyword
/// matched: a missing name list is a syntax error (with recovery), not a
/// failed alternative. Entries are bare names; a qualified spelling and an
/// unknown name are driver diagnostics.
pub(crate) fn ddd_throws<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<Name>, MoxExtra<'src>> + Clone {
    // Each entry is scanned as a qualified name so a dotted spelling gets a
    // **clear** diagnostic rather than a generic `expected ';'`: the error
    // vocabulary is application-wide unique, so the bare last segment is the
    // canonical spelling.
    let entry =
        qname()
            .map_with(|qname, e| (qname, e.span()))
            .validate(|(qname, span), _e, emitter| {
                let segments = qname.segments.len();
                let last = qname.segments.last().cloned().unwrap_or(Name {
                    text: String::new(),
                    span,
                    escaped: false,
                });
                if segments > 1 {
                    emitter.emit(Rich::custom(
                        span,
                        format!(
                            "`throws` entries are bare error names; error '{}' is referenced \
                         application-wide, so write `{}`",
                            last.text, last.text
                        ),
                    ));
                }
                last
            });
    ddd_keyword(ddd::THROWS)
        .ignore_then(entry.clone().or_not())
        .then(
            kw(Token::Comma)
                .ignore_then(entry)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, rest), e| (first, rest, e.span()))
        .validate(|(first, rest, span), _e, emitter| {
            if first.is_none() {
                emitter.emit(Rich::custom(
                    span,
                    "`throws` requires at least one error name",
                ));
            }
            first.into_iter().chain(rest).collect::<Vec<_>>()
        })
}

/// One `error <Name>(<field> (, <field>)*);` declaration of an application
/// or module body (DD7, issue pherolang#304). The `error` word is the body
/// raise word reused at declaration position — contextual like every other
/// `.ddd` word — and the payload reuses the operation-parameter production
/// (`type_ref multiplicity? name`). The `error*` declarations lead their
/// enclosing body: `base? error* module*` at application scope, `error*
/// module_member*` at module scope (the driver reports a late declaration).
pub(crate) fn ddd_error_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddErrorDecl, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::ERROR)
        .ignore_then(name())
        .then(ddd_params())
        .then_ignore(kw(Token::Other(';')))
        .map_with(|(name, fields), e| DddErrorDecl {
            name,
            fields,
            doc: None,
            span: e.span(),
        })
}

/// The `version <pin>` clause of a decision-service reference (pherolang#154):
/// `version compatible` or `version "<X.Y.Z>"`. The `version` discriminator
/// is the existing lexer keyword token (the `.mox` vocabulary keyword — no
/// new keyword, escape rules unchanged); `compatible` is the contextual
/// word (an ordinary identifier everywhere else). The clause is committed
/// once `version` matched: a missing pin value is a reported error (with
/// recovery), never a silent drop.
pub(crate) fn ddd_version_pin<'src>(
) -> impl Parser<'src, Tokens<'src>, DddVersionPin, MoxExtra<'src>> + Clone {
    kw(Token::Version)
        .ignore_then(
            choice((
                ddd_keyword(ddd::COMPATIBLE).to(DddVersionPin::Compatible),
                string_lit().map(DddVersionPin::Exact),
            ))
            .or_not(),
        )
        .map_with(|pin, e| (pin, e.span()))
        .validate(|(pin, span), _e, emitter| {
            pin.unwrap_or_else(|| {
                emitter.emit(Rich::custom(
                    span,
                    "`version` requires `compatible` or an \"X.Y.Z\" version string",
                ));
                DddVersionPin::Compatible
            })
        })
}

/// The `invokes <DecisionService> (, <DecisionService>)*` clause of a
/// service operation: the decision services the operation invokes. The
/// `invokes` keyword is contextual (the [`ddd::INVOKES`] identifier, not a
/// lexer token), so an operation may be named `invokes` — the keyword
/// position is unambiguous. The clause is committed once the keyword
/// matched: a missing name list is a syntax error (with recovery), not a
/// failed alternative. Names are loosely coupled: the `.ddd` compile
/// resolves them against the provided `.dmn` artifacts only (the decisions
/// entry point), never at the wire layer. Each entry may carry a `version`
/// pin (pherolang#154) — optional, per entry; entries without one keep the
/// pin-less shape.
pub(crate) fn ddd_invokes<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<DddDecisionRef>, MoxExtra<'src>> + Clone {
    let entry = qname()
        .then(ddd_version_pin().or_not())
        .map(|(name, pin)| DddDecisionRef { name, pin });
    ddd_keyword(ddd::INVOKES)
        .ignore_then(entry.clone().or_not())
        .then(
            kw(Token::Comma)
                .ignore_then(entry)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map_with(|(first, rest), e| (first, rest, e.span()))
        .validate(|(first, rest, span), _e, emitter| {
            if first.is_none() {
                emitter.emit(Rich::custom(
                    span,
                    "`invokes` requires at least one decision service name",
                ));
            }
            first.into_iter().chain(rest).collect::<Vec<_>>()
        })
}

/// One `"(" (param ("," param)*)? ")"` parameter list of a `.ddd` operation
/// signature, shared by declared service and repository operations and the
/// body forms' optional-return signatures.
pub(crate) fn ddd_params<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<DddParam>, MoxExtra<'src>> + Clone {
    let param = tref().then(multiplicity().or_not()).then(name()).map_with(
        |((type_ref, multiplicity), name), e| DddParam {
            type_ref,
            multiplicity,
            name,
            span: e.span(),
        },
    );
    let list = param
        .clone()
        .then(
            kw(Token::Comma)
                .ignore_then(param)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| once(first).chain(rest).collect::<Vec<_>>());
    kw(Token::LParen)
        .ignore_then(list.or_not())
        .then_ignore(kw(Token::RParen))
        .map(|list| list.unwrap_or_default())
}

/// A `type_ref multiplicity? name "(" params? ")"` operation signature,
/// shared verbatim by service and repository operations.
pub(crate) fn ddd_signature<'src>() -> impl Parser<
    'src,
    Tokens<'src>,
    (TypeRef, Option<Multiplicity>, Name, Vec<DddParam>),
    MoxExtra<'src>,
> + Clone {
    tref()
        .then(multiplicity().or_not())
        .then(name())
        .then(ddd_params())
        .map(|(((type_ref, multiplicity), name), params)| (type_ref, multiplicity, name, params))
}

/// The optional-return signature of a body operation (issue pherolang#178):
/// `(type_ref multiplicity?)? name "(" params? ")"`. A body operation may be
/// void (`submit(Draft draft) => Repository { ... }`) or value-returning
/// (`boolean approve() => Repository { ... }`); the no-type alternative is
/// unambiguous because the operation name is the name immediately before the
/// parameter list.
pub(crate) fn ddd_body_signature<'src>() -> impl Parser<
    'src,
    Tokens<'src>,
    (Option<TypeRef>, Option<Multiplicity>, Name, Vec<DddParam>),
    MoxExtra<'src>,
> + Clone {
    let typed = tref()
        .then(multiplicity().or_not())
        .then(name())
        .then(ddd_params())
        .map(|(((type_ref, multiplicity), name), params)| {
            (Some(type_ref), multiplicity, name, params)
        });
    let untyped = name()
        .then(ddd_params())
        .map(|(name, params)| (None, None, name, params));
    typed.or(untyped)
}

/// The `<target>.<operation>` delegation target of a delegated service
/// operation. [`qname`] cannot know where the dependency name ends (a
/// following `.` might continue a package-qualified target), so the dotted
/// pair is parsed as one qualified name and split at its last segment. Once
/// the `=>` matched the operation is committed: a bare single-segment name
/// (no `.`) recovers with an empty target and a reported error.
pub(crate) fn ddd_delegation<'src>(
) -> impl Parser<'src, Tokens<'src>, DddDelegation, MoxExtra<'src>> + Clone {
    qname().map_with(|qname, e| (qname, e.span())).validate(
        |(qname, span), _e, emitter| match qname.segments.split_last() {
            Some((operation, target_segments)) if !target_segments.is_empty() => DddDelegation {
                target: QualifiedName {
                    segments: target_segments.to_vec(),
                    span: (qname.span.start..operation.span.start).into(),
                },
                operation: operation.clone(),
                span: qname.span,
            },
            _ => {
                emitter.emit(Rich::custom(span, "expected `dependency.operation`"));
                DddDelegation {
                    target: QualifiedName {
                        segments: Vec::new(),
                        span: (span.end..span.end).into(),
                    },
                    operation: Name {
                        text: String::new(),
                        span,
                        escaped: false,
                    },
                    span,
                }
            }
        },
    )
}

/// The `transition <Trigger>` (or `transition <EnumType>.<Trigger>`)
/// clause of a delegated service operation (issue pherolang#176): the
/// explicit transition binding. The `transition` keyword is contextual
/// (the [`ddd::TRANSITION`] identifier, not a lexer token), so an operation
/// may be named `transition` — the keyword position is unambiguous. The
/// clause is committed once the keyword matched: a missing trigger is a
/// syntax error (with recovery), not a failed alternative. The spelling is
/// one name or two dotted names — `PayRunStatus.Reverse` splits at the dot
/// into the machine qualifier (the enum type name) and the trigger; a
/// third segment is a syntax error.
pub(crate) fn ddd_transition_clause<'src>(
) -> impl Parser<'src, Tokens<'src>, DddTransitionRef, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::TRANSITION)
        .ignore_then(name().or_not())
        .then(kw(Token::Dot).ignore_then(name()).or_not())
        .map_with(|(first, second), e| (first, second, e.span()))
        .validate(|(first, second, span), _e, emitter| {
            // `transition Submit`: the trigger only. `transition
            // PayRunStatus.Reverse`: the qualifier (the machine's enum type
            // name) precedes the dot, the trigger follows it.
            match (first, second) {
                (Some(qualifier), Some(trigger)) => DddTransitionRef {
                    trigger,
                    machine: Some(qualifier),
                    span,
                },
                (Some(trigger), None) => DddTransitionRef {
                    trigger,
                    machine: None,
                    span,
                },
                (None, _) => {
                    emitter.emit(Rich::custom(span, "`transition` requires a trigger name"));
                    DddTransitionRef {
                        trigger: Name {
                            text: String::new(),
                            span,
                            escaped: false,
                        },
                        machine: None,
                        span,
                    }
                }
            }
        })
}

/// Which body form a `/`-statement block belongs to: the diagnostic noun
/// (`"service"`/`"repository"`) the statement-subset refusal embeds.
#[derive(Clone, Copy)]
pub(crate) enum DddBodyKind {
    Service,
    Repository,
}

impl DddBodyKind {
    fn label(self) -> &'static str {
        match self {
            DddBodyKind::Service => "service",
            DddBodyKind::Repository => "repository",
        }
    }
}

/// Captures the token stream of one `{ ... }` statement body, outer braces
/// excluded, with each token's file-absolute span. Only brace nesting is
/// tracked here; the statement parser walks parentheses, brackets, and
/// braces itself (the body's contents are not a `.mox` expression).
pub(crate) fn ddd_body_tokens<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<(Token<'src>, Span)>, MoxExtra<'src>> + Clone {
    let balanced = recursive(|balanced| {
        let atom = select! {
            t if !matches!(t, Token::LBrace | Token::RBrace) => t
        }
        .map_with(|token, e| vec![(token, e.span())]);
        let group = select! { Token::LBrace = e => (Token::LBrace, e.span()) }
            .map(|pair| vec![pair])
            .then(balanced.clone())
            .then(select! { Token::RBrace = e => (Token::RBrace, e.span()) }.map(|pair| vec![pair]))
            .map(|((mut open, mut inner), mut close)| {
                open.append(&mut inner);
                open.append(&mut close);
                open
            });
        atom.or(group)
            .repeated()
            .collect::<Vec<_>>()
            .map(|parts| parts.into_iter().flatten().collect::<Vec<_>>())
    });
    kw(Token::LBrace)
        .ignore_then(balanced)
        .then_ignore(kw(Token::RBrace))
}

/// The executable statement body of a service/repository operation (issue
/// pherolang#178): `{ <statements> }`. The closed subset is parsed by
/// [`parse_ddd_body`]; every refusal is emitted as a `Rich` parser error
/// naming the construct, so a `while` in a body is a syntax error, never a
/// silent drop.
pub(crate) fn ddd_body<'src>(
    kind: DddBodyKind,
) -> impl Parser<'src, Tokens<'src>, DddBody, MoxExtra<'src>> + Clone {
    ddd_body_tokens()
        .map_with(|tokens, e| (tokens, e.span()))
        .validate(move |(tokens, span), _e, emitter| {
            let (body, errors) = parse_ddd_body(&tokens, kind, span);
            for (span, message) in errors {
                emitter.emit(Rich::custom(span, message));
            }
            body
        })
}

/// Which token terminates a raw expression slot: the depth-0 statement `;`
/// (also stopping before the body's closing `}`), the lambda-like `else` of
/// a let/guard, the `{` opening an `if` statement's block, or the `,`/`)` of
/// a raise argument.
#[derive(Clone, Copy)]
enum ExprStop {
    Semi,
    SemiOrElse,
    Brace,
    Arg,
}

/// The manual statement parser over a captured body token slice. The outer
/// `.ddd` parser captures the balanced brace body as tokens (spans are
/// already file-absolute), so this walker needs no chumsky nesting: it can
/// stop expression slots at the right depth-0 token, recognize an
/// `if`-expression atom (whose braces are not a statement block), and emit
/// named subset refusals.
struct BodyParser<'t, 'src> {
    tokens: &'t [(Token<'src>, Span)],
    pos: usize,
    kind: DddBodyKind,
    fallback: Span,
    errors: Vec<(Span, String)>,
}

fn parse_ddd_body(
    tokens: &[(Token<'_>, Span)],
    kind: DddBodyKind,
    fallback: Span,
) -> (DddBody, Vec<(Span, String)>) {
    let mut parser = BodyParser {
        tokens,
        pos: 0,
        kind,
        fallback,
        errors: Vec::new(),
    };
    let stmts = parser.parse_block_contents();
    (DddBody { stmts }, parser.errors)
}

impl<'src> BodyParser<'_, 'src> {
    fn peek(&self) -> Option<&(Token<'src>, Span)> {
        self.tokens.get(self.pos)
    }

    fn at(&self, pred: impl Fn(&Token<'src>) -> bool) -> bool {
        self.peek().is_some_and(|(token, _)| pred(token))
    }

    fn at_ident(&self, word: &str) -> bool {
        self.at(|token| matches!(token, Token::Ident(text) if *text == word))
    }

    fn take(&mut self, pred: impl Fn(&Token<'src>) -> bool) -> bool {
        if self.at(pred) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn take_ident(&mut self, word: &str) -> bool {
        self.take(|token| matches!(token, Token::Ident(text) if *text == word))
    }

    fn error_here(&mut self, message: impl Into<String>) {
        let span = self.peek().map(|(_, span)| *span).unwrap_or(self.fallback);
        self.errors.push((span, message.into()));
    }

    fn span_from(&self, start: usize) -> Span {
        if self.pos > start {
            (self.tokens[start].1.start..self.tokens[self.pos - 1].1.end).into()
        } else {
            self.fallback
        }
    }

    fn name_at(&self, index: usize) -> Option<Name> {
        match self.tokens.get(index)? {
            (Token::Ident(text), span) => Some(Name {
                text: (*text).to_string(),
                span: *span,
                escaped: false,
            }),
            (Token::IdentEscaped(text), span) => Some(Name {
                text: (*text).to_string(),
                span: *span,
                escaped: true,
            }),
            _ => None,
        }
    }

    fn parse_name(&mut self) -> Option<Name> {
        let name = self.name_at(self.pos)?;
        self.pos += 1;
        Some(name)
    }

    fn parse_block_contents(&mut self) -> Vec<DddStmt> {
        let mut stmts = Vec::new();
        while let Some((token, _)) = self.peek() {
            if matches!(token, Token::RBrace) {
                break;
            }
            let before = self.pos;
            if let Some(stmt) = self.parse_statement() {
                stmts.push(stmt);
            }
            // Guarantee forward progress even after a refusable construct.
            if self.pos == before {
                self.pos += 1;
            }
        }
        stmts
    }

    /// The statement dispatch. `None` means the statement was refused (the
    /// named error is recorded and the cursor resynchronized).
    ///
    /// The deliberate exclusions are **named** refusals, not the generic
    /// unsupported-statement message: loops (`for`/`while`), `try`/`catch`,
    /// compound assignment, collection mutation, and non-binding expression
    /// statements are the orchestration-only cap DD7 states, so a body says
    /// exactly which construct it refused and where the construct belongs.
    fn parse_statement(&mut self) -> Option<DddStmt> {
        if self.at_ident(ddd::LET) {
            return self.parse_let();
        }
        if self.at_ident(ddd::GUARD) {
            return self.parse_guard();
        }
        if self.at_ident(ddd::IF) {
            return self.parse_if();
        }
        if self.at_ident(ddd::RETURN) {
            return self.parse_return();
        }
        if self.at_ident("for") || self.at_ident("while") {
            let word = if self.at_ident("for") { "for" } else { "while" };
            self.error_here(format!(
                "unsupported statement '{word}' in a {} body: loops (`for`, `while`) are \
                 outside the orchestration-only subset; move the iteration into a domain \
                 `op` the body calls",
                self.kind.label()
            ));
            self.resync();
            return None;
        }
        if self.at_ident("try") || self.at_ident("catch") {
            let word = if self.at_ident("try") { "try" } else { "catch" };
            self.error_here(format!(
                "unsupported statement '{word}' in a {} body: `try`/`catch` is outside the \
                 orchestration-only subset — a body raises an error, it never catches one",
                self.kind.label()
            ));
            self.resync();
            return None;
        }
        if let Some((path, next)) = self.path_ahead() {
            match self.tokens.get(next).map(|(token, _)| token) {
                Some(Token::Eq) => {
                    self.pos = next + 1; // the `=`
                    let Some(expr) = self.scan_expr(ExprStop::Semi) else {
                        self.error_here("expected an expression after `=`");
                        self.resync();
                        return None;
                    };
                    if !self.take(|token| matches!(token, Token::Other(';'))) {
                        self.error_here("expected `;` to end the assignment");
                        self.resync();
                    }
                    return Some(DddStmt::Assign { path, expr });
                }
                Some(Token::Other(operator @ ('+' | '-' | '*' | '/' | '%')))
                    if matches!(
                        self.tokens.get(next + 1).map(|(token, _)| token),
                        Some(Token::Eq)
                    ) =>
                {
                    self.error_here(format!(
                        "unsupported compound assignment '{operator}=' in a {} body: the \
                         orchestration-only subset has no compound assignment; assign the \
                         computed value to a feature path (`<local>.<feature> = …`)",
                        self.kind.label()
                    ));
                    self.resync();
                    return None;
                }
                Some(Token::LParen) => {
                    let callee = path.last().map(|name| name.text.as_str()).unwrap_or("");
                    if matches!(callee, "push" | "remove" | "clear") {
                        self.error_here(format!(
                            "unsupported collection mutation '{callee}' in a {} body: the \
                             orchestration-only subset mutates no collection; move the \
                             mutation into a domain `op` the body calls",
                            self.kind.label()
                        ));
                    } else {
                        self.error_here(format!(
                            "unsupported expression statement in a {} body: bind the call's \
                             result (`let <name> = <expression>;`) — the orchestration-only \
                             subset has no bare expression statements",
                            self.kind.label()
                        ));
                    }
                    self.resync();
                    return None;
                }
                _ => {}
            }
        }

        // Anything else is outside the closed subset: name the construct and
        // resynchronize at the next depth-0 `;` (or the closing `}`).
        let construct = match self.peek() {
            Some((Token::Ident(text), _)) => format!("'{text}'"),
            Some((Token::IdentEscaped(text), _)) => format!("'^{text}'"),
            Some((token, _)) => format!("{token}"),
            None => "end of body".to_string(),
        };
        self.error_here(format!(
            "unsupported statement {construct} in a {} body; supported: let, \
             assignment, guard, if, return",
            self.kind.label()
        ));
        self.resync();
        None
    }

    /// Looks ahead for `name ("." name)*`; on success returns the path and the
    /// index of the token after it, without moving the cursor. The caller
    /// decides whether the following token makes it an assignment, a
    /// compound assignment, an expression statement, or ordinary junk.
    fn path_ahead(&self) -> Option<(Vec<Name>, usize)> {
        let mut index = self.pos;
        let mut path = Vec::new();
        loop {
            path.push(self.name_at(index)?);
            index += 1;
            if matches!(self.tokens.get(index), Some((Token::Dot, _))) {
                index += 1;
                continue;
            }
            break;
        }
        Some((path, index))
    }

    fn parse_let(&mut self) -> Option<DddStmt> {
        self.pos += 1; // `let`
        let Some(name) = self.parse_name() else {
            self.error_here("expected a name after `let`");
            self.resync();
            return None;
        };
        if !self.take(|token| matches!(token, Token::Eq)) {
            self.error_here("expected `=` after the let name");
            self.resync();
            return None;
        }
        let Some(expr) = self.scan_expr(ExprStop::SemiOrElse) else {
            self.error_here("expected an expression after `=` in `let`");
            self.resync();
            return None;
        };
        let raise = if self.at_ident(ddd::ELSE) {
            self.pos += 1;
            Some(self.parse_raise())
        } else {
            None
        };
        self.expect_semi();
        Some(DddStmt::Let { name, expr, raise })
    }

    fn parse_guard(&mut self) -> Option<DddStmt> {
        self.pos += 1; // `guard`
        let Some(expr) = self.scan_expr(ExprStop::SemiOrElse) else {
            self.error_here("expected a guard condition");
            self.resync();
            return None;
        };
        if !self.take_ident(ddd::ELSE) {
            self.error_here("expected `else error ...` after the guard condition");
            self.resync();
            return None;
        }
        let raise = self.parse_raise();
        self.expect_semi();
        Some(DddStmt::Guard { expr, raise })
    }

    fn parse_if(&mut self) -> Option<DddStmt> {
        self.pos += 1; // `if`
        let Some(cond) = self.scan_expr(ExprStop::Brace) else {
            self.error_here("expected an if condition");
            self.resync();
            return None;
        };
        if !self.take(|token| matches!(token, Token::LBrace)) {
            self.error_here("expected `{` after the if condition");
            self.resync();
            return None;
        }
        let then = self.parse_block_contents();
        if !self.take(|token| matches!(token, Token::RBrace)) {
            self.error_here("expected `}` to close the if branch");
        }
        let else_ = if self.at_ident(ddd::ELSE) {
            self.pos += 1;
            if !self.take(|token| matches!(token, Token::LBrace)) {
                self.error_here("expected `{` after `else`");
                Vec::new()
            } else {
                let statements = self.parse_block_contents();
                if !self.take(|token| matches!(token, Token::RBrace)) {
                    self.error_here("expected `}` to close the else branch");
                }
                statements
            }
        } else {
            Vec::new()
        };
        Some(DddStmt::If { cond, then, else_ })
    }

    fn parse_return(&mut self) -> Option<DddStmt> {
        self.pos += 1; // `return`
        if self.take(|token| matches!(token, Token::Other(';'))) {
            return Some(DddStmt::Return { expr: None });
        }
        let Some(expr) = self.scan_expr(ExprStop::Semi) else {
            self.error_here("expected an expression or `;` after `return`");
            self.resync();
            return None;
        };
        self.expect_semi();
        Some(DddStmt::Return { expr: Some(expr) })
    }

    fn parse_raise(&mut self) -> DddRaise {
        let start = self.pos;
        if !self.take_ident(ddd::ERROR) {
            self.error_here("expected `error` before the raise variant");
        }
        let variant = self.parse_name().unwrap_or_else(|| Name {
            text: String::new(),
            span: self.fallback,
            escaped: false,
        });
        if !self.take(|token| matches!(token, Token::LParen)) {
            self.error_here("expected `(` after the error variant");
        }
        let mut args = Vec::new();
        if !self.at(|token| matches!(token, Token::RParen)) {
            loop {
                let Some(arg) = self.scan_expr(ExprStop::Arg) else {
                    self.error_here("expected an error argument");
                    break;
                };
                args.push(arg);
                if self.take(|token| matches!(token, Token::Comma)) {
                    continue;
                }
                break;
            }
        }
        if !self.take(|token| matches!(token, Token::RParen)) {
            self.error_here("expected `)` to close the error raise");
        }
        DddRaise {
            variant,
            args,
            span: self.span_from(start),
        }
    }

    fn expect_semi(&mut self) {
        if self.take(|token| matches!(token, Token::Other(';'))) {
            return;
        }
        if self.at(|token| matches!(token, Token::RBrace)) {
            // Missing terminator before the block close: recover at the
            // brace, the way the member junk parsers do.
            self.error_here("expected `;` to end the statement");
            return;
        }
        self.error_here("expected `;` to end the statement");
        self.resync();
    }

    /// Skips the rest of a malformed statement: to the next depth-0 `;`
    /// (consumed) or the enclosing `}` (left for the block loop).
    fn resync(&mut self) {
        let mut depth = 0usize;
        while let Some((token, _)) = self.peek() {
            match token {
                Token::LParen | Token::LBracket | Token::LBrace => {
                    depth += 1;
                    self.pos += 1;
                }
                Token::RParen | Token::RBracket | Token::RBrace => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    self.pos += 1;
                }
                Token::Other(';') if depth == 0 => {
                    self.pos += 1;
                    return;
                }
                _ => self.pos += 1,
            }
        }
    }

    /// Captures one raw expression slot: tokens up to the depth-0 terminator
    /// `stop`, balancing `()`, `[]`, and `{}`. At depth 0 an `if` *atom*
    /// (`if cond { ... } else { ... }`) is consumed whole, so its braces and
    /// its `else` never terminate the enclosing slot.
    fn scan_expr(&mut self, stop: ExprStop) -> Option<Span> {
        let start = self.pos;
        let mut depth = 0usize;
        while let Some((token, _)) = self.peek() {
            if depth == 0 {
                let terminates = match (stop, token) {
                    (_, Token::RBrace) => true,
                    (ExprStop::Semi | ExprStop::SemiOrElse, Token::Other(';')) => true,
                    (ExprStop::SemiOrElse, Token::Ident(text)) => *text == ddd::ELSE,
                    (ExprStop::Brace, Token::LBrace) => true,
                    (ExprStop::Arg, Token::Comma | Token::RParen) => true,
                    _ => false,
                };
                if terminates {
                    break;
                }
                if matches!(token, Token::Ident(text) if *text == ddd::IF) {
                    self.skip_if_expression();
                    continue;
                }
            }
            match token {
                Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                Token::RParen | Token::RBracket | Token::RBrace => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            self.pos += 1;
        }
        if self.pos == start {
            None
        } else {
            Some(self.span_from(start))
        }
    }

    /// Consumes one expression-level `if cond { then } else { else }` atom
    /// starting at the `if` token. The inner condition and branches are raw
    /// (balanced braces); a malformed atom leaves the cursor after whatever
    /// matched, and the expression slot still ends at its own terminator.
    fn skip_if_expression(&mut self) {
        self.pos += 1; // `if`
        self.scan_expr(ExprStop::Brace);
        if self.at(|token| matches!(token, Token::LBrace)) {
            self.skip_braced_group();
        }
        if self.at_ident(ddd::ELSE) {
            self.pos += 1;
            if self.at(|token| matches!(token, Token::LBrace)) {
                self.skip_braced_group();
            }
        }
    }

    fn skip_braced_group(&mut self) {
        let mut depth = 0usize;
        while let Some((token, _)) = self.peek() {
            match token {
                Token::LBrace => {
                    depth += 1;
                    self.pos += 1;
                }
                Token::RBrace => {
                    depth = depth.saturating_sub(1);
                    self.pos += 1;
                    if depth == 0 {
                        return;
                    }
                }
                _ => self.pos += 1,
            }
        }
    }
}

/// One operation of a `service` body: an optional `protected` modifier, then
/// a declared signature
/// (`type_ref multiplicity? name "(" params? ")" capability*? invokes? emits?
/// transition? throws? ";"`)
/// or a delegation (`name "=>" target.operation capability*? invokes? emits?
/// transition? throws? ";"`).
/// The two alternatives are distinguished by the `=>` after the leading name;
/// chumsky backtracks, so a delegated op may be named `save` and a declared
/// op's type may be any contextual keyword. Both forms carry the clause
/// sequence `capability? invokes? emits? transition? throws? when?` (the
/// canonical clause order the formatter emits; the `transition` binding is
/// issue pherolang#176's, the `throws` vocabulary DD7's); repository
/// operations take none of the `capability`/`invokes`/`emits`/`transition`
/// clauses — the wire artifact's repository operations carry neither, so
/// accepting one would silently drop data.
pub(crate) fn ddd_service_op<'src>(
) -> impl Parser<'src, Tokens<'src>, DddServiceOp, MoxExtra<'src>> + Clone {
    let declared = ddd_signature()
        .then(ddd_capabilities().or_not())
        .then(ddd_invokes().or_not())
        .then(ddd_emits().or_not())
        .then(ddd_transition_clause().or_not())
        .then(ddd_throws().or_not())
        .then(ddd_guard_clause().or_not())
        .then_ignore(kw(Token::Other(';')))
        .map_with(
            |(
                (
                    (
                        ((((type_ref, multiplicity, name, params), capabilities), invokes), emits),
                        transition,
                    ),
                    throws,
                ),
                condition,
            ),
             e| {
                DddServiceOp {
                    name,
                    return_type: Some(type_ref),
                    multiplicity,
                    params,
                    delegation: None,
                    is_protected: false,
                    capabilities: capabilities.unwrap_or_default(),
                    invokes: invokes.unwrap_or_default(),
                    emits: emits.unwrap_or_default(),
                    transition,
                    throws: throws.unwrap_or_default(),
                    condition,
                    body: None,
                    body_receiver: None,
                    span: e.span(),
                }
            },
        );
    let delegated = name()
        .then_ignore(kw(Token::FatArrow))
        .then(ddd_delegation())
        .then(ddd_capabilities().or_not())
        .then(ddd_invokes().or_not())
        .then(ddd_emits().or_not())
        .then(ddd_transition_clause().or_not())
        .then(ddd_throws().or_not())
        .then(ddd_guard_clause().or_not())
        .then_ignore(kw(Token::Other(';')))
        .map_with(
            |(
                ((((((name, delegation), capabilities), invokes), emits), transition), throws),
                condition,
            ),
             e| {
                DddServiceOp {
                    name,
                    return_type: None,
                    multiplicity: None,
                    params: Vec::new(),
                    delegation: Some(delegation),
                    is_protected: false,
                    capabilities: capabilities.unwrap_or_default(),
                    invokes: invokes.unwrap_or_default(),
                    emits: emits.unwrap_or_default(),
                    transition,
                    throws: throws.unwrap_or_default(),
                    condition,
                    body: None,
                    body_receiver: None,
                    span: e.span(),
                }
            },
        );
    // The body form (issue pherolang#178): `Type? name(params) clauses? =>
    // <Repository> { statements }`. The receiver is one name (the primary
    // repository bound as `self`); the following `{` is what distinguishes
    // the form from a delegation, which continues with `.operation`.
    let body = ddd_body_signature()
        .then(ddd_capabilities().or_not())
        .then(ddd_invokes().or_not())
        .then(ddd_emits().or_not())
        .then(ddd_transition_clause().or_not())
        .then(ddd_throws().or_not())
        .then(ddd_guard_clause().or_not())
        .then_ignore(kw(Token::FatArrow))
        .then(qname())
        .then(ddd_body(DddBodyKind::Service))
        .map_with(
            |(
                (
                    (
                        (((((signature, capabilities), invokes), emits), transition), throws),
                        condition,
                    ),
                    receiver,
                ),
                body,
            ),
             e| {
                let (return_type, multiplicity, name, params) = signature;
                let segments = receiver.segments.len();
                let receiver_span = receiver.span;
                let body_receiver = receiver.segments.last().cloned();
                (
                    DddServiceOp {
                        name,
                        return_type,
                        multiplicity,
                        params,
                        delegation: None,
                        is_protected: false,
                        capabilities: capabilities.unwrap_or_default(),
                        invokes: invokes.unwrap_or_default(),
                        emits: emits.unwrap_or_default(),
                        transition,
                        throws: throws.unwrap_or_default(),
                        condition,
                        body: Some(body),
                        body_receiver,
                        span: e.span(),
                    },
                    receiver_span,
                    segments,
                )
            },
        )
        .validate(|(operation, receiver_span, segments), _e, emitter| {
            if segments != 1 {
                emitter.emit(Rich::custom(
                    receiver_span,
                    "the body receiver must be a single repository name; the body \
                     binds one repository as `self`",
                ));
            }
            operation
        });
    ddd_keyword(ddd::PROTECTED)
        .or_not()
        .then(body.or(delegated).or(declared))
        .map_with(|(protected, mut op), e| {
            op.is_protected = protected.is_some();
            // The span covers the whole operation, modifier included.
            op.span = e.span();
            op
        })
}

/// An `inject <name>;` or `inject decision <qualified-name> [version
/// <pin>];` line of a `service` body. After the `inject` keyword the parser
/// attempts the contextual `decision` discriminator ([`ddd::DECISION`]):
/// present, the rest of the line is a qualified decision-service name (the
/// `.dmn` dimension) with its optional `version` pin (pherolang#154);
/// absent, the plain dependency-name form. Chumsky backtracks, so
/// `inject decision;` — a plain dependency literally named `decision` —
/// still parses as the plain form (the decision form requires a name
/// behind the discriminator).
pub(crate) fn ddd_inject_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddServiceMember, MoxExtra<'src>> + Clone {
    let decision = ddd_keyword(ddd::DECISION)
        .ignore_then(
            qname()
                .then(ddd_version_pin().or_not())
                .map(|(name, pin)| DddDecisionRef { name, pin }),
        )
        .map(DddServiceMember::InjectDecision);
    let plain = name().map(DddServiceMember::Inject);
    ddd_keyword(ddd::INJECT)
        .ignore_then(decision.or(plain))
        .then_ignore(kw(Token::Other(';')))
}

/// One member of a `service` body.
#[derive(Clone)]
pub(crate) enum DddServiceMember {
    Operation(Box<DddServiceOp>),
    Inject(Name),
    InjectDecision(DddDecisionRef),
}

/// Consumes a run of tokens that cannot start or continue a service member,
/// stopping before the body's closing `}` and before identifiers (which
/// could begin the next operation or `inject` line). Always consumes at
/// least one token.
pub(crate) fn junk_ddd_service_member<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a service member"));
        })
}

/// One member of a `service` body with junk recovery.
pub(crate) fn ddd_service_member<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddServiceMember>, MoxExtra<'src>> + Clone {
    choice((
        ddd_service_op().map(|operation| DddServiceMember::Operation(Box::new(operation))),
        ddd_inject_decl(),
    ))
    .map(Some)
    .or(junk_ddd_service_member().to(None))
}

/// A `service <name> { ... }` declaration of a `module`. Doc comments (`///`
/// runs) directly above it become the service description after parsing.
pub(crate) fn ddd_service_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddService, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::SERVICE)
        .ignore_then(name())
        .then_ignore(kw(Token::LBrace))
        .then(ddd_service_member().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|(name, members), e| {
            let mut operations = Vec::new();
            let mut dependencies = Vec::new();
            let mut decision_dependencies = Vec::new();
            for member in members {
                match member {
                    Some(DddServiceMember::Operation(operation)) => {
                        operations.push(*operation);
                    }
                    Some(DddServiceMember::Inject(name)) => dependencies.push(name),
                    Some(DddServiceMember::InjectDecision(name)) => {
                        decision_dependencies.push(name);
                    }
                    None => {}
                }
            }
            DddService {
                name,
                doc: None,
                operations,
                dependencies,
                decision_dependencies,
                span: e.span(),
            }
        })
}

/// The `("abstract")? stereotype` head of a design declaration.
pub(crate) fn ddd_design_head<'src>(
) -> impl Parser<'src, Tokens<'src>, (Option<Span>, DddStereotype), MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::ABSTRACT).or_not().then(ddd_stereotype())
}

/// The stereotype keyword of a design head: the `choice` alternatives come
/// from [`ddd::STEREOTYPES`] in the table's order, so the parse order and
/// the table order (the formatter's dispatch set) agree by construction.
pub(crate) fn ddd_stereotype<'src>(
) -> impl Parser<'src, Tokens<'src>, DddStereotype, MoxExtra<'src>> + Clone {
    choice(ddd::STEREOTYPES.map(|(keyword, stereotype)| ddd_keyword(keyword).to(stereotype)))
}

/// The design flags of a design declaration: any subset of the
/// [`ddd::DESIGN_FLAGS`] keywords in any order, each optionally negated
/// (`!auditable` — the opt-out from a flag that defaults on; the canonical
/// order is the table's, a formatter concern). Repeats are idempotent.
pub(crate) fn ddd_flags<'src>() -> impl Parser<'src, Tokens<'src>, DddFlags, MoxExtra<'src>> + Clone
{
    kw(Token::Other('!'))
        .or_not()
        .then(select! {
            // chumsky's `select!` takes a boolean guard expression, so the
            // lookup runs twice; the `unwrap` is the guard's `Some`.
            Token::Ident(text) = e if ddd::flag_kind(text).is_some() => {
                (ddd::flag_kind(text).unwrap(), e.span())
            }
        })
        .repeated()
        .collect::<Vec<_>>()
        .map(|pairs| {
            let mut flags = DddFlags::default();
            for (bang, (kind, span)) in pairs {
                // A negated occurrence spans from the `!` through the
                // keyword, so diagnostics point at the whole `!flag` form.
                let span = match bang {
                    Some(bang) => (bang.start..span.end).into(),
                    None => span,
                };
                flags.push(
                    kind,
                    DddFlagUse {
                        negated: bang.is_some(),
                        span,
                    },
                );
            }
            flags
        })
}

/// One parsed DTO suffix of a design declaration (issue pherolang#300): the
/// projection source of a `dto Name from Source { ... }`, or the standalone
/// `dto Name { ... }` member block. [`DddDtoSuffix::members`] is `None` only
/// for the recovered `from <source>` form written without a block (a parse
/// error naming the missing block); the other forms always carry members.
pub(crate) struct DddDtoSuffix {
    /// The `from <source>` class, when written.
    pub source: Option<QualifiedName>,
    /// The `{ ... }` members, when written.
    pub members: Option<DddDtoMembers>,
    /// Whether the `from` word was written (to name the right refusal on a
    /// non-DTO design).
    pub from_written: bool,
    /// Span of the whole suffix.
    pub span: Span,
}

/// Consumes one token that cannot start or continue a projection member,
/// stopping before the block's closing `}` and before identifiers (which
/// could begin the next member).
pub(crate) fn junk_ddd_projection_member<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected a projection member (`name;` or `name = <expression>;`); a \
                 declared `type name;` member belongs in a standalone `dto Name { ... }` \
                 block",
            ));
        })
}

/// One member of a DTO projection block (issue pherolang#300):
/// `name ";"` (select) or `name "=" raw_expr ";"` (assign). The expression
/// is the same raw capture the search `document` clause uses. Junk recovery
/// names the projection shape, so a declared `Type name;` member is a
/// reported error rather than a silent drop.
pub(crate) fn ddd_projection_member<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddProjectionMember>, MoxExtra<'src>> + Clone {
    let assign = name()
        .then_ignore(kw(Token::Eq))
        .then(ddd_raw_expr())
        .map_with(|(name, expr), e| DddProjectionMember::Assign {
            name,
            expr,
            span: e.span(),
        });
    let select = name()
        .then_ignore(kw(Token::Other(';')))
        .map_with(|name, e| DddProjectionMember::Select {
            name,
            span: e.span(),
        });
    choice((assign, select))
        .map(Some)
        .or(junk_ddd_projection_member().to(None))
}

/// The `{ (name ";" | name "=" raw_expr ";")* }` block of a `from` DTO
/// projection (issue pherolang#300).
pub(crate) fn ddd_projection_block<'src>(
) -> impl Parser<'src, Tokens<'src>, DddDtoMembers, MoxExtra<'src>> + Clone {
    kw(Token::LBrace)
        .ignore_then(ddd_projection_member().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map(|members| DddDtoMembers::Projection(members.into_iter().flatten().collect()))
}

/// Consumes one token that cannot start or continue a declared DTO member,
/// stopping before the block's closing `}` and before identifiers (which
/// could begin the next member).
pub(crate) fn junk_ddd_declared_member<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected a declared member (`type name;`); the `name;` and \
                 `name = <expression>;` shorthand belongs in a projection \
                 (`dto Name from Source { ... }`)",
            ));
        })
}

/// One `type_ref multiplicity? name ";"` member of a standalone DTO block
/// (issue pherolang#300), sharing the operation-parameter productions.
pub(crate) fn ddd_declared_member<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddDeclaredMember>, MoxExtra<'src>> + Clone {
    let member = tref()
        .then(multiplicity().or_not())
        .then(name())
        .then_ignore(kw(Token::Other(';')))
        .map_with(|((type_ref, multiplicity), name), e| DddDeclaredMember {
            type_ref,
            multiplicity,
            name,
            span: e.span(),
        });
    member.map(Some).or(junk_ddd_declared_member().to(None))
}

/// The `{ (type_ref multiplicity? name ";")* }` block of a standalone DTO
/// (issue pherolang#300).
pub(crate) fn ddd_standalone_block<'src>(
) -> impl Parser<'src, Tokens<'src>, DddDtoMembers, MoxExtra<'src>> + Clone {
    kw(Token::LBrace)
        .ignore_then(ddd_declared_member().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map(|members| DddDtoMembers::Declared(members.into_iter().flatten().collect()))
}

/// The optional DTO suffix of a design declaration (issue pherolang#300):
/// `from qualified_name projection_block` or `standalone_block`. The suffix
/// is only legal after a `dto` stereotype; the design parser reports it as a
/// named error on any other stereotype. `from` is matched as the existing
/// lexer keyword token ([`Token::From`]) — the [`ddd::FROM`] constant names
/// its text.
pub(crate) fn ddd_dto_suffix<'src>(
) -> impl Parser<'src, Tokens<'src>, DddDtoSuffix, MoxExtra<'src>> + Clone {
    let projection = kw(Token::From)
        .ignore_then(qname())
        .then(ddd_projection_block().or_not())
        .map_with(|(source, members), e| DddDtoSuffix {
            source: Some(source),
            members,
            from_written: true,
            span: e.span(),
        });
    let standalone = ddd_standalone_block().map_with(|members, e| DddDtoSuffix {
        source: None,
        members: Some(members),
        from_written: false,
        span: e.span(),
    });
    projection.or(standalone)
}

/// A design declaration: `("abstract")? stereotype name flag* boundary?
/// invariant* dto_suffix? ("repository" ...)? ("machine" ...)?`.
/// The optional authored aggregate-boundary clause of a design (DD4, issue
/// pherolang#301): `root` or `aggregate <Root>`.
pub(crate) fn ddd_boundary_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddBoundaryDecl, MoxExtra<'src>> + Clone {
    let root = ddd_keyword(ddd::ROOT).to(DddBoundary::Root);
    let aggregate = ddd_keyword(ddd::AGGREGATE)
        .ignore_then(qname())
        .map(DddBoundary::Aggregate);
    root.or(aggregate).map_with(|kind, e| DddBoundaryDecl {
        kind,
        span: e.span(),
    })
}

/// One `invariant <raw_expr>;` clause of a design (DD5, issue pherolang#302):
/// the expression is captured raw (the `raw_expr` convention) and typed by
/// the driver against the designed class.
pub(crate) fn ddd_invariant_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, Span, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::INVARIANT).ignore_then(ddd_raw_expr())
}

pub(crate) fn ddd_design_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddDesign, MoxExtra<'src>> + Clone {
    ddd_design_head()
        .then(name())
        .then(ddd_flags())
        .then(ddd_boundary_decl().or_not())
        .then(ddd_boundary_decl().repeated().collect::<Vec<_>>())
        .then(ddd_invariant_decl().repeated().collect::<Vec<_>>())
        .then(ddd_dto_suffix().or_not())
        .then(ddd_repository_decl().or_not())
        .then(ddd_machine_decl().or_not())
        .map_with(|parts, e| {
            let (
                (
                    ((((((head, class), flags), boundary), extra_boundaries), invariants), suffix),
                    repository,
                ),
                machine,
            ) = parts;
            let (abstract_span, stereotype) = head;
            let mut problems = Vec::new();
            if !extra_boundaries.is_empty() {
                let span = boundary
                    .as_ref()
                    .map(|boundary| boundary.span)
                    .unwrap_or_else(|| extra_boundaries[0].span);
                problems.push((
                    span,
                    "E0379 a design may declare only one aggregate boundary; `root` and \
                         `aggregate <Root>` contradict each other"
                        .to_string(),
                ));
            }
            let mut dto_source = None;
            let mut dto_members = None;
            if let Some(DddDtoSuffix {
                source,
                members,
                from_written,
                span,
            }) = suffix
            {
                if stereotype == DddStereotype::Dto {
                    dto_source = source;
                    match members {
                        Some(members) => dto_members = Some(members),
                        None => {
                            problems.push((
                                span,
                                "expected a projection block `{ ... }` after \
                                     `from <source>`"
                                    .to_string(),
                            ));
                            // Recover as an empty projection so the
                            // source is not silently promoted to a
                            // standalone DTO.
                            dto_members = Some(DddDtoMembers::Projection(Vec::new()));
                        }
                    }
                } else if from_written {
                    problems.push((
                        span,
                        format!("`{} <source>` is only allowed on a `dto` design", ddd::FROM),
                    ));
                } else {
                    problems.push((
                        span,
                        "a DTO member block is only allowed on a `dto` design \
                             (`dto Name { ... }` or `dto Name from Source { ... }`)"
                            .to_string(),
                    ));
                }
            }
            (
                DddDesign {
                    is_abstract: abstract_span.is_some(),
                    stereotype,
                    class,
                    flags,
                    boundary,
                    invariants,
                    dto_source,
                    dto_members,
                    repository,
                    machine,
                    span: e.span(),
                },
                problems,
            )
        })
        .validate(|(design, problems), _, emitter| {
            for (span, message) in problems {
                emitter.emit(Rich::custom(span, message));
            }
            design
        })
}

/// The `where ( ... )` predicate clause of a repository query operation (DD9):
/// the contextual [`ddd::WHERE`] word followed by a [`raw_parens`] span the
/// driver slices and types (the `when`-guard convention). A query takes
/// `where`, never `when`; the caller reports the `when` mismatch by name.
pub(crate) fn ddd_query_where<'src>(
) -> impl Parser<'src, Tokens<'src>, Span, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::WHERE).ignore_then(raw_parens())
}

/// One `field ("asc" | "desc")?` key of a repository query's `sort` clause
/// (DD9). Both direction words are contextual; `asc` is recorded faithfully
/// and dropped by the formatter (the default), `desc` is re-emitted.
pub(crate) fn ddd_sort_key<'src>(
) -> impl Parser<'src, Tokens<'src>, DddSortKey, MoxExtra<'src>> + Clone {
    name()
        .then(
            ddd_keyword(ddd::ASC)
                .to(DddSortDirection::Asc)
                .or(ddd_keyword(ddd::DESC).to(DddSortDirection::Desc))
                .or_not(),
        )
        .map_with(|(field, direction), e| DddSortKey {
            field,
            direction,
            span: e.span(),
        })
}

/// The `sort <key> (, <key>)*` clause of a repository query operation (DD9):
/// the ordered keys of the read-only projection. A missing key list fails the
/// clause (there is no valid empty `sort`), which backtracks the operation
/// into the repository's recovery path.
pub(crate) fn ddd_query_sort<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<DddSortKey>, MoxExtra<'src>> + Clone {
    let key = ddd_sort_key();
    ddd_keyword(ddd::SORT).ignore_then(
        key.clone()
            .then(
                kw(Token::Comma)
                    .ignore_then(key)
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .map(|(first, rest)| once(first).chain(rest).collect::<Vec<_>>()),
    )
}

/// The `page (limit <int>)? (max <int>)? cursor?` clause of a repository
/// query operation (DD9): the query's page window. Every member is optional
/// (the consumer's profile supplies the defaults) and the members are
/// order-independent.
pub(crate) fn ddd_query_page<'src>(
) -> impl Parser<'src, Tokens<'src>, DddPage, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::PAGE)
        .ignore_then(ddd_keyword(ddd::LIMIT).ignore_then(int_lit()).or_not())
        .then(ddd_keyword(ddd::MAX).ignore_then(int_lit()).or_not())
        .then(ddd_keyword(ddd::CURSOR).or_not())
        .map_with(|((limit, max), cursor), e| DddPage {
            limit,
            max,
            cursor: cursor.is_some(),
            span: e.span(),
        })
}

/// The query-clause sequence of a repository query operation (DD9), in the
/// canonical order `where sort? page?`: the `where` predicate is **required**
/// (a repository operation without one is not a query), and the `sort` and
/// `page` clauses are optional. The `capability` clause belongs to the
/// operation, not the query, so it is parsed by the caller.
pub(crate) fn ddd_query_clauses<'src>(
) -> impl Parser<'src, Tokens<'src>, DddQuery, MoxExtra<'src>> + Clone {
    ddd_query_where()
        .then(ddd_query_sort().or_not())
        .then(ddd_query_page().or_not())
        .map_with(|((predicate, sort), page), e| DddQuery {
            predicate,
            sort: sort.unwrap_or_default(),
            page,
            span: e.span(),
        })
}

/// One operation of a `repository` body: an optional `protected` modifier,
/// then a built-in (`findById`, `findAll`, `findByExample`, `findByKeys`,
/// `save`, `delete` — recognized by their contextual keywords, no signature
/// of their own), a declared operation with an explicit signature, or a
/// declared **query** operation (DD9, typed repository queries):
/// `signature throws? where sort? page? capability? ;`. All forms end in `;`.
/// A non-query repository operation takes no `capability` clause: the wire
/// artifact's repository operations carry none, so accepting one would
/// silently drop data (a signature followed by only a `capability` clause is
/// therefore still the generic "expected a repository operation" error).
pub(crate) fn ddd_repository_op<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddRepositoryOp>, MoxExtra<'src>> + Clone {
    let builtin_keyword = select! {
        // chumsky's `select!` takes a boolean guard expression, so the
        // lookup runs twice; the `unwrap` is the guard's `Some`.
        Token::Ident(text) = e if ddd::builtin_from_keyword(text).is_some() => {
            (e.span(), ddd::builtin_from_keyword(text).unwrap())
        }
    };
    // A builtin has no signature to attach a body to (issue pherolang#178):
    // `save { ... }` is refused explicitly rather than falling into the
    // generic member junk. The balanced body is consumed so recovery lands
    // on the next member.
    let builtin_body_refusal = builtin_keyword.then(raw_body()).validate(
        |((keyword_span, builtin), body_span), _e, emitter| {
            emitter.emit(Rich::custom(
                (keyword_span.start..body_span.end).into(),
                format!(
                    "built-in repository operation '{}' takes no body; declare a \
                     signature (`<Type> name(params) {{ ... }}`) to give it one",
                    builtin.keyword()
                ),
            ));
        },
    );
    let builtin = builtin_keyword.then_ignore(kw(Token::Other(';'))).map_with(
        |(keyword_span, builtin), e| DddRepositoryOp {
            name: Name {
                text: builtin.keyword().to_string(),
                span: keyword_span,
                escaped: false,
            },
            builtin: Some(builtin),
            return_type: None,
            multiplicity: None,
            params: Vec::new(),
            is_protected: false,
            throws: Vec::new(),
            condition: None,
            body: None,
            query: None,
            capabilities: Vec::new(),
            span: e.span(),
        },
    );
    // A built-in takes no `throws` clause (DD7): its failures are structural,
    // not declared. Consume the clause so recovery lands on the next member
    // and name the refusal.
    let builtin_throws_refusal = builtin_keyword
        .then(ddd_throws())
        .then_ignore(kw(Token::Other(';')).or_not())
        .validate(|((keyword_span, builtin), throws_span), _e, emitter| {
            let end = throws_span
                .last()
                .map(|name| name.span.end)
                .unwrap_or(keyword_span.end);
            emitter.emit(Rich::custom(
                (keyword_span.start..end).into(),
                format!(
                    "built-in repository operation '{}' takes no `throws` clause; its \
                     failures are structural, not declared (declare a signature to \
                     declare errors)",
                    builtin.keyword()
                ),
            ));
        });
    // The body form (issue pherolang#178): `Type? name(params) when (...)? {
    // statements }`. Declared operations only — a builtin has no signature.
    let body = ddd_body_signature()
        .then(ddd_throws().or_not())
        .then(ddd_guard_clause().or_not())
        .then(ddd_body(DddBodyKind::Repository))
        .map_with(|(((signature, throws), condition), body), e| {
            let (return_type, multiplicity, name, params) = signature;
            DddRepositoryOp {
                name,
                builtin: None,
                return_type,
                multiplicity,
                params,
                is_protected: false,
                throws: throws.unwrap_or_default(),
                condition,
                body: Some(body),
                query: None,
                capabilities: Vec::new(),
                span: e.span(),
            }
        });
    // The query form (DD9, typed repository queries): `signature throws?
    // where sort? page? capability? ;`. The `where` clause is what makes an
    // operation a query — a signature followed by only a `capability` clause
    // is not one and keeps the generic rejection below. `throws` stays
    // grammatically legal before the query clauses (the driver rejects it
    // later). A `when` guard after the query clauses is consumed (in either
    // order with the `capability` clause) so the mismatch is named rather
    // than falling into the generic recovery.
    let trailing = ddd_guard_clause()
        .map(|span| (None, Some(span)))
        .or(ddd_capabilities().map(|capabilities| (Some(capabilities), None)))
        .repeated()
        .collect::<Vec<_>>();
    let query = ddd_signature()
        .then(ddd_throws().or_not())
        .then(ddd_query_clauses())
        .then(trailing)
        .then_ignore(kw(Token::Other(';')))
        .map_with(
            |((((type_ref, multiplicity, name, params), throws), query), trailing), e| {
                let mut capabilities: Vec<Name> = Vec::new();
                let mut guard: Option<Span> = None;
                for (clause_capabilities, clause_guard) in trailing {
                    if let Some(clause_capabilities) = clause_capabilities {
                        capabilities.extend(clause_capabilities);
                    }
                    if guard.is_none() {
                        guard = clause_guard;
                    }
                }
                (
                    DddRepositoryOp {
                        name,
                        builtin: None,
                        return_type: Some(type_ref),
                        multiplicity,
                        params,
                        is_protected: false,
                        throws: throws.unwrap_or_default(),
                        condition: None,
                        body: None,
                        query: Some(query),
                        capabilities,
                        span: e.span(),
                    },
                    guard,
                )
            },
        )
        .validate(|(op, guard), _e, emitter| {
            if let Some(guard) = guard {
                emitter.emit(Rich::custom(
                    guard,
                    "a repository query operation takes a `where` predicate, not a `when` \
                     guard (write `where (<predicate>)` instead)",
                ));
            }
            op
        });
    let declared = ddd_signature()
        .then(ddd_throws().or_not())
        .then(ddd_guard_clause().or_not())
        .then_ignore(kw(Token::Other(';')))
        .map_with(
            |(((type_ref, multiplicity, name, params), throws), condition), e| DddRepositoryOp {
                name,
                builtin: None,
                return_type: Some(type_ref),
                multiplicity,
                params,
                is_protected: false,
                throws: throws.unwrap_or_default(),
                condition,
                body: None,
                query: None,
                capabilities: Vec::new(),
                span: e.span(),
            },
        );
    ddd_keyword(ddd::PROTECTED)
        .or_not()
        .then(
            builtin_body_refusal
                .to(None)
                .or(builtin_throws_refusal.to(None))
                .or(builtin.map(Some))
                .or(body.map(Some))
                .or(query.map(Some))
                .or(declared.map(Some))
                .or(junk_ddd_repository_op().to(None)),
        )
        .map(|(protected, op)| {
            op.map(|mut op| {
                op.is_protected = protected.is_some();
                op
            })
        })
}

/// Consumes a run of tokens that cannot start or continue a repository
/// operation, stopping before the body's closing `}` and before identifiers.
pub(crate) fn junk_ddd_repository_op<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a repository operation"));
        })
}

/// A `repository <name> { ... }` block of a design declaration.
pub(crate) fn ddd_repository_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddRepository, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::REPOSITORY)
        .ignore_then(name())
        .then_ignore(kw(Token::LBrace))
        .then(ddd_repository_op().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|(name, operations), e| DddRepository {
            name,
            operations: operations.into_iter().flatten().collect(),
            span: e.span(),
        })
}

/// The `when ( ... )` guard clause shared by machine transitions and service
/// operations: the real `when` keyword followed by a [`raw_parens`] span the
/// driver slices and types (the `.actor` condition convention).
pub(crate) fn ddd_guard_clause<'src>(
) -> impl Parser<'src, Tokens<'src>, Span, MoxExtra<'src>> + Clone {
    kw(Token::When).ignore_then(raw_parens())
}

/// One `from -> to (via <Trigger>)? (when (...))?;` transition of a machine
/// body. The arrow is the real [`Token::Arrow`]; a transition takes exactly
/// one target — a `,` after the target cannot start any machine member, so a
/// multi-target arrow fails into the member junk path (the deliberate
/// exclusion noted in the grammar).
pub(crate) fn ddd_machine_transition<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddMachineTransition>, MoxExtra<'src>> + Clone {
    let transition = name()
        .then_ignore(kw(Token::Arrow))
        .then(name())
        .then(ddd_keyword(ddd::VIA).ignore_then(name()).or_not())
        .then(ddd_guard_clause().or_not())
        .then_ignore(kw(Token::Other(';')))
        .map_with(|(((from, to), trigger), guard), e| DddMachineTransition {
            from,
            to,
            trigger,
            guard,
            span: e.span(),
        });
    transition.map(Some).or(junk_ddd_machine_member().to(None))
}

/// Consumes a run of tokens that cannot start or continue a machine member,
/// stopping before the body's closing `}` and before identifiers (which
/// could begin the next member). Always consumes at least one token.
pub(crate) fn junk_ddd_machine_member<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a machine member"));
        })
}

/// A `machine <field>: <Enum> { ... }` block of a design declaration. Body
/// members are `initial`/`terminal` lines and transitions; the per-kind
/// lists fold exactly like the search members.
pub(crate) fn ddd_machine_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddMachine, MoxExtra<'src>> + Clone {
    let initial = ddd_keyword(ddd::INITIAL)
        .ignore_then(name())
        .then_ignore(kw(Token::Other(';')));
    let terminal = ddd_keyword(ddd::TERMINAL)
        .ignore_then(name())
        .then_ignore(kw(Token::Other(';')));
    let member = choice((
        initial.map(DddMachineMember::Initial),
        terminal.map(DddMachineMember::Terminal),
        ddd_machine_transition().map(DddMachineMember::Transition),
    ));
    ddd_keyword(ddd::MACHINE)
        .ignore_then(name())
        .then_ignore(kw(Token::Other(':')))
        .then(tref())
        .then_ignore(kw(Token::LBrace))
        .then(member.repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|((field, type_), members), e| {
            let mut machine = DddMachine {
                field,
                type_,
                initial: None,
                terminals: Vec::new(),
                transitions: Vec::new(),
                span: e.span(),
            };
            for member in members {
                match member {
                    DddMachineMember::Initial(name) => machine.initial = Some(name),
                    DddMachineMember::Terminal(name) => machine.terminals.push(name),
                    DddMachineMember::Transition(Some(transition)) => {
                        machine.transitions.push(transition)
                    }
                    DddMachineMember::Transition(None) => {}
                }
            }
            machine
        })
}

/// One member of a `machine` body.
#[derive(Clone)]
pub(crate) enum DddMachineMember {
    Initial(Name),
    Terminal(Name),
    Transition(Option<DddMachineTransition>),
}

/// Scans a raw expression: tokens up to the terminating `;`, tracking the
/// nesting depth of `()`, `[]` and `{}` so an `if ... { ... } else { ... }`
/// expression's braces stay inside the capture while the terminator must sit
/// at depth 0. String literals are single tokens, so a `;` inside one never
/// terminates. The returned span slices the expression source strictly
/// between the `=` and the `;` (the `;` excluded), the `.mox` op-body
/// convention; at least one token is required, so a bare `name = ;` fails.
pub(crate) fn ddd_raw_expr<'src>() -> impl Parser<'src, Tokens<'src>, Span, MoxExtra<'src>> + Clone
{
    let balanced = recursive(|body| {
        let atom = select! {
            t if !matches!(
                t,
                Token::LParen
                    | Token::RParen
                    | Token::LBracket
                    | Token::RBracket
                    | Token::LBrace
                    | Token::RBrace
                    | Token::Other(';')
            ) =>
                ()
        };
        // A call's argument list may be empty (`sum()`, `size()`), so the
        // balanced interior is optional in every bracket family.
        let parens = kw(Token::LParen)
            .ignore_then(body.clone().or_not().ignored())
            .then_ignore(kw(Token::RParen));
        let brackets = kw(Token::LBracket)
            .ignore_then(body.clone().or_not().ignored())
            .then_ignore(kw(Token::RBracket));
        let braces = kw(Token::LBrace)
            .ignore_then(body.or_not().ignored())
            .then_ignore(kw(Token::RBrace));
        atom.or(parens)
            .or(brackets)
            .or(braces)
            .repeated()
            .at_least(1)
    });
    balanced
        .map_with(|(), e| e.span())
        .then_ignore(kw(Token::Other(';')))
}

/// One `name = <expr>;` entry of a `document { ... }` clause. The entry is
/// the shared [`DddProjectionMember::Assign`] AST (issue pherolang#300): the
/// expression is captured raw — its span slices the expression source
/// strictly between the `=` and the terminating `;` — exactly like the
/// `.mox` op bodies; parsing it is the driver's job. Search syntax is the
/// assignment form only; the `name;` selection shorthand is a DTO
/// projection form, refused here by [`junk_ddd_document_entry`].
pub(crate) fn ddd_document_entry<'src>(
) -> impl Parser<'src, Tokens<'src>, DddProjectionMember, MoxExtra<'src>> + Clone {
    name()
        .then_ignore(kw(Token::Eq))
        .then(ddd_raw_expr())
        .map_with(|(name, expr), e| DddProjectionMember::Assign {
            name,
            expr,
            span: e.span(),
        })
}

/// Consumes one token that cannot start or continue a `document` entry,
/// stopping before the clause's closing `}` and before identifiers. The
/// refusal names the document-entry shape, so the projection-only `name;`
/// shorthand is reported rather than silently dropped.
pub(crate) fn junk_ddd_document_entry<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected a document entry (`name = <expression>;`); the `name;` \
                 selection shorthand belongs in a DTO projection",
            ));
        })
}

/// One `text { ... }` field: `<property> (boost <int>)? (analyzer "<...>")?`.
pub(crate) fn ddd_search_field<'src>(
) -> impl Parser<'src, Tokens<'src>, DddSearchField, MoxExtra<'src>> + Clone {
    qname()
        .then(ddd_keyword(ddd::BOOST).ignore_then(int_lit()).or_not())
        .then(
            ddd_keyword(ddd::ANALYZER)
                .ignore_then(string_lit())
                .or_not(),
        )
        .map_with(|((property, boost), analyzer), e| DddSearchField {
            property,
            boost,
            analyzer,
            span: e.span(),
        })
}

/// The `ranking (bm25 | tfIdf | exact | custom "<...>")` line.
pub(crate) fn ddd_ranking<'src>(
) -> impl Parser<'src, Tokens<'src>, DddRanking, MoxExtra<'src>> + Clone {
    ddd_keyword("ranking").ignore_then(choice((
        ddd_keyword("bm25").to(DddRanking::Bm25),
        ddd_keyword("tfIdf").to(DddRanking::TfIdf),
        ddd_keyword("exact").to(DddRanking::Exact),
        ddd_keyword("custom")
            .ignore_then(string_lit())
            .map(DddRanking::Custom),
    )))
}

/// The `pagination { (limit <int>)? (max <int>)? (cursor)? }` block. An
/// empty block is legal at the syntax layer (the driver judges it).
pub(crate) fn ddd_pagination<'src>(
) -> impl Parser<'src, Tokens<'src>, DddPagination, MoxExtra<'src>> + Clone {
    let member = choice((
        ddd_keyword(ddd::LIMIT)
            .ignore_then(int_lit())
            .map(|value| (Some(value), None, false)),
        ddd_keyword(ddd::MAX)
            .ignore_then(int_lit())
            .map(|value| (None, Some(value), false)),
        ddd_keyword(ddd::CURSOR).to((None, None, true)),
    ));
    ddd_keyword("pagination")
        .ignore_then(kw(Token::LBrace))
        .ignore_then(member.repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|members, e| DddPagination {
            limit: members.iter().find_map(|(limit, _, _)| *limit),
            max: members.iter().find_map(|(_, max, _)| *max),
            cursor: members.iter().any(|(_, _, cursor)| *cursor),
            span: e.span(),
        })
}

/// One member of a `search` body.
#[derive(Clone)]
pub(crate) enum DddSearchMember {
    Entity(QualifiedName),
    Text(DddSearchField),
    Filter(QualifiedName),
    Sort(QualifiedName),
    Document(DddProjectionMember),
    DocumentRef(QualifiedName),
    Ranking(DddRanking),
    Analyzer(String),
    Pagination(DddPagination),
    Capability(Vec<Name>),
}

/// Consumes a run of tokens that cannot start or continue a search member,
/// stopping before the body's closing `}` and before identifiers (which
/// could begin the next member). Always consumes at least one token.
pub(crate) fn junk_ddd_search_member<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a search member"));
        })
}

/// The leading keyword of a [`ddd::SEARCH_MEMBERS`] row. Only the
/// `capability` clause — led by the real `capability` token, not an
/// identifier — has no keyword, and its arm never calls this.
pub(crate) fn member_keyword(keyword: Option<&'static str>) -> &'static str {
    keyword.expect("only the `capability` search member lacks a keyword")
}

/// One member of a `search` body with junk recovery. The alternatives come
/// from [`ddd::SEARCH_MEMBERS`] in the table's order — the same order the
/// formatter canonicalizes a search body to — so the parse order and the
/// canonical order cannot drift apart. Repeated `text`, `filters`, `sort`,
/// `document`, and `capability` clauses merge into their lists; `entity`,
/// `ranking`, `analyzer`, and `pagination` are single — a repeat is
/// idempotent, the first declaration wins (the design-flags precedent). The
/// `capability` clause is the same [`ddd_capabilities()`] grammar a service
/// operation takes (pherolang#145): one clause grammar for both surfaces.
pub(crate) fn ddd_search_member<'src>(
) -> impl Parser<'src, Tokens<'src>, Vec<DddSearchMember>, MoxExtra<'src>> + Clone {
    let alternatives: Vec<Boxed<'src, 'src, Tokens<'src>, Vec<DddSearchMember>, MoxExtra<'src>>> =
        ddd::SEARCH_MEMBERS
            .iter()
            .map(|(kind, keyword)| match kind {
                ddd::SearchMemberKind::Entity => ddd_keyword(member_keyword(*keyword))
                    .ignore_then(qname())
                    .map(|qname| vec![DddSearchMember::Entity(qname)])
                    .boxed(),
                ddd::SearchMemberKind::Text => ddd_keyword(member_keyword(*keyword))
                    .ignore_then(kw(Token::LBrace))
                    .ignore_then(
                        ddd_search_field()
                            .map(Some)
                            .or(junk_ddd_search_member().to(None))
                            .repeated()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(kw(Token::RBrace))
                    .map(|fields| {
                        fields
                            .into_iter()
                            .flatten()
                            .map(DddSearchMember::Text)
                            .collect()
                    })
                    .boxed(),
                ddd::SearchMemberKind::Filters => {
                    ddd_search_names(member_keyword(*keyword), DddSearchMember::Filter).boxed()
                }
                ddd::SearchMemberKind::Sort => {
                    ddd_search_names(member_keyword(*keyword), DddSearchMember::Sort).boxed()
                }
                ddd::SearchMemberKind::Document => {
                    let inline = kw(Token::LBrace)
                        .ignore_then(
                            ddd_document_entry()
                                .map(Some)
                                .or(junk_ddd_document_entry().to(None))
                                .repeated()
                                .collect::<Vec<_>>(),
                        )
                        .then_ignore(kw(Token::RBrace))
                        .map(|entries| {
                            entries
                                .into_iter()
                                .flatten()
                                .map(DddSearchMember::Document)
                                .collect::<Vec<_>>()
                        });
                    let named = qname()
                        .then_ignore(kw(Token::Other(';')))
                        .map(|name| vec![DddSearchMember::DocumentRef(name)]);
                    ddd_keyword(member_keyword(*keyword))
                        .ignore_then(inline.or(named))
                        .boxed()
                }
                ddd::SearchMemberKind::Ranking => ddd_ranking()
                    .map(|ranking| vec![DddSearchMember::Ranking(ranking)])
                    .boxed(),
                ddd::SearchMemberKind::Analyzer => ddd_keyword(member_keyword(*keyword))
                    .ignore_then(string_lit())
                    .map(|analyzer| vec![DddSearchMember::Analyzer(analyzer)])
                    .boxed(),
                ddd::SearchMemberKind::Pagination => ddd_pagination()
                    .map(|pagination| vec![DddSearchMember::Pagination(pagination)])
                    .boxed(),
                ddd::SearchMemberKind::Capability => ddd_capabilities()
                    .map(|caps| vec![DddSearchMember::Capability(caps)])
                    .boxed(),
            })
            .collect();
    choice(alternatives).or(junk_ddd_search_member().to(Vec::new()))
}

/// The `filters { ... }` / `sort { ... }` bodies: newline-separated
/// qualified names, one per line, with junk recovery between them.
pub(crate) fn ddd_search_names<'src>(
    keyword: &'static str,
    wrap: fn(QualifiedName) -> DddSearchMember,
) -> impl Parser<'src, Tokens<'src>, Vec<DddSearchMember>, MoxExtra<'src>> + Clone {
    ddd_keyword(keyword)
        .ignore_then(kw(Token::LBrace))
        .ignore_then(
            qname()
                .map(Some)
                .or(junk_ddd_search_member().to(None))
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(kw(Token::RBrace))
        .map(move |names| names.into_iter().flatten().map(wrap).collect())
}

/// A `search <name> { ... }` projection of a `module`. Doc comments (`///`
/// runs) directly above it become the search description after parsing.
pub(crate) fn ddd_search_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddSearch, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::SEARCH)
        .ignore_then(name())
        .then_ignore(kw(Token::LBrace))
        .then(ddd_search_member().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|(name, members), e| {
            let mut search = DddSearch {
                name,
                doc: None,
                entity: None,
                text: Vec::new(),
                filters: Vec::new(),
                sort: Vec::new(),
                document: Vec::new(),
                document_ref: None,
                ranking: None,
                analyzer: None,
                pagination: None,
                capabilities: Vec::new(),
                span: e.span(),
            };
            for member in members.into_iter().flatten() {
                match member {
                    DddSearchMember::Entity(entity) => {
                        if search.entity.is_none() {
                            search.entity = Some(entity);
                        }
                    }
                    DddSearchMember::Text(field) => search.text.push(field),
                    DddSearchMember::Filter(name) => search.filters.push(name),
                    DddSearchMember::Sort(name) => search.sort.push(name),
                    DddSearchMember::Document(entry) => search.document.push(entry),
                    DddSearchMember::DocumentRef(name) => {
                        if search.document_ref.is_none() {
                            search.document_ref = Some(name);
                        }
                    }
                    DddSearchMember::Ranking(ranking) => {
                        if search.ranking.is_none() {
                            search.ranking = Some(ranking);
                        }
                    }
                    DddSearchMember::Analyzer(analyzer) => {
                        if search.analyzer.is_none() {
                            search.analyzer = Some(analyzer);
                        }
                    }
                    DddSearchMember::Pagination(pagination) => {
                        if search.pagination.is_none() {
                            search.pagination = Some(pagination);
                        }
                    }
                    DddSearchMember::Capability(names) => search.capabilities.extend(names),
                }
            }
            search
        })
}

/// One child operation of a `children` block (issue pherolang#179; the block
/// was named `extend` before DD4, issue pherolang#301):
/// `("protected")? signature "=>" child_feature "." child_action
/// guard_clause? ";"`. The signature is the shared [`ddd_signature`]
/// production; the dotted pair is exactly two names — the root's child
/// feature and the action word. `protected` is the optional modifier the
/// service/repository operations take. The action's closed vocabulary
/// (`add`, `remove`, `save`) and the feature's existence are driver
/// concerns.
pub(crate) fn ddd_child_op<'src>(
) -> impl Parser<'src, Tokens<'src>, DddChildOp, MoxExtra<'src>> + Clone {
    let target = name()
        .then(kw(Token::Dot).ignore_then(name()).or_not())
        .map_with(|(feature, action), e| (feature, action, e.span()))
        .validate(|(feature, action, span), _e, emitter| match action {
            Some(action) => (feature, action),
            None => {
                emitter.emit(Rich::custom(span, "expected `feature.action`"));
                let action = Name {
                    text: String::new(),
                    span,
                    escaped: false,
                };
                (feature, action)
            }
        });
    ddd_keyword(ddd::PROTECTED)
        .or_not()
        .then(ddd_signature())
        .then_ignore(kw(Token::FatArrow))
        .then(target)
        .then(ddd_guard_clause().or_not())
        .then_ignore(kw(Token::Other(';')))
        .map_with(
            |(((protected, signature), (feature, action)), condition), e| {
                let (type_ref, multiplicity, name, params) = signature;
                DddChildOp {
                    name,
                    return_type: type_ref,
                    multiplicity,
                    params,
                    feature,
                    action,
                    is_protected: protected.is_some(),
                    condition,
                    span: e.span(),
                }
            },
        )
}

/// Consumes a run of tokens that cannot start or continue a child
/// operation, stopping before the body's closing `}` and before
/// identifiers (which could begin the next operation). Always consumes at
/// least one token.
pub(crate) fn junk_ddd_child_op<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a child operation"));
        })
}

/// A module-level `children <Class> { ... }` block of child operations
/// (issue pherolang#179; renamed by DD4, issue pherolang#301). The body is a
/// run of [`ddd_child_op`]s (with junk recovery between them); the driver
/// validates the target (an entity design with a repository in the same
/// module) and each operation. The 0.4 `extend` alias was removed from the
/// grammar in 0.5 (DF4, issue pherolang#327): `pherolang fmt` is the
/// mechanical migration path.
pub(crate) fn ddd_extend_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddExtend, MoxExtra<'src>> + Clone {
    select! {
        Token::Ident(text) = e if text == ddd::CHILDREN => e.span()
    }
    .then(name())
    .then_ignore(kw(Token::LBrace))
    .then(
        ddd_child_op()
            .map(Some)
            .or(junk_ddd_child_op().to(None))
            .repeated()
            .collect::<Vec<_>>(),
    )
    .then_ignore(kw(Token::RBrace))
    .map_with(|((_, class), operations), e| DddExtend {
        class,
        operations: operations.into_iter().flatten().collect(),
        span: e.span(),
    })
}

/// One member of a `module` body.
#[derive(Clone)]
pub(crate) enum DddModuleMember {
    Error(DddErrorDecl),
    Service(DddService),
    Design(Box<DddDesign>),
    Search(Box<DddSearch>),
    Extend(DddExtend),
}

/// Consumes a run of tokens that cannot start or continue a module member,
/// stopping before the body's closing `}` and before identifiers (which
/// could begin the next `service` or design).
pub(crate) fn junk_ddd_module_member<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(e.span(), "expected a module member"));
        })
}

/// One member of a `module` body with junk recovery.
pub(crate) fn ddd_module_member<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddModuleMember>, MoxExtra<'src>> + Clone {
    choice((
        ddd_error_decl().map(DddModuleMember::Error),
        ddd_service_decl().map(DddModuleMember::Service),
        ddd_design_decl().map(|design| DddModuleMember::Design(Box::new(design))),
        ddd_search_decl().map(|search| DddModuleMember::Search(Box::new(search))),
        ddd_extend_decl().map(DddModuleMember::Extend),
    ))
    .map(Some)
    .or(junk_ddd_module_member().to(None))
}

/// A `module <name> { ... }` declaration of the application.
pub(crate) fn ddd_module_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddModule, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::MODULE)
        .ignore_then(name())
        .then_ignore(kw(Token::LBrace))
        .then(ddd_module_member().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|(name, members), e| {
            let mut errors = Vec::new();
            let mut services = Vec::new();
            let mut designs = Vec::new();
            let mut searches = Vec::new();
            let mut extends = Vec::new();
            let mut problems = Vec::new();
            let mut saw_member = false;
            for member in members {
                match member {
                    Some(DddModuleMember::Error(decl)) if !saw_member => errors.push(decl),
                    Some(DddModuleMember::Error(decl)) => problems.push((
                        decl.span,
                        "`error` declarations must precede the module's members".to_string(),
                    )),
                    Some(DddModuleMember::Service(service)) => {
                        saw_member = true;
                        services.push(service);
                    }
                    Some(DddModuleMember::Design(design)) => {
                        saw_member = true;
                        designs.push(*design);
                    }
                    Some(DddModuleMember::Search(search)) => {
                        saw_member = true;
                        searches.push(*search);
                    }
                    Some(DddModuleMember::Extend(extend)) => {
                        saw_member = true;
                        extends.push(extend);
                    }
                    None => {}
                }
            }
            (
                DddModule {
                    name,
                    errors,
                    services,
                    designs,
                    searches,
                    extends,
                    span: e.span(),
                },
                problems,
            )
        })
        .validate(|(module, problems), _, emitter| {
            for (span, message) in problems {
                emitter.emit(Rich::custom(span, message));
            }
            module
        })
}

/// The `base <qualified-name>` member of the application body.
pub(crate) fn ddd_base_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddBase, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::BASE)
        .ignore_then(qname())
        .map_with(|package, e| DddBase {
            package,
            span: e.span(),
        })
}

/// One member of an `application` body.
#[derive(Clone)]
pub(crate) enum DddApplicationItem {
    Base(DddBase),
    Error(DddErrorDecl),
    Module(DddModule),
}

/// Consumes a run of tokens that cannot start or continue an application
/// member, stopping before the body's closing `}` and before identifiers.
pub(crate) fn junk_ddd_application_item<'src>(
) -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let first = select! { t if !matches!(t, Token::RBrace) => () };
    let rest = select! {
        t if !matches!(t, Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)) => ()
    };
    first
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected `base`, `error`, or `module`",
            ));
        })
}

/// One member of an `application` body with junk recovery.
pub(crate) fn ddd_application_item<'src>(
) -> impl Parser<'src, Tokens<'src>, Option<DddApplicationItem>, MoxExtra<'src>> + Clone {
    choice((
        ddd_base_decl().map(DddApplicationItem::Base),
        ddd_error_decl().map(DddApplicationItem::Error),
        ddd_module_decl().map(DddApplicationItem::Module),
    ))
    .map(Some)
    .or(junk_ddd_application_item().to(None))
}

/// The `application <name> { ... }` declaration: at most one `base` (which
/// the grammar pins to the first member position), the application-scoped
/// `error` declarations, then the modules.
pub(crate) fn ddd_application_decl<'src>(
) -> impl Parser<'src, Tokens<'src>, DddApplication, MoxExtra<'src>> + Clone {
    ddd_keyword(ddd::APPLICATION)
        .ignore_then(name())
        .then_ignore(kw(Token::LBrace))
        .then(ddd_application_item().repeated().collect::<Vec<_>>())
        .then_ignore(kw(Token::RBrace))
        .map_with(|(name, items), e| {
            let mut base = None;
            let mut errors = Vec::new();
            let mut modules = Vec::new();
            let mut problems = Vec::new();
            for item in items {
                match item {
                    Some(DddApplicationItem::Base(decl))
                        if base.is_none() && modules.is_empty() && errors.is_empty() =>
                    {
                        base = Some(decl);
                    }
                    Some(DddApplicationItem::Base(decl)) if base.is_some() => {
                        problems.push((decl.span, "duplicate `base` declaration".to_string()))
                    }
                    Some(DddApplicationItem::Base(decl)) => problems.push((
                        decl.span,
                        "`base` must be the first member of the application".to_string(),
                    )),
                    Some(DddApplicationItem::Error(decl)) if modules.is_empty() => {
                        errors.push(decl);
                    }
                    Some(DddApplicationItem::Error(decl)) => problems.push((
                        decl.span,
                        "`error` declarations must precede the application's modules".to_string(),
                    )),
                    Some(DddApplicationItem::Module(module)) => modules.push(module),
                    None => {}
                }
            }
            (
                DddApplication {
                    name,
                    base,
                    errors,
                    modules,
                    span: e.span(),
                },
                problems,
            )
        })
        .validate(|(decl, problems), _, emitter| {
            for (span, message) in problems {
                emitter.emit(Rich::custom(span, message));
            }
            decl
        })
}

/// One top-level item of a `.ddd` file.
#[derive(Clone)]
pub(crate) enum DddFileItem {
    Import(Import),
    Application(DddApplication),
    Module(DddModule),
    Junk,
}

/// Consumes a run of tokens up to the next `import` keyword or the
/// contextual `application` keyword (or end of input). File-level recovery:
/// always consumes at least one token (guaranteeing progress) and emits an
/// error for the skipped region.
pub(crate) fn junk_ddd_file<'src>() -> impl Parser<'src, Tokens<'src>, (), MoxExtra<'src>> + Clone {
    let rest = select! {
        t if !matches!(
            t,
            Token::Import | Token::Ident(ddd::APPLICATION | ddd::MODULE)
        ) => ()
    };
    any()
        .ignore_then(rest.repeated().ignored())
        .validate(|(), e, emitter| {
            emitter.emit(Rich::custom(
                e.span(),
                "expected an import, `application`, or `module` declaration",
            ));
        })
}

/// Folds the top-level items of a `.ddd` file: imports must precede the
/// application or top-level modules; a file is either one **root**
/// (`application`) or a **fragment** (top-level modules), never both.
/// Violations are reported as (span, message) problems; the AST recovers the
/// first application, keeps the fragment modules, and drops the offending
/// items regardless.
pub(crate) fn fold_ddd_file(items: Vec<DddFileItem>) -> (DddFile, Vec<(Span, String)>) {
    let mut imports = Vec::new();
    let mut application = None;
    let mut fragments = Vec::new();
    let mut problems = Vec::new();
    for item in items {
        match item {
            DddFileItem::Import(decl) => {
                if application.is_none() && fragments.is_empty() {
                    imports.push(decl);
                } else {
                    problems.push((
                        decl.span,
                        "`import` after the `application`/`module` declarations".to_string(),
                    ));
                }
            }
            DddFileItem::Application(decl) => {
                if application.is_none() {
                    if !fragments.is_empty() {
                        problems.push((
                            decl.span,
                            "a file declares either one `application` (a root) or top-level \
                             `module` blocks (a fragment), not both"
                                .to_string(),
                        ));
                    }
                    application = Some(decl);
                } else {
                    problems.push((decl.span, "duplicate `application` declaration".to_string()));
                }
            }
            DddFileItem::Module(module) => {
                if application.is_some() {
                    problems.push((
                        module.span,
                        "module declarations belong inside the `application` block; a \
                         top-level module makes the file a fragment"
                            .to_string(),
                    ));
                } else {
                    fragments.push(module);
                }
            }
            DddFileItem::Junk => {}
        }
    }
    (
        DddFile {
            imports,
            application,
            fragments,
        },
        problems,
    )
}

pub(crate) fn ddd_file<'src>() -> impl Parser<'src, Tokens<'src>, DddFile, MoxExtra<'src>> + Clone {
    let item = choice((
        ddd_import_decl().map(DddFileItem::Import),
        ddd_application_decl().map(DddFileItem::Application),
        ddd_module_decl().map(DddFileItem::Module),
    ))
    .or(junk_ddd_file().to(DddFileItem::Junk));

    item.repeated()
        .collect::<Vec<_>>()
        .then_ignore(end())
        .map_with(|items, e| (fold_ddd_file(items), e.span()))
        .validate(|((file, problems), span), _, emitter| {
            for (span, message) in problems {
                emitter.emit(Rich::custom(span, message));
            }
            if file.application.is_none() && file.fragments.is_empty() {
                emitter.emit(Rich::custom(
                    Span::from(span.end..span.end),
                    "expected an `application` declaration (a root) or at least one \
                     top-level `module` (a fragment)",
                ));
            }
            file
        })
}

/// The outcome of parsing a `.ddd` source: as much of the file as could be
/// recovered, plus all encountered errors. Parsing never panics and always
/// produces a result.
#[derive(Debug, Clone, PartialEq)]
pub struct DddParseResult {
    /// The recovered design file, or `None` if no output could be produced
    /// at all.
    pub ast: Option<DddFile>,
    /// All errors encountered during lexing and parsing.
    pub errors: Vec<ParseError>,
}

/// Lex and parse a `.ddd` source text: `import` declarations followed by
/// exactly one `application` declaration of modules, application services,
/// and class designs.
///
/// This function never panics and always recovers as much of the AST as
/// possible; check [`DddParseResult::errors`] for syntax problems. Doc
/// comment runs (`///`) directly above a `service` declaration become its
/// description. A file without an `application` declaration is a syntax
/// error (recovered as `application: None`).
pub fn parse_ddd(source: &str) -> DddParseResult {
    let (tokens, comments) = match lex_with_comments(source) {
        Ok(result) => result,
        Err(error) => {
            return DddParseResult {
                ast: None,
                errors: vec![ParseError {
                    message: error.to_string(),
                    span: error.span,
                }],
            };
        }
    };
    let (mut ast, errors) = ddd_file().parse(Tokens::new(&tokens)).into_output_errors();
    if let Some(file) = ast.as_mut() {
        attach_ddd_docs(file, &comments, source);
    }
    DddParseResult {
        ast,
        errors: errors
            .into_iter()
            .map(|error| ParseError {
                message: error.to_string(),
                span: *error.span(),
            })
            .collect(),
    }
}

// --- .deploy files ------------------------------------------------------------
