//! The `.ddd` formatter: application/module/service/design/repository/search scanners, search-member reordering, and the `import` hoist.

use super::*;
use crate::ddd;

impl<'src> Formatter<'src> {
    /// Like [`Formatter::run_actors`], but for `.ddd` files: the only
    /// recognized top-level constructs are `import` declarations and the
    /// `application` block, with imports hoisted ahead of it.
    pub(crate) fn run_ddd(&mut self) -> String {
        self.hoist_ddd_imports();
        loop {
            let front = self.front().cloned();
            match front {
                None => break,
                Some(Node::Comment { .. }) => self.advance(),
                Some(Node::Token(token, _)) => match token {
                    Token::Import => self.scan_ddd_import(),
                    Token::Ident(ddd::APPLICATION) => self.scan_ddd_application(),
                    Token::Ident(ddd::MODULE) => {
                        self.begin_top_decl();
                        self.advance();
                        self.take_name();
                        self.scan_body(BodyKind::DddModule);
                    }
                    _ => self.scan_top_junk_ddd(),
                },
            }
        }
        self.finish()
    }

    pub(crate) fn scan_ddd_application_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(ddd::BASE)) => {
                self.advance();
                self.scan_qname();
            }
            Some(Token::Ident(ddd::MODULE)) => {
                self.advance();
                self.take_name();
                self.scan_body(BodyKind::DddModule);
            }
            Some(Token::Ident(ddd::ERROR)) => self.scan_ddd_error_decl(),
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    pub(crate) fn scan_ddd_module_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(ddd::ERROR)) => self.scan_ddd_error_decl(),
            Some(Token::Ident(ddd::SERVICE)) => {
                self.advance();
                self.take_name();
                self.scan_body(BodyKind::DddService);
            }
            Some(Token::Ident(ddd::SEARCH)) => self.scan_ddd_search(),
            Some(Token::Ident(ddd::CHILDREN | ddd::EXTEND)) => self.scan_ddd_extend(),
            Some(Token::Ident(text)) if ddd::starts_design_decl(text) => self.scan_ddd_design(),
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// One design declaration: `abstract`? stereotype, the class name, the
    /// five design flags (each possibly negated, `!auditable`) — collected
    /// and re-emitted in the canonical order of [`ddd::DESIGN_FLAGS`]
    /// (`scaffold auditable optimisticLocking nonPersistent cache`) whatever
    /// the source order (mirroring the parser's `DddFlags`, whose fields the
    /// driver reads positionally) — and an optional `repository` block.
    pub(crate) fn scan_ddd_design(&mut self) {
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::ABSTRACT))) {
            self.advance();
        }
        self.advance(); // the stereotype keyword
        self.take_name();
        let mut seen = [None; ddd::DESIGN_FLAGS.len()];
        loop {
            // A negated flag is the two-token run `! keyword`; the `!` is
            // consumed without emitting (flags re-emit canonically below)
            // only once a flag keyword confirms behind it.
            let negated = matches!(self.peek_tok(), Some(Token::Other('!')))
                && self.peek_tok_nth(1).is_some_and(
                    |token| matches!(&token, Token::Ident(text) if ddd::flag_kind(text).is_some()),
                );
            if negated {
                self.bump_token();
            }
            let index = match self.peek_tok() {
                Some(Token::Ident(text)) => ddd::DESIGN_FLAGS
                    .iter()
                    .position(|(keyword, _)| *keyword == text),
                _ => None,
            };
            let Some(index) = index else {
                break;
            };
            self.bump_token();
            seen[index] = Some(negated);
        }
        for (index, (keyword, _)) in ddd::DESIGN_FLAGS.iter().enumerate() {
            if let Some(negated) = seen[index] {
                if negated {
                    self.push_text("!", false);
                    self.push_text(keyword, true);
                } else {
                    self.push_text(keyword, false);
                }
            }
        }
        // The authored aggregate boundary (DD4): `root` or
        // `aggregate <Root>`, kept on the design header line in canonical
        // order.
        let aggregate_boundary = matches!(self.peek_tok(), Some(Token::Ident(ddd::AGGREGATE)));
        if aggregate_boundary || matches!(self.peek_tok(), Some(Token::Ident(ddd::ROOT))) {
            self.advance();
            if aggregate_boundary {
                self.scan_qname();
            }
        }
        // The invariant clauses (DD5): each on its own line at one extra
        // indent, before the repository/machine blocks.
        let mut invariant_indent = false;
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::INVARIANT))) {
            self.flush_line();
            self.indent += 1;
            invariant_indent = true;
            while matches!(self.peek_tok(), Some(Token::Ident(ddd::INVARIANT))) {
                self.advance();
                self.scan_raw_expr_verbatim_until_semicolon();
                self.flush_line();
            }
        }
        // The DTO member suffix (issue pherolang#300): a projection's
        // `from <Source> { ... }` or a standalone `{ ... }` block. Both
        // blocks expand one member per line and are followed by one
        // structural blank line when another member follows them.
        if matches!(self.peek_tok(), Some(Token::From)) {
            self.advance(); // `from`
            self.scan_qname();
            if matches!(self.peek_tok(), Some(Token::LBrace)) {
                self.scan_body(BodyKind::DddDtoProjection);
                self.blank_line_after_dto_block();
            }
        } else if matches!(self.peek_tok(), Some(Token::LBrace)) {
            self.scan_body(BodyKind::DddDtoDeclared);
            self.blank_line_after_dto_block();
        }
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::REPOSITORY))) {
            self.advance();
            self.take_name();
            self.scan_body(BodyKind::DddRepository);
        }
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::MACHINE))) {
            // The machine block starts on its own line (unlike the
            // `repository` block, which joins the design line).
            self.flush_line();
            self.advance();
            self.take_name();
            if matches!(self.peek_tok(), Some(Token::Other(':'))) {
                self.bump_token();
            }
            self.push_text(":", true);
            self.scan_qname();
            if matches!(self.peek_tok(), Some(Token::From)) {
                // The reference form (issue pherolang#388): one line,
                // `machine <field>: <Enum> from "<path>";` — no body.
                self.advance();
                self.take_if(|token| matches!(token, Token::Str(_)));
                self.take_if(|token| matches!(token, Token::Other(';')));
            } else {
                self.scan_body(BodyKind::DddMachine);
            }
        }
        if invariant_indent {
            self.indent -= 1;
        }
    }

    /// One `error <Name>(<field> (, <field>)*);` declaration of an
    /// application or module body (DD7, issue pherolang#304): the
    /// declaration keyword, the name, the parenthesized payload fields (the
    /// service-operation parameter layout), and the terminating `;`.
    pub(crate) fn scan_ddd_error_decl(&mut self) {
        self.advance(); // the `error` keyword
        self.take_name();
        self.scan_ddd_params();
        self.take_if(|token| matches!(token, Token::Other(';')));
    }

    /// One member of a `machine` body: an `initial <State>;` line, a
    /// `terminal <State>;` line, or a `<From> -> <To> [via <Trigger>]
    /// [when (...)] ;` transition (the guard's parens re-emit verbatim, the
    /// `when`-condition rule).
    pub(crate) fn scan_ddd_machine_item(&mut self) {
        match self.peek_tok() {
            // The member keywords are contextual: a leading `initial` or
            // `terminal` followed by the arrow is a transition *from* a
            // state of that name, not a member line.
            Some(Token::Ident(ddd::INITIAL | ddd::TERMINAL))
                if !matches!(self.peek_tok_nth(1), Some(Token::Arrow)) =>
            {
                self.advance();
                self.take_name();
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                self.take_name();
                if self.take_if(|token| matches!(token, Token::Arrow)) {
                    self.take_name();
                    if self.take_if(|token| matches!(token, Token::Ident(ddd::VIA))) {
                        self.take_name();
                    }
                    if self.take_if(|token| matches!(token, Token::When)) {
                        self.scan_raw_parens();
                    }
                }
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    pub(crate) fn scan_ddd_service_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(ddd::INJECT)) => {
                self.advance();
                if matches!(self.peek_tok(), Some(Token::Ident(ddd::DECISION))) {
                    // The `inject decision <qualified-name> version <pin>?;`
                    // form: the discriminator re-emits verbatim, the
                    // reference scans as one dotted name, and the optional
                    // `version` pin (pherolang#154) re-emits verbatim.
                    self.advance();
                    self.scan_qname();
                    self.scan_version_pin();
                } else {
                    self.take_name();
                }
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                // An optional `protected` modifier precedes the operation
                // (contextual: anywhere else it is an ordinary name, and
                // `advance()` re-emits it verbatim).
                if matches!(self.peek_tok(), Some(Token::Ident(ddd::PROTECTED))) {
                    self.advance();
                }
                self.scan_qname();
                if self.take_if(|token| matches!(token, Token::FatArrow)) {
                    // Delegated: the `Target.operation` pair (one dotted
                    // reference).
                    self.scan_qname();
                } else {
                    // Declared: multiplicity, operation name, parameters.
                    self.scan_multiplicity();
                    self.take_name();
                    self.scan_ddd_params();
                }
                self.scan_ddd_capabilities();
                self.scan_ddd_invokes();
                self.scan_ddd_emits();
                self.scan_ddd_transition();
                self.scan_ddd_throws();
                if self.take_if(|token| matches!(token, Token::When)) {
                    self.scan_raw_parens();
                }
                if self.take_if(|token| matches!(token, Token::FatArrow)) {
                    // A statement body's primary receiver (issue pherolang#178):
                    // one repository name, then the block.
                    self.scan_qname();
                    if matches!(self.peek_tok(), Some(Token::LBrace)) {
                        self.scan_ddd_body();
                        return;
                    }
                }
                if matches!(self.peek_tok(), Some(Token::LBrace)) {
                    self.scan_ddd_body();
                    return;
                }
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// Consumes and emits an optional `capability A, B` clause, kept on the
    /// operation's line.
    pub(crate) fn scan_ddd_capabilities(&mut self) {
        if !self.take_if(|token| matches!(token, Token::Capability)) {
            return;
        }
        self.take_name();
        while self.take_if(|token| matches!(token, Token::Comma)) {
            self.take_name();
        }
    }

    /// Consumes and emits an optional `invokes A.B version <pin>?, C
    /// version <pin>?` clause, kept on the operation's line: the canonical
    /// position between the `capability` clause and the `emits` clause
    /// (the parser enforces that order, so re-emitting in scan order is
    /// the canonical order). `invokes` is a contextual word — an ordinary
    /// identifier everywhere else — so the scan matches the
    /// [`ddd::INVOKES`] identifier, not a keyword token. The names are
    /// qualified and re-emitted as dotted references; each entry's
    /// optional `version` pin (pherolang#154) re-emits verbatim after its
    /// name. Repository operations never take the clause.
    pub(crate) fn scan_ddd_invokes(&mut self) {
        if !self.take_if(|token| matches!(token, Token::Ident(ddd::INVOKES))) {
            return;
        }
        self.scan_qname();
        self.scan_version_pin();
        while self.take_if(|token| matches!(token, Token::Comma)) {
            self.scan_qname();
            self.scan_version_pin();
        }
    }

    /// Consumes and emits an optional `version compatible` /
    /// `version "X.Y.Z"` pin of a decision-service reference (pherolang#154),
    /// re-emitted verbatim. The `version` discriminator is the existing
    /// lexer keyword token; the value is the contextual `compatible` word
    /// or the exact-version string. Anything else after the discriminator
    /// stays unconsumed — the parser already reported the malformed pin.
    pub(crate) fn scan_version_pin(&mut self) {
        if !self.take_if(|token| matches!(token, Token::Version)) {
            return;
        }
        self.take_if(|token| {
            matches!(token, Token::Str(_)) || matches!(token, Token::Ident(ddd::COMPATIBLE))
        });
    }

    /// Consumes and emits an optional `emits A, B` clause, kept on the
    /// operation's line: the canonical position between the `invokes`
    /// clause and the `when` guard (the parser enforces that order, so
    /// re-emitting in scan order is the canonical order). `emits` is a
    /// contextual word — an ordinary identifier everywhere else — so the
    /// scan matches the [`ddd::EMITS`] identifier, not a keyword token.
    /// Repository operations never take the clause.
    pub(crate) fn scan_ddd_emits(&mut self) {
        if !self.take_if(|token| matches!(token, Token::Ident(ddd::EMITS))) {
            return;
        }
        self.take_name();
        while self.take_if(|token| matches!(token, Token::Comma)) {
            self.take_name();
        }
    }

    /// Consumes and emits an optional `transition <Trigger>` clause, kept
    /// on the operation's line: the canonical position between the `emits`
    /// clause and the `when` guard (the parser enforces that order, so
    /// re-emitting in scan order is the canonical order; issue
    /// pherolang#176). `transition` is a contextual word — an ordinary
    /// identifier everywhere else — so the scan matches the
    /// [`ddd::TRANSITION`] identifier, not a keyword token. The binding is
    /// one name or a dotted `EnumType.Trigger` pair, re-emitted as written.
    /// Repository operations never take the clause.
    pub(crate) fn scan_ddd_transition(&mut self) {
        if !self.take_if(|token| matches!(token, Token::Ident(ddd::TRANSITION))) {
            return;
        }
        self.scan_qname();
    }

    /// Consumes and emits an optional `throws <Error> (, <Error>)*` clause,
    /// kept on the operation's line: the canonical position between the
    /// `transition` binding and the `when` guard (service operations) or
    /// between the signature and the `when` guard (repository operations) —
    /// the parser enforces that order, so re-emitting in scan order is the
    /// canonical order (DD7, issue pherolang#304). `throws` is a contextual
    /// word, so the scan matches the [`ddd::THROWS`] identifier.
    pub(crate) fn scan_ddd_throws(&mut self) {
        if !self.take_if(|token| matches!(token, Token::Ident(ddd::THROWS))) {
            return;
        }
        self.take_name();
        while self.take_if(|token| matches!(token, Token::Comma)) {
            self.take_name();
        }
    }

    /// Consumes and emits a repository query operation's clause sequence
    /// (DD9, typed repository queries) in canonical order `where? sort?
    /// page?`: the `where` predicate re-emits through
    /// [`Formatter::scan_raw_parens`] — the `when`-guard convention, so the
    /// source between the parentheses stays verbatim — the `sort` keys one
    /// after another with the default `asc` direction dropped, and the
    /// `page` members (`limit`, `max`, `cursor`) as written (`page` must
    /// follow `sort`, and the caller scans the trailing `capability` clause).
    /// The clause order itself is canonicalized ahead of the scan by
    /// [`Formatter::reorder_ddd_repository_clauses`].
    pub(crate) fn scan_ddd_query_clauses(&mut self) {
        if self.take_if(|token| matches!(token, Token::Ident(ddd::WHERE))) {
            self.scan_raw_parens();
        }
        if self.take_if(|token| matches!(token, Token::Ident(ddd::SORT))) {
            self.scan_ddd_sort_key();
            while self.take_if(|token| matches!(token, Token::Comma)) {
                self.scan_ddd_sort_key();
            }
        }
        if self.take_if(|token| matches!(token, Token::Ident(ddd::PAGE))) {
            loop {
                match self.peek_tok() {
                    Some(Token::Ident(text)) if text == ddd::LIMIT || text == ddd::MAX => {
                        self.advance();
                        self.take_if(|token| matches!(token, Token::Int(_)));
                    }
                    Some(Token::Ident(ddd::CURSOR)) => self.advance(),
                    _ => break,
                }
            }
        }
    }

    /// One `field ("asc" | "desc")?` key of a repository query's `sort`
    /// clause (DD9): the field re-emits as written and `desc` is kept, while
    /// the default `asc` is consumed without emitting — canonical output
    /// omits it (`sort createdDate desc, id asc` formats to
    /// `sort createdDate desc, id`).
    fn scan_ddd_sort_key(&mut self) {
        if !self.take_name() {
            return;
        }
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::ASC))) {
            self.bump_token();
        } else if matches!(self.peek_tok(), Some(Token::Ident(ddd::DESC))) {
            self.advance();
        }
    }

    /// Like [`Formatter::scan_params`], but for `.ddd` parameters, which may
    /// carry a multiplicity between the type and the name.
    pub(crate) fn scan_ddd_params(&mut self) {
        if !self.take_if(|token| matches!(token, Token::LParen)) {
            return;
        }
        if self.take_if(|token| matches!(token, Token::RParen)) {
            return;
        }
        loop {
            self.scan_qname();
            self.scan_multiplicity();
            self.take_name();
            if !self.take_if(|token| matches!(token, Token::Comma)) {
                break;
            }
        }
        self.take_if(|token| matches!(token, Token::RParen));
    }

    /// Rewrites the node stream of one repository operation ahead of the
    /// formatter (positioned at the operation's first token) so its clauses
    /// appear in the canonical order `throws? when? where? sort? page?
    /// capability?` whatever the source order — the same node-range technique
    /// [`Formatter::reorder_ddd_search_members`] uses for search bodies.
    ///
    /// The operation's extent is the token run up to (not including) its
    /// depth-0 terminator: the `;`, the opening `{` of a statement body, or
    /// the repository body's closing `}`. Segments split at the clause-leading
    /// words only when the word's shape confirms the clause (a `where` must
    /// be followed by `(`, a `sort` by a key, a `page` by one of its members
    /// or the terminator), so a contextual identifier that merely spells a
    /// clause word — an operation named `where`, a sort key named `page` —
    /// is never mistaken for a boundary. The first segment (the signature)
    /// keeps the lowest rank; comments travel with the segment they precede.
    /// An already-canonical operation is left untouched.
    pub(crate) fn reorder_ddd_repository_clauses(&mut self) {
        self.drain_comments();
        let start = self.pos;
        let mut depth = 0usize;
        let mut end = self.nodes.len();
        let mut index = start;
        while let Some(node) = self.nodes.get(index) {
            match node {
                Node::Token(Token::Other(';') | Token::RBrace, _) if depth == 0 => {
                    end = index;
                    break;
                }
                Node::Token(Token::LBrace, _) if depth == 0 => {
                    end = index;
                    break;
                }
                Node::Token(Token::LParen | Token::LBracket | Token::LBrace, _) => depth += 1,
                Node::Token(Token::RParen | Token::RBracket | Token::RBrace, _) => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
            index += 1;
        }
        let region = &self.nodes[start..end];
        let mut segments: Vec<(usize, Vec<Node<'src>>)> = Vec::new();
        let mut depth = 0usize;
        for (offset, node) in region.iter().enumerate() {
            let rank = if depth == 0 {
                repository_clause_rank(region, offset)
            } else {
                None
            };
            if segments.is_empty() {
                // The signature segment leads; every clause ranks above it.
                segments.push((rank.unwrap_or(usize::MIN), Vec::new()));
            } else if let Some(rank) = rank {
                segments.push((rank, Vec::new()));
            }
            match node {
                Node::Token(Token::LParen | Token::LBracket | Token::LBrace, _) => depth += 1,
                Node::Token(Token::RParen | Token::RBracket | Token::RBrace, _) => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
            segments.last_mut().unwrap().1.push(node.clone());
        }
        if segments.windows(2).all(|pair| pair[0].0 <= pair[1].0) {
            return;
        }
        segments.sort_by_key(|(rank, _)| *rank);
        let mut reordered: Vec<Node<'src>> = self.nodes[..start].to_vec();
        for (_, segment) in &segments {
            reordered.extend(segment.iter().cloned());
        }
        reordered.extend(self.nodes[end..].iter().cloned());
        self.nodes = reordered;
        self.pos = start;
    }

    pub(crate) fn scan_ddd_repository_item(&mut self) {
        // The query clauses are re-emitted in the canonical order
        // `throws? where? sort? page? capability?` whatever the source order
        // (DD9); every other repository operation is already in order, so
        // the rewrite is the identity for it.
        self.reorder_ddd_repository_clauses();
        match self.peek_tok() {
            // A `protected` modifier on a built-in (`protected findByKeys;`):
            // both tokens re-emit verbatim via `advance()`.
            Some(Token::Ident(ddd::PROTECTED))
                if self.peek_tok_nth(1).is_some_and(|token| {
                    matches!(&token, Token::Ident(text) if ddd::builtin_from_keyword(text).is_some())
                }) && !matches!(self.peek_tok_nth(2), Some(Token::LParen)) =>
            {
                self.advance();
                self.advance();
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            Some(Token::Ident(text))
                if ddd::builtin_from_keyword(text).is_some()
                    && !matches!(self.peek_tok_nth(1), Some(Token::LParen)) =>
            {
                self.advance();
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                // An optional `protected` modifier precedes the operation
                // (contextual: anywhere else it is an ordinary name, and
                // `advance()` re-emits it verbatim).
                if matches!(self.peek_tok(), Some(Token::Ident(ddd::PROTECTED))) {
                    self.advance();
                }
                self.scan_qname();
                self.scan_multiplicity();
                self.take_name();
                self.scan_ddd_params();
                self.scan_ddd_throws();
                if self.take_if(|token| matches!(token, Token::When)) {
                    self.scan_raw_parens();
                }
                // The query clauses (DD9), canonical order `where sort page`,
                // followed by the operation's `capability` clause.
                self.scan_ddd_query_clauses();
                if matches!(self.peek_tok(), Some(Token::LBrace)) {
                    self.scan_ddd_body();
                    return;
                }
                self.scan_ddd_capabilities();
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// Consumes and emits one executable statement body (issue pherolang#178),
    /// positioned at its `{`: the opening brace stays on the operation's
    /// header line, statements render one per line at the next indent, and
    /// the closing `}` sits alone on its line. Interior blank lines are
    /// collapsed (the `.ddd` body rule).
    pub(crate) fn scan_ddd_body(&mut self) {
        if !self.take_if(|token| matches!(token, Token::LBrace)) {
            self.flush_line();
            return;
        }
        self.flush_line();
        self.indent += 1;
        self.scan_ddd_body_statements();
        self.indent -= 1;
        self.take_if(|token| matches!(token, Token::RBrace));
        self.flush_line();
    }

    fn scan_ddd_body_statements(&mut self) {
        loop {
            match self.peek_tok() {
                Some(Token::RBrace) | None => break,
                _ => {}
            }
            self.flush_pending(self.indent);
            self.scan_ddd_body_statement();
        }
    }

    fn scan_ddd_body_statement(&mut self) {
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::IF))) {
            self.scan_ddd_body_if();
        } else {
            self.scan_ddd_body_simple_statement();
        }
    }

    /// One non-`if` statement: tokens through the depth-0 `;` (or up to the
    /// closing body brace), emitted on one line at canonical spacing.
    fn scan_ddd_body_simple_statement(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek_tok() {
                None => break,
                Some(Token::Other(';')) if depth == 0 => {
                    self.advance_body_token();
                    break;
                }
                Some(Token::RBrace) if depth == 0 => break,
                Some(token) => {
                    match token {
                        Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                        Token::RParen | Token::RBracket | Token::RBrace => {
                            depth = depth.saturating_sub(1);
                        }
                        _ => {}
                    }
                    self.advance_body_token();
                }
            }
        }
        self.flush_line();
    }

    /// One `if` statement: `if <cond> {` / nested statements / `}` and an
    /// optional `else { ... }` on the closing line (`} else {`).
    fn scan_ddd_body_if(&mut self) {
        self.advance_body_token(); // `if`
                                   // `if (cond)` keeps its space: the generic token rules glue `(` to
                                   // the preceding name (`f(x)`), but a statement keyword reads as a
                                   // keyword followed by the parenthesized condition.
        if matches!(self.peek_tok(), Some(Token::LParen)) {
            self.bump_token();
            self.push_text("(", false);
            self.glue_next = true;
        }
        self.scan_ddd_body_condition();
        self.advance_body_token(); // `{`
        self.flush_line();
        self.indent += 1;
        self.scan_ddd_body_statements();
        self.indent -= 1;
        self.advance_body_token(); // `}`
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::ELSE))) {
            self.advance_body_token(); // `else`
            self.advance_body_token(); // `{`
            self.flush_line();
            self.indent += 1;
            self.scan_ddd_body_statements();
            self.indent -= 1;
            self.advance_body_token(); // `}`
        }
        self.flush_line();
    }

    /// An `if` statement's condition: tokens up to the block's depth-0 `{`,
    /// consuming an expression-level `if cond { ... } else { ... }` atom
    /// whole so its braces are not mistaken for the statement block.
    fn scan_ddd_body_condition(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek_tok() {
                None => break,
                Some(Token::LBrace) if depth == 0 => break,
                Some(Token::Ident(ddd::IF)) if depth == 0 => {
                    self.scan_ddd_body_if_expression();
                }
                Some(token) => {
                    match token {
                        Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                        Token::RParen | Token::RBracket | Token::RBrace => {
                            depth = depth.saturating_sub(1);
                        }
                        _ => {}
                    }
                    self.advance_body_token();
                }
            }
        }
    }

    /// One expression-level `if cond { then } else { else }` atom, emitted
    /// inline (it is part of a statement expression, not a block).
    fn scan_ddd_body_if_expression(&mut self) {
        self.advance_body_token(); // `if`
        self.scan_ddd_body_condition();
        self.scan_ddd_body_braced_group();
        if matches!(self.peek_tok(), Some(Token::Ident(ddd::ELSE))) {
            self.advance_body_token();
            self.scan_ddd_body_braced_group();
        }
    }

    fn scan_ddd_body_braced_group(&mut self) {
        if !self.take_if(|token| matches!(token, Token::LBrace)) {
            return;
        }
        let mut depth = 1usize;
        while self.peek_tok().is_some() {
            let token = self.peek_tok();
            match token {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.advance_body_token();
            if depth == 0 {
                break;
            }
        }
    }

    /// Advances one body-expression token at canonical spacing, gluing the
    /// two halves of the multi-character operators the lexer splits
    /// (`!`/`=`, `=`/`=`, `<`/`=`, `>`/`=`, `&`/`&`, `|`/`|`, `?`/`.`,
    /// `?`/`:`) and unary `!` to its operand, so the emitted text re-lexes
    /// to the same expression.
    fn advance_body_token(&mut self) {
        let token = self.peek_tok();
        self.advance();
        let glue = match token {
            Some(Token::Other('!' | '&' | '|' | '?')) => true,
            Some(Token::Other('<' | '>')) => matches!(self.peek_tok(), Some(Token::Eq)),
            Some(Token::Eq) => matches!(self.peek_tok(), Some(Token::Eq)),
            _ => false,
        };
        if glue {
            self.glue_next = true;
        }
    }

    /// Consumes and emits one `search <name> { ... }` projection. The body's
    /// members are rewritten into the canonical order (see
    /// [`Formatter::reorder_ddd_search_members`]) before the linear scan.
    pub(crate) fn scan_ddd_search(&mut self) {
        self.advance(); // the `search` keyword
        self.take_name();
        self.reorder_ddd_search_members();
        self.scan_body(BodyKind::DddSearch);
    }

    /// Consumes and emits one module-level `children <Class> { ... }` block
    /// (issue pherolang#179). The header expands like the `service`/design
    /// headers; each child operation is one canonical line
    /// (`op signature => feature.action when (...) ;`, the service-op
    /// layout).
    pub(crate) fn scan_ddd_extend(&mut self) {
        // The 0.4 `extend` alias is rewritten to the canonical `children`
        // (DD4); it was removed from the grammar in 0.5 (DF4), so this
        // rewrite is the mechanical migration path — a `children` header
        // re-emits as written. Both consume the same body.
        self.bump_token();
        self.push_text(ddd::CHILDREN, false);
        self.take_name();
        self.scan_body(BodyKind::DddExtend);
    }

    /// One child operation of a `children` body:
    /// `protected? type_ref multiplicity? name(params) "=>"
    /// feature.action when? ";"` — the declared-service-operation layout
    /// followed by the dotted child target.
    pub(crate) fn scan_ddd_child_op_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                // An optional `protected` modifier precedes the operation
                // (contextual: anywhere else it is an ordinary name, and
                // `advance()` re-emits it verbatim).
                if matches!(self.peek_tok(), Some(Token::Ident(ddd::PROTECTED))) {
                    self.advance();
                }
                // The signature: return type, optional multiplicity, the
                // operation name, and the parameters.
                self.scan_qname();
                self.scan_multiplicity();
                self.take_name();
                self.scan_ddd_params();
                if self.take_if(|token| matches!(token, Token::FatArrow)) {
                    // The `<feature>.<action>` pair, re-emitted as written.
                    self.scan_qname();
                }
                if self.take_if(|token| matches!(token, Token::When)) {
                    self.scan_raw_parens();
                }
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// Rewrites the node stream of the search body ahead of the formatter
    /// (positioned at the body's `{`) so its members appear in the canonical
    /// order (the [`ddd::SEARCH_MEMBERS`] table order, via
    /// [`ddd::search_member_rank`]) whatever the source order — the same
    /// node-range technique the import hoists use. Segments split at
    /// brace-depth 0 on the member-leading keywords; segments that start
    /// with anything else (junk from a parse-error region) keep their
    /// relative order at the end, and a segment's leading comments travel
    /// with it.
    pub(crate) fn reorder_ddd_search_members(&mut self) {
        if !matches!(self.front(), Some(Node::Token(Token::LBrace, _))) {
            return;
        }
        let open = self.pos;
        let mut depth = 0usize;
        let close = loop {
            match self.nodes.get(self.pos) {
                Some(Node::Token(Token::LBrace, _)) => {
                    depth += 1;
                    self.pos += 1;
                }
                Some(Node::Token(Token::RBrace, _)) => {
                    depth = depth.saturating_sub(1);
                    self.pos += 1;
                    if depth == 0 {
                        break self.pos;
                    }
                }
                Some(_) => self.pos += 1,
                None => return,
            }
        };
        let mut segments: Vec<(usize, Vec<Node<'src>>)> = Vec::new();
        let mut depth = 0usize;
        for node in self.nodes[open + 1..close - 1].iter() {
            let rank = match node {
                Node::Token(token, _) if depth == 0 => ddd::search_member_rank(token),
                _ => None,
            };
            if segments.is_empty() {
                segments.push((rank.unwrap_or(usize::MAX), Vec::new()));
            } else if let Some(rank) = rank {
                segments.push((rank, Vec::new()));
            }
            match node {
                Node::Token(Token::LBrace, _) => depth += 1,
                Node::Token(Token::RBrace, _) => depth = depth.saturating_sub(1),
                _ => {}
            }
            segments.last_mut().unwrap().1.push(node.clone());
        }
        segments.sort_by_key(|(rank, _)| *rank);
        let mut reordered: Vec<Node<'src>> = self.nodes[..=open].to_vec();
        for (_, segment) in &segments {
            reordered.extend(segment.iter().cloned());
        }
        reordered.extend(self.nodes[close - 1..].iter().cloned());
        self.pos = open;
        self.nodes = reordered;
    }

    pub(crate) fn scan_ddd_search_item(&mut self) {
        match self
            .peek_tok()
            .and_then(|token| ddd::search_member_kind(&token))
        {
            Some(ddd::SearchMemberKind::Entity) => {
                self.advance();
                self.scan_qname();
            }
            Some(ddd::SearchMemberKind::Text) => {
                self.advance();
                self.scan_body(BodyKind::DddSearchField);
            }
            Some(ddd::SearchMemberKind::Filters | ddd::SearchMemberKind::Sort) => {
                self.advance();
                self.scan_body(BodyKind::DddSearchNames);
            }
            Some(ddd::SearchMemberKind::Document) => {
                self.advance();
                if matches!(self.peek_tok(), Some(Token::LBrace)) {
                    self.scan_body(BodyKind::DddSearchDocument);
                } else {
                    // `document DtoName;` (DF3, issue pherolang#326): the
                    // named form is one canonical line, like an `entity` line.
                    self.scan_qname();
                    self.take_if(|token| matches!(token, Token::Other(';')));
                }
            }
            Some(ddd::SearchMemberKind::Ranking) => {
                self.advance();
                if matches!(self.peek_tok(), Some(Token::Ident(_)) | Some(Token::Str(_))) {
                    self.advance();
                }
            }
            Some(ddd::SearchMemberKind::Analyzer) => {
                self.advance();
                self.take_if(|token| matches!(token, Token::Str(_)));
            }
            Some(ddd::SearchMemberKind::Pagination) => {
                self.advance();
                self.scan_body(BodyKind::DddSearchPagination);
            }
            Some(ddd::SearchMemberKind::Capability) => self.scan_ddd_capabilities(),
            _ => {
                // Junk always consumes the front token first (the stop set
                // contains identifiers, and the body loop cannot make
                // progress otherwise) — unless the front is the body's
                // closing `}` (or the stream ended), which the loop breaks
                // on; a drained trailing comment may have just revealed it.
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// One `text` field: `<property> (boost <int>)? (analyzer "<...>")?`,
    /// one field per line.
    pub(crate) fn scan_ddd_search_field_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                self.scan_qname();
                if self.take_if(|token| matches!(token, Token::Ident(ddd::BOOST))) {
                    self.take_if(|token| matches!(token, Token::Int(_)));
                }
                if self.take_if(|token| matches!(token, Token::Ident(ddd::ANALYZER))) {
                    self.take_if(|token| matches!(token, Token::Str(_)));
                }
            }
            _ => {
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// One name of a `filters`/`sort` clause, one per line.
    pub(crate) fn scan_ddd_search_name_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => self.scan_qname(),
            _ => {
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// Emits exactly one structural blank line after a DTO member block,
    /// unless the block ends the enclosing body (or a trailing comment owns
    /// the line directly after it).
    fn blank_line_after_dto_block(&mut self) {
        match self.front() {
            None | Some(Node::Token(Token::RBrace, _)) => return,
            Some(Node::Comment {
                own_line: false, ..
            }) => return,
            _ => {}
        }
        if !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    /// Re-emits a raw expression **verbatim** from the source slice, from
    /// the current token through the depth-0 `;` (the `raw_expr`
    /// convention). Verbatim is the contract for invariants: operator
    /// spellings (`==`, `>=`, `=>`) must not be respaced token by token.
    /// The caller owns the keyword before it and the line break after it.
    pub(crate) fn scan_raw_expr_verbatim_until_semicolon(&mut self) {
        let Some(start) = self.front().map(|node| node.span().start) else {
            return;
        };
        let mut depth = 0usize;
        let mut index = self.pos;
        let mut end = self.source.len();
        while let Some(node) = self.nodes.get(index) {
            match node {
                Node::Token(Token::Other(';'), span) if depth == 0 => {
                    end = span.start;
                    break;
                }
                Node::Token(Token::LParen | Token::LBracket | Token::LBrace, _) => depth += 1,
                Node::Token(Token::RParen | Token::RBracket | Token::RBrace, _) => {
                    depth = depth.saturating_sub(1)
                }
                _ => {}
            }
            index += 1;
        }
        let text = self.source[start..end].trim().to_string();
        self.pos = index;
        if !text.is_empty() {
            self.push_text(&text, false);
        }
        self.take_if(|token| matches!(token, Token::Other(';')));
    }

    /// One member of a projection DTO block (issue pherolang#300): `name;`
    /// or `name = <expression>;`. The expression re-emits verbatim token by
    /// token at canonical spacing, terminating at the depth-0 `;` — the same
    /// layout the search `document` clause uses (the two share the AST).
    pub(crate) fn scan_ddd_projection_member(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                self.take_name();
                if self.take_if(|token| matches!(token, Token::Eq)) {
                    let mut depth = 0usize;
                    loop {
                        match self.peek_tok() {
                            None => break,
                            Some(Token::Other(';')) if depth == 0 => {
                                self.advance();
                                break;
                            }
                            Some(Token::RBrace) if depth == 0 => break,
                            Some(token) => {
                                match token {
                                    Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                                    Token::RParen | Token::RBracket | Token::RBrace => {
                                        depth = depth.saturating_sub(1)
                                    }
                                    _ => {}
                                }
                                self.advance();
                            }
                        }
                    }
                } else {
                    self.take_if(|token| matches!(token, Token::Other(';')));
                }
            }
            _ => {
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// One member of a standalone DTO block (issue pherolang#300):
    /// `type_ref multiplicity? name;`, one member per line.
    pub(crate) fn scan_ddd_declared_member(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(_) | Token::IdentEscaped(_)) => {
                self.scan_qname();
                self.scan_multiplicity();
                self.take_name();
                self.take_if(|token| matches!(token, Token::Other(';')));
            }
            _ => {
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    /// One `name = <expr>;` entry of a `document` clause: the expression is
    /// re-emitted verbatim token by token (canonical spacing), terminating
    /// at the depth-0 `;`. String literals are single tokens, so a `;`
    /// inside one never terminates; a missing terminator leaves the closing
    /// `}` for the body loop. The entry shares the projection-member layout.
    pub(crate) fn scan_ddd_search_document_item(&mut self) {
        self.scan_ddd_projection_member();
    }

    /// One `limit <int>` / `max <int>` / `cursor` member of `pagination`.
    pub(crate) fn scan_ddd_search_pagination_item(&mut self) {
        match self.peek_tok() {
            Some(Token::Ident(text)) if text == ddd::LIMIT || text == ddd::MAX => {
                self.advance();
                self.take_if(|token| matches!(token, Token::Int(_)));
            }
            Some(Token::Ident(ddd::CURSOR)) => self.advance(),
            _ => {
                if !matches!(self.peek_tok(), Some(Token::RBrace) | None) {
                    self.advance();
                    self.scan_junk_until(|token| {
                        matches!(
                            token,
                            Token::RBrace | Token::Ident(_) | Token::IdentEscaped(_)
                        )
                    });
                }
            }
        }
    }

    // --- .evt bodies ---------------------------------------------------------

    /// Rewrites the node stream into canonical order for `.ddd` files:
    /// `import` declarations first (source order), then the `application`
    /// block and any junk items in source order.
    ///
    /// Items are contiguous node ranges split at brace-depth 0 on `Import`
    /// tokens and the contextual `application` identifier; the first node
    /// unconditionally begins item 0, so leading comments travel with their
    /// item.
    pub(crate) fn hoist_ddd_imports(&mut self) {
        let mut items: Vec<(usize, usize)> = Vec::new();
        let mut depth = 0usize;
        for (index, node) in self.nodes.iter().enumerate() {
            let token = match node {
                Node::Token(token, _) => Some(token),
                Node::Comment { .. } => None,
            };
            let starts_item = matches!(
                token,
                Some(Token::Import)
                    | Some(Token::Ident("application"))
                    | Some(Token::Ident(ddd::MODULE))
            );
            if items.is_empty() || (depth == 0 && starts_item) {
                items.push((index, index));
            }
            let last = items.len() - 1;
            items[last].1 = index + 1;
            match token {
                Some(Token::LBrace) => depth += 1,
                Some(Token::RBrace) => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if items.len() < 2 {
            return;
        }
        // A leading comment block (a file header) stays at the very front:
        // the import section is inserted after it.
        let anchor = if matches!(self.nodes.first(), Some(Node::Comment { .. })) {
            1
        } else {
            0
        };
        let imports: Vec<(usize, usize)> = items
            .iter()
            .copied()
            .filter(|(start, _)| matches!(self.nodes[*start], Node::Token(Token::Import, _)))
            .collect();
        if imports.is_empty() {
            return;
        }
        let is_moved = |index: usize, imports: &[(usize, usize)]| {
            imports
                .iter()
                .any(|(start, end)| index >= *start && index < *end)
        };
        let mut reordered: Vec<Node<'src>> = Vec::with_capacity(self.nodes.len());
        for range in &items[..anchor] {
            reordered.extend(self.nodes[range.0..range.1].iter().cloned());
        }
        for range in &imports {
            reordered.extend(self.nodes[range.0..range.1].iter().cloned());
        }
        for range in &items[anchor..] {
            if is_moved(range.0, &imports) {
                continue;
            }
            reordered.extend(self.nodes[range.0..range.1].iter().cloned());
        }
        self.nodes = reordered;
    }

    /// Consumes and emits one `import "<path>";` declaration of a `.ddd`
    /// file: the tight import section of the `.actor` formatter, plus the
    /// canonical trailing `;` (emitted whether or not the source had one; a
    /// source `;` is consumed silently so it never doubles up).
    pub(crate) fn scan_ddd_import(&mut self) {
        self.flush_line();
        self.flush_pending(0);
        self.advance();
        self.take_if(|token| matches!(token, Token::Str(_)));
        if matches!(self.front(), Some(Node::Token(Token::Other(';'), _))) {
            self.bump_token();
        }
        self.push_text(";", true);
        self.flush_line();
    }

    /// Consumes and emits the `application <name> { ... }` block of a
    /// `.ddd` file.
    pub(crate) fn scan_ddd_application(&mut self) {
        self.begin_top_decl();
        self.advance();
        self.take_name();
        self.scan_body(BodyKind::DddApplication);
    }

    /// Unrecognized top-level tokens of a `.ddd` file: emit them on one
    /// line, stopping at the next `import` keyword or the contextual
    /// `application` keyword (mirrors the parser's file-level recovery).
    pub(crate) fn scan_top_junk_ddd(&mut self) {
        self.begin_top_decl();
        loop {
            match self.peek_tok() {
                Some(token) if !matches!(token, Token::Import | Token::Ident("application")) => {
                    self.advance()
                }
                _ => break,
            }
        }
        self.flush_line();
    }
}

/// The canonical rank of a repository operation's clause keyword at `index`
/// (DD9 plus the pre-existing `throws`/`when`), or `None` when the token does
/// not start a clause.
///
/// The ranks are the canonical clause order
/// `throws(1) when(2) where(3) sort(4) page(5) capability(6)`; the operation's
/// signature segment takes [`usize::MIN`] so it always leads. Each contextual
/// word additionally has to be **shaped** like its clause — `where` followed
/// by `(`, `sort` followed by a key, `page` followed by a member or the
/// terminator, `throws` followed by an error name — so an ordinary identifier
/// that happens to spell a clause word never becomes a segment boundary.
fn repository_clause_rank(nodes: &[Node<'_>], index: usize) -> Option<usize> {
    let token = match nodes.get(index) {
        Some(Node::Token(token, _)) => token,
        _ => return None,
    };
    let next = nodes[index + 1..].iter().find_map(|node| match node {
        Node::Token(token, _) => Some(token),
        Node::Comment { .. } => None,
    });
    match token {
        Token::Ident(ddd::THROWS) => {
            matches!(next, Some(Token::Ident(_) | Token::IdentEscaped(_))).then_some(1)
        }
        Token::When => Some(2),
        Token::Ident(ddd::WHERE) => matches!(next, Some(Token::LParen)).then_some(3),
        Token::Ident(ddd::SORT) => {
            matches!(next, Some(Token::Ident(_) | Token::IdentEscaped(_))).then_some(4)
        }
        Token::Ident(ddd::PAGE) => matches!(
            next,
            Some(Token::Ident(ddd::LIMIT | ddd::MAX | ddd::CURSOR))
                | Some(Token::Other(';'))
                | None
        )
        .then_some(5),
        Token::Capability => Some(6),
        _ => None,
    }
}
