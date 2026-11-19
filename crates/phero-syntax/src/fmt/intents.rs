//! The `.intent` formatter: the import hoist and the intent/link scanners.

use super::*;
use crate::intents;

impl<'src> Formatter<'src> {
    /// Like [`Formatter::run_usecases`], but for `.intent` files: the only
    /// recognized top-level constructs are `import` declarations and the
    /// contextual `intent` blocks, canonicalized into imports → intents
    /// order by [`Formatter::hoist_intent_items`].
    pub(crate) fn run_intents(&mut self) -> String {
        self.hoist_intent_items();
        loop {
            let front = self.front().cloned();
            match front {
                None => break,
                Some(Node::Comment { .. }) => self.advance(),
                Some(Node::Token(token, _)) => match token {
                    Token::Import => self.scan_intent_import(),
                    Token::Ident(intents::INTENT) => self.scan_intent_decl(),
                    _ => self.scan_top_junk_intents(),
                },
            }
        }
        self.finish()
    }

    /// The `.intent` hoist: the [`Formatter::hoist_uc_items`] machinery with
    /// the contextual `intent` rank.
    pub(crate) fn hoist_intent_items(&mut self) {
        fn rank(token: &Token<'_>) -> usize {
            match token {
                Token::Import => 0,
                Token::Ident(intents::INTENT) => 1,
                _ => usize::MAX,
            }
        }

        // The header is the comment run before the first token.
        let header_end = self
            .nodes
            .iter()
            .position(|node| matches!(node, Node::Token(..)))
            .unwrap_or(self.nodes.len());

        let mut items: Vec<(usize, usize, usize)> = Vec::new(); // (rank, start, end)
        let mut depth = 0usize;
        for (index, node) in self.nodes.iter().enumerate().skip(header_end) {
            let (starts_item, rank) = match node {
                Node::Token(token, _) => (depth == 0 && rank(token) != usize::MAX, rank(token)),
                Node::Comment { own_line, .. } => {
                    // A depth-0 own-line comment whose next token is a
                    // top-level declaration begins that declaration's item.
                    let leads_decl = *own_line
                        && depth == 0
                        && self.nodes[index..].iter().find_map(|node| match node {
                            Node::Token(token, _) => Some(rank(token) != usize::MAX),
                            Node::Comment { .. } => None,
                        }) == Some(true);
                    let rank = if leads_decl {
                        self.nodes[index..]
                            .iter()
                            .find_map(|node| match node {
                                Node::Token(token, _) => Some(rank(token)),
                                _ => None,
                            })
                            .unwrap_or(usize::MAX)
                    } else {
                        usize::MAX
                    };
                    (leads_decl, rank)
                }
            };
            if starts_item || items.is_empty() {
                items.push((rank, index, index));
            }
            items.last_mut().unwrap().2 = index + 1;
            if let Node::Token(token, _) = node {
                match token {
                    Token::LBrace => depth += 1,
                    Token::RBrace => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        if items.len() < 2 {
            return;
        }
        // Stable: source order survives within a rank (junk included).
        items.sort_by_key(|(rank, _, _)| *rank);
        let mut reordered: Vec<Node<'src>> = self.nodes[..header_end].to_vec();
        for (_, start, end) in &items {
            reordered.extend(self.nodes[*start..*end].iter().cloned());
        }
        self.nodes = reordered;
    }

    /// One `import "<path>";` line, re-emitted tight with its canonical `;`.
    pub(crate) fn scan_intent_import(&mut self) {
        self.flush_line();
        self.flush_pending(0);
        self.advance();
        self.take_if(|token| matches!(token, Token::Str(_)));
        if !self.take_if(|token| matches!(token, Token::Other(';'))) {
            self.push_text(";", true);
        }
        self.flush_line();
    }

    /// Consumes and emits one `intent <Id> "<statement>" { ... }`
    /// declaration. The header stays one line (the id and the statement
    /// string); each link is one canonical line (`element`/`check`,
    /// `kind:name`, canonical `;`).
    pub(crate) fn scan_intent_decl(&mut self) {
        self.begin_top_decl();
        self.advance(); // the `intent` keyword
        self.take_intent_id();
        self.take_if(|token| matches!(token, Token::Str(_)));
        self.scan_body(BodyKind::IntentsBody);
    }

    /// Consumes and emits one stable id: the lead name plus every
    /// hyphen-led segment, all glued on one line. The hyphen lexes inside a
    /// signed int (`-001` is one `Int` token) or as the `Other` catch-all
    /// before a letter segment — both re-emit glued, so `LOAN-001` and
    /// `REFUND-DESK-007` come back exactly as written.
    fn take_intent_id(&mut self) {
        if !self.take_name() {
            return;
        }
        loop {
            self.drain_comments();
            match self.front() {
                Some(Node::Token(Token::Int(_), span)) => {
                    // The signed-int segment (`-001`): emit the raw source
                    // slice — the parsed value would lose leading zeros.
                    let text = self.source[span.start..span.end].to_string();
                    self.glue_next = true;
                    self.bump_token();
                    self.push_text(&text, true);
                }
                Some(Node::Token(Token::Other('-'), _)) => {
                    self.glue_next = true;
                    self.advance();
                    // A letter segment follows its hyphen, glued.
                    self.glue_next = true;
                    self.take_ref_name();
                }
                _ => break,
            }
        }
    }

    /// File-level junk: consume through the chunk up to the next import or
    /// intent declaration, keeping progress.
    pub(crate) fn scan_top_junk_intents(&mut self) {
        self.advance();
        self.scan_junk_until(|token| {
            matches!(token, Token::Import | Token::Ident(intents::INTENT))
        });
    }

    /// One link line of an intent body: the `element`/`check` keyword, the
    /// kind word, the tight `:`, the dotted name as written, the canonical
    /// `;`, and the line break.
    pub(crate) fn scan_intent_link_item(&mut self) {
        match self.peek_tok() {
            Some(token) if token_is_name(token.clone()) => {
                self.advance(); // the `element`/`check` keyword
                self.take_ref_name(); // the kind word (a keyword token names it too: `class`)
                if matches!(self.peek_tok(), Some(Token::Other(':'))) {
                    self.bump_token();
                    self.push_text(":", true);
                    self.glue_next = true;
                    self.scan_qname();
                }
                if !self.take_if(|token| matches!(token, Token::Other(';'))) {
                    self.push_text(";", true);
                }
                self.flush_line();
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
                        token_is_name(token.clone()) || matches!(token, Token::RBrace)
                    });
                }
            }
        }
    }
}

/// `true` for the tokens a link line can begin with: an identifier (the
/// `element`/`check` contextual words) or — the kind-word discipline is
/// name-or-keyword — a keyword token.
fn token_is_name(token: Token<'_>) -> bool {
    matches!(token, Token::Ident(_) | Token::IdentEscaped(_)) || token.keyword().is_some()
}
