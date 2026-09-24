use crate::languages::VisibilityRule;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator, Tree};

/// Signatures are collapsed to one line; anything longer than this is
/// truncated so a single generic-heavy declaration cannot dominate the map.
const MAX_SIGNATURE_LEN: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct Symbol {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub line: usize,
    pub end_line: usize,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub exported: bool,
    /// Declared inside a function or block rather than at file scope.
    pub local: bool,
}

impl Symbol {
    /// `Parent > name` where a parent is known, with Markdown headings
    /// indented by their level instead.
    pub fn display_name(&self) -> String {
        match &self.parent {
            Some(p) => format!("{} > {}", p, self.name),
            None => match self.heading_level() {
                Some(level) => {
                    format!("{}{}", "  ".repeat(level.saturating_sub(1)), self.name)
                }
                None => self.name.clone(),
            },
        }
    }

    fn heading_level(&self) -> Option<usize> {
        let rest = self.kind.strip_prefix('h')?;
        let level: usize = rest.parse().ok()?;
        (1..=6).contains(&level).then_some(level)
    }

    /// The label shown in the map: the breadcrumb, with the signature
    /// substituted for the bare name when one was extracted.
    pub fn label(&self, with_signature: bool) -> String {
        let Some(signature) = self.signature.as_deref().filter(|_| with_signature) else {
            return self.display_name();
        };

        match &self.parent {
            Some(parent) => format!("{} > {}", parent, signature),
            None => signature.to_string(),
        }
    }

    pub fn span(&self) -> usize {
        self.end_line.saturating_sub(self.line) + 1
    }
}

/// Parses `source` once so the resulting tree can be reused across
/// both symbol and import extraction, instead of parsing twice per file.
pub fn parse_source(source: &str, lang: &tree_sitter::Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(lang).ok()?;
    parser.parse(source, None)
}

/// A symbol as first seen, before matches for the same name node are merged.
struct Partial {
    name: String,
    parent: Option<String>,
    line: usize,
    end_line: usize,
    kind: String,
    signature: Option<String>,
    exported: bool,
    local: bool,
}

pub fn extract_symbols(
    source: &str,
    tree: &Tree,
    lang: &tree_sitter::Language,
    query_str: &str,
    visibility: VisibilityRule,
) -> Vec<Symbol> {
    let query = match Query::new(lang, query_str) {
        Ok(q) => q,
        Err(_) => return Vec::new(),
    };
    let mut cursor = QueryCursor::new();
    let source_bytes = source.as_bytes();

    // Keyed by the name node's byte offset so that several query patterns
    // matching the same declaration (for example a bare method and the same
    // method seen through its class, or through a decorator) collapse into one
    // symbol regardless of the order tree-sitter yields them in.
    let mut by_name_offset: HashMap<usize, Partial> = HashMap::new();
    let mut order: Vec<usize> = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut name_offset = None;
        let mut parent = None;
        // A `@scope` parent is a namespace rather than a receiver: it earns a
        // breadcrumb but does not make the symbol a method.
        let mut is_receiver = false;
        let mut item: Option<Node> = None;

        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            let node = capture.node;

            match capture_name {
                "name" => {
                    if let Some(n) = node_text(source, &node) {
                        name = n.trim().to_string();
                        name_offset = Some(node.start_byte());
                    }
                }
                "parent" | "scope" => {
                    is_receiver |= capture_name == "parent";
                    if let Some(p) = node_text(source, &node) {
                        // A Go method receiver arrives as `*Impl`; the pointer
                        // is noise in a breadcrumb.
                        let cleaned = p.trim().trim_start_matches(['*', '&']).trim();
                        if !cleaned.is_empty() {
                            parent = Some(cleaned.to_string());
                        }
                    }
                }
                "item" => item = Some(node),
                _ => {}
            }
        }

        let Some(item) = item else { continue };
        let mut heading_end_line = None;
        let mut kind = friendly_kind(source, &item, is_receiver);
        let mut signature = signature_of(source, &item);
        let exported = is_exported(source, &item, &name, visibility);
        let local = is_local_declaration(&item);

        // A loop counter is not a symbol anybody is looking for.
        if local && is_loop_binding(&item) {
            continue;
        }

        // A local binding floats without context, so name the function it
        // lives in. This is a scope, not a receiver, so the kind is unchanged.
        if local && parent.is_none() {
            parent = enclosing_scope_name(source, &item);
        }

        // Markdown headings carry their level in the node text rather than in
        // a named field, so name and kind are derived from the raw text.
        if item.kind() == "atx_heading"
            && let Some(raw) = node_text(source, &item)
        {
            let level = raw.chars().take_while(|&c| c == '#').count();
            kind = format!("h{}", level.clamp(1, 6));
            name = raw.trim_start_matches('#').trim().to_string();
            name_offset = Some(item.start_byte());
            signature = None;
            heading_end_line = Some(item.start_position().row + 1);
        }

        if name.is_empty() {
            continue;
        }
        let Some(offset) = name_offset else { continue };

        let partial = Partial {
            name,
            parent,
            line: item.start_position().row + 1,
            end_line: heading_end_line.unwrap_or(item.end_position().row + 1),
            kind,
            signature,
            exported,
            local,
        };

        match by_name_offset.get_mut(&offset) {
            // A later match may be the one that knows the enclosing type, so
            // keep whichever information is richer rather than first-wins.
            Some(existing) => {
                if existing.parent.is_none() && partial.parent.is_some() {
                    existing.parent = partial.parent;
                    existing.kind = partial.kind;
                    existing.line = partial.line;
                    existing.end_line = partial.end_line;
                }
                if existing.signature.is_none() {
                    existing.signature = partial.signature;
                }
                existing.exported |= partial.exported;
                existing.local |= partial.local;
            }
            None => {
                order.push(offset);
                by_name_offset.insert(offset, partial);
            }
        }
    }

    let mut symbols: Vec<Symbol> = order
        .iter()
        .filter_map(|offset| by_name_offset.remove(offset))
        .map(|p| Symbol {
            name: p.name,
            parent: p.parent,
            line: p.line,
            end_line: p.end_line,
            kind: p.kind,
            signature: p.signature,
            exported: p.exported,
            local: p.local,
        })
        .collect();

    // On a one-line declaration the container and its members share a line;
    // list the container first.
    symbols.sort_by_key(|s| (s.line, s.parent.is_some(), s.name.clone()));

    // A local name can be bound more than once in the same scope: Python
    // reassigns and Rust shadows. Report where it first appears.
    let mut seen: HashSet<(Option<String>, String)> = HashSet::new();
    symbols.retain(|symbol| {
        !symbol.local || seen.insert((symbol.parent.clone(), symbol.name.clone()))
    });

    symbols
}

fn node_text<'a>(source: &'a str, node: &Node) -> Option<&'a str> {
    source.get(node.start_byte()..node.end_byte())
}

/// The declaration text up to (but not including) its body, collapsed onto one
/// line. `fn foo(a: u32) -> String { .. }` becomes `fn foo(a: u32) -> String`.
///
/// The body is looked for on the item itself and then on its descendants, so
/// that wrapper nodes such as a `const x = () => { .. }` declaration still cut
/// at the arrow function's body.
fn signature_of(source: &str, item: &Node) -> Option<String> {
    // A declarator carries the name and value but not the `const`/`let`
    // keyword, which sits on the statement above it. Taking the statement's
    // whole text instead would repeat every declarator in `let a, b`.
    let keyword = declaration_keyword(source, item);

    // `export` wraps the declaration rather than prefixing it, but it belongs
    // in the signature.
    let start = match item.parent() {
        Some(parent) if parent.kind() == "export_statement" => parent.start_byte(),
        _ => item.start_byte(),
    };
    let end = match find_body_start(item) {
        Some(body_start) if body_start > start => body_start,
        // No body to cut at (a constant, a type alias): keep the first line.
        // Measured from `start`, which may sit before the item itself.
        _ => {
            let text = source.get(start..item.end_byte())?;
            start + text.find('\n').unwrap_or(text.len())
        }
    };

    let raw = source.get(start..end)?;
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    // A fat arrow is the boundary of an arrow function, not part of its shape.
    let without_arrow = collapsed
        .trim_end()
        .strip_suffix("=>")
        .unwrap_or(&collapsed);
    let trimmed = without_arrow
        .trim_end_matches(|c: char| {
            c == '{' || c == '=' || c == ';' || c == ':' || c.is_whitespace()
        })
        .trim();

    if trimmed.is_empty() {
        return None;
    }

    let trimmed = match keyword {
        Some(keyword) => format!("{keyword} {trimmed}"),
        None => trimmed.to_string(),
    };
    let trimmed = trimmed.as_str();

    if trimmed.len() > MAX_SIGNATURE_LEN {
        let cut = trimmed
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|i| *i <= MAX_SIGNATURE_LEN)
            .last()
            .unwrap_or(0);
        return Some(format!("{}...", &trimmed[..cut]));
    }

    Some(trimmed.to_string())
}

/// The `export const` / `let` / `var` prefix for a declarator, taken from the
/// statement that owns it.
fn declaration_keyword(source: &str, item: &Node) -> Option<String> {
    if item.kind() != "variable_declarator" {
        return None;
    }

    let statement = item
        .parent()
        .filter(|parent| BINDING_KINDS.contains(&parent.kind()))?;

    let mut cursor = statement.walk();
    let keyword = statement
        .children(&mut cursor)
        .find(|child| !child.is_named())
        .and_then(|child| node_text(source, &child))?
        .to_string();

    let exported = matches!(
        statement.parent().map(|parent| parent.kind()),
        Some("export_statement")
    );

    Some(if exported {
        format!("export {keyword}")
    } else {
        keyword
    })
}

/// The leading keyword token of a declaration, where it names the kind more
/// precisely than the node does.
fn keyword_kind(source: &str, item: &Node) -> Option<&'static str> {
    let mut cursor = item.walk();
    let children: Vec<Node> = item.children(&mut cursor).collect();

    // Only the tokens before the name can be declaration keywords.
    children
        .iter()
        .take(6)
        .filter(|child| !child.is_named())
        .find_map(|child| {
            let text = node_text(source, child)?;
            KEYWORD_KINDS
                .iter()
                .find(|(keyword, _)| *keyword == text)
                .map(|(_, kind)| *kind)
        })
}

/// Whether the declaration declares a function, as C and C++ do by wrapping a
/// `function_declarator` in an otherwise generic declaration node.
fn has_function_declarator(item: &Node) -> bool {
    if !matches!(item.kind(), "declaration" | "field_declaration") {
        return false;
    }

    item.child_by_field_name("declarator")
        .is_some_and(|d| d.kind() == "function_declarator")
}

/// Whether this declaration's value is a function, searched shallowly because
/// an arrow function sits just below the declarator.
fn holds_function(node: &Node) -> bool {
    fn search(node: &Node, depth: usize) -> bool {
        if matches!(
            node.kind(),
            "arrow_function" | "function_expression" | "closure_expression" | "lambda"
        ) {
            return true;
        }
        if depth == 0 {
            return false;
        }
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .any(|child| search(&child, depth - 1))
    }

    search(node, 3)
}

/// Node kinds that are a declaration's body in grammars that do not expose it
/// under a `body` field.
const BODY_KINDS: &[&str] = &[
    "function_body",
    "class_body",
    "enum_class_body",
    "body_statement",
    "declaration_list",
    "field_declaration_list",
    "enum_variant_list",
    "statement_block",
    "compound_statement",
    "block",
];

fn find_body_start(node: &Node) -> Option<usize> {
    if let Some(body) = node.child_by_field_name("body") {
        return Some(body.start_byte());
    }

    let mut cursor = node.walk();
    let children: Vec<Node> = node.named_children(&mut cursor).collect();

    if let Some(body) = children
        .iter()
        .find(|child| BODY_KINDS.contains(&child.kind()))
    {
        return Some(body.start_byte());
    }

    children.iter().find_map(find_body_start)
}

/// Whether the declaration is part of the file's public surface, according to
/// the convention the language actually uses.
fn is_exported(source: &str, item: &Node, name: &str, rule: VisibilityRule) -> bool {
    match rule {
        VisibilityRule::Keyword(keywords) => {
            has_leading_keyword(source, item, keywords) || is_wrapped_in_export(item)
        }
        VisibilityRule::UnlessKeyword(keywords) => !has_leading_keyword(source, item, keywords),
        VisibilityRule::Underscore => !name.starts_with('_'),
        VisibilityRule::Capitalized => name.chars().next().is_some_and(char::is_uppercase),
    }
}

fn has_leading_keyword(source: &str, item: &Node, keywords: &[&str]) -> bool {
    let Some(text) = node_text(source, item) else {
        return false;
    };
    let head = text.trim_start();

    keywords.iter().any(|keyword| {
        head.strip_prefix(keyword).is_some_and(|rest| {
            // `pub` matches `pub fn` and `pub(crate) fn`, but not `public_id`.
            rest.chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '(' || c == ':')
        })
    })
}

/// `export const x = ...` and `export default ...` wrap the declaration rather
/// than prefixing it, so the keyword sits on an ancestor node.
fn is_wrapped_in_export(item: &Node) -> bool {
    let mut current = item.parent();
    for _ in 0..2 {
        let Some(node) = current else { return false };
        if node.kind().starts_with("export") {
            return true;
        }
        current = node.parent();
    }
    false
}

/// Grammar nodes that bind a value to a plain name. Their kind is decided by
/// what they hold, so a name bound to a closure reads as a function.
const BINDING_KINDS: &[&str] = &[
    "lexical_declaration",
    "variable_declaration",
    "variable_declarator",
    "let_declaration",
    "assignment",
];

/// Bindings that can be local, which adds the declarations that keep their own
/// kind when they appear at file scope: a `const` inside a function body is a
/// local, while the same node at the top of the file is part of the surface.
/// Only value bindings are listed, so a nested helper function still shows up
/// under `--no-locals`.
const LOCALIZABLE_KINDS: &[&str] = &[
    "lexical_declaration",
    "variable_declaration",
    "variable_declarator",
    "let_declaration",
    "assignment",
    "const_item",
    "static_item",
    "property_declaration",
];

/// The statement a declarator belongs to, or the node itself when it already
/// is the statement.
fn binding_statement<'a>(item: &Node<'a>) -> Option<Node<'a>> {
    if BINDING_KINDS.contains(&item.kind()) {
        if item.kind() == "variable_declarator" {
            return item.parent();
        }
        return Some(*item);
    }
    None
}

/// Whether a node is a function, a method, or a closure, in any of the
/// supported grammars.
fn is_function_like(kind: &str) -> bool {
    kind.contains("function")
        || kind.contains("lambda")
        || kind.contains("closure")
        || matches!(
            kind,
            "method"
                | "singleton_method"
                | "method_definition"
                | "method_declaration"
                | "constructor_declaration"
        )
}

/// Whether this binding sits inside a function body rather than at file or
/// type scope. Asked structurally, so it holds for `const` in TypeScript,
/// `let` in Rust and a bare assignment in Python alike.
fn is_local_declaration(item: &Node) -> bool {
    if !LOCALIZABLE_KINDS.contains(&item.kind()) {
        return false;
    }

    let mut current = item.parent();
    while let Some(node) = current {
        if is_function_like(node.kind()) {
            return true;
        }
        current = node.parent();
    }

    false
}

/// `for (let i = 0; ...)` binds a counter, not something worth mapping.
fn is_loop_binding(item: &Node) -> bool {
    let Some(statement) = binding_statement(item) else {
        return false;
    };

    matches!(
        statement.parent().map(|p| p.kind()),
        Some("for_statement" | "for_in_statement" | "for_of_statement")
    )
}

/// The name of the nearest enclosing declaration, used as a breadcrumb for a
/// local binding. Nearest wins, so a binding inside a nested closure names the
/// closure rather than the outermost function.
fn enclosing_scope_name(source: &str, item: &Node) -> Option<String> {
    let mut current = item.parent();

    while let Some(node) = current {
        if matches!(node.kind(), "program" | "module" | "source_file") {
            return None;
        }

        for field in ["name", "property"] {
            if let Some(named) = node.child_by_field_name(field)
                && let Some(text) = node_text(source, &named)
                && !text.contains(char::is_whitespace)
            {
                return Some(text.to_string());
            }
        }

        current = node.parent();
    }

    None
}

/// Declaration keywords that name the kind better than the grammar's node
/// does. Swift, for one, parses `struct`, `enum` and `class` all as
/// `class_declaration`.
const KEYWORD_KINDS: &[(&str, &str)] = &[
    ("struct", "struct"),
    ("enum", "enum"),
    ("union", "union"),
    ("class", "class"),
    ("actor", "actor"),
    ("protocol", "protocol"),
    ("interface", "interface"),
    ("trait", "trait"),
    ("object", "object"),
    ("record", "record"),
    ("namespace", "module"),
    ("module", "module"),
    ("extension", "extension"),
    ("typealias", "type"),
];

fn friendly_kind(source: &str, item: &Node, has_parent: bool) -> String {
    let node_kind = item.kind();

    // A prototype or an out-of-line member definition is a function even
    // though C and C++ file both under declaration nodes.
    if has_function_declarator(item) {
        return if has_parent { "method" } else { "fn" }.to_string();
    }

    if let Some(kind) = keyword_kind(source, item) {
        return kind.to_string();
    }

    // A name bound to a closure is a function to every reader, even though the
    // grammar calls it a variable declaration.
    if BINDING_KINDS.contains(&node_kind) {
        return if holds_function(item) {
            if has_parent { "method" } else { "fn" }.to_string()
        } else {
            "var".to_string()
        };
    }

    let kind = match node_kind {
        "atx_heading" => "heading",
        k if k.starts_with("struct") => "struct",
        k if k.starts_with("union") => "union",
        k if k.starts_with("enum") => "enum",
        k if k.starts_with("trait") => "trait",
        k if k.starts_with("record") => "record",
        k if k.starts_with("protocol") && !k.contains("function") => "protocol",
        k if k.starts_with("interface") => "interface",
        k if k.starts_with("class") => "class",
        k if k.starts_with("namespace") || k == "mod_item" || k == "module" => "module",
        "object_declaration" => "object",
        "macro_definition" => "macro",
        k if k.starts_with("constructor") => "ctor",
        k if k.contains("method") || k.contains("function") => {
            if has_parent {
                "method"
            } else {
                "fn"
            }
        }
        k if k.contains("field") => "field",
        "singleton_method" => "method",
        k if k.starts_with("const") || k.starts_with("static") => "const",
        k if k.starts_with("type") => "type",
        k if k.starts_with("property") => "property",
        k if k.starts_with("field")
            || k.starts_with("var")
            || k == "assignment"
            || k.contains("variable") =>
        {
            "var"
        }
        "delegate_declaration" => "delegate",
        "annotation_type_declaration" => "annotation",
        other => other,
    };

    kind.to_string()
}

pub fn extract_imports(
    source: &str,
    tree: &Tree,
    lang: &tree_sitter::Language,
    query_str: &str,
) -> Vec<String> {
    let query = match Query::new(lang, query_str) {
        Ok(q) => q,
        Err(_) => return vec![],
    };

    let mut cursor = QueryCursor::new();
    let source_bytes = source.as_bytes();
    let mut imports = Vec::new();

    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let capture_name = query.capture_names()[capture.index as usize];
            if capture_name == "import"
                && let Some(text) = node_text(source, &capture.node)
            {
                let cleaned = text
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim_matches('<')
                    .trim_matches('>')
                    .to_string();
                if !cleaned.is_empty() && !imports.contains(&cleaned) {
                    imports.push(cleaned);
                }
            }
        }
    }

    imports
}
