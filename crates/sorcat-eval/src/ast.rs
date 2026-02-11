use crate::EvalError;
use std::collections::{BTreeMap, BTreeSet};
use syn::{
    BinOp, Expr, GenericArgument, Item, Lit, Pat, ReturnType, Stmt, Type, UnOp, visit::Visit,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizationOptions {
    pub canonicalize_identifiers: bool,
    pub normalize_whitespace: bool,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            canonicalize_identifiers: true,
            normalize_whitespace: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedAst {
    pub canonical_source: String,
    pub node_count: usize,
}

pub fn normalize_original_rust(
    source: &str,
    options: &NormalizationOptions,
) -> Result<NormalizedAst, EvalError> {
    normalize_source(source, options, "original_source")
}

pub fn normalize_reconstructed_rust(
    source: &str,
    options: &NormalizationOptions,
) -> Result<NormalizedAst, EvalError> {
    normalize_source(source, options, "reconstructed_source")
}

fn normalize_source(
    source: &str,
    options: &NormalizationOptions,
    field: &'static str,
) -> Result<NormalizedAst, EvalError> {
    if source.trim().is_empty() {
        return Err(EvalError::InvalidInput {
            field,
            message: "source cannot be empty".to_string(),
        });
    }

    let mut tokens = tokenize_rust_like(source)?;
    if tokens.is_empty() {
        return Err(EvalError::InvalidInput {
            field,
            message: "source did not contain any tokens".to_string(),
        });
    }

    if options.canonicalize_identifiers {
        canonicalize_identifiers(&mut tokens);
    }

    let canonical_source = if options.normalize_whitespace {
        render_tokens(&tokens)
    } else {
        tokens.join(" ")
    };

    if canonical_source.trim().is_empty() {
        return Err(EvalError::InvalidInput {
            field,
            message: "normalized source became empty".to_string(),
        });
    }

    let ast_tree = parse_rust_ast_to_tree(&canonical_source, field, "normalized source")?;

    Ok(NormalizedAst {
        canonical_source,
        node_count: ast_tree.node_count(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AstTree {
    labels: Vec<String>,
    children: Vec<Vec<usize>>,
    root: usize,
}

impl AstTree {
    pub(crate) fn node_count(&self) -> usize {
        self.labels.len()
    }

    pub(crate) fn root(&self) -> usize {
        self.root
    }

    pub(crate) fn label(&self, node_id: usize) -> &str {
        &self.labels[node_id]
    }

    pub(crate) fn children(&self, node_id: usize) -> &[usize] {
        &self.children[node_id]
    }
}

pub(crate) fn parse_rust_ast_to_tree(
    source: &str,
    field: &'static str,
    context: &'static str,
) -> Result<AstTree, EvalError> {
    let parsed = syn::parse_file(source).map_err(|error| EvalError::InvalidInput {
        field,
        message: format!("{context} is not valid Rust syntax: {error}"),
    })?;

    let mut visitor = AstTreeVisitor::default();
    visitor.visit_file(&parsed);
    visitor
        .finish()
        .map_err(|message| EvalError::InvalidInput { field, message })
}

#[derive(Debug, Default)]
struct AstTreeBuilder {
    labels: Vec<String>,
    children: Vec<Vec<usize>>,
    stack: Vec<usize>,
    root: Option<usize>,
}

impl AstTreeBuilder {
    fn enter_node(&mut self, label: String) -> usize {
        let node_id = self.labels.len();
        self.labels.push(label);
        self.children.push(Vec::new());

        if let Some(&parent_id) = self.stack.last() {
            self.children[parent_id].push(node_id);
        } else if self.root.is_none() {
            self.root = Some(node_id);
        }

        self.stack.push(node_id);
        node_id
    }

    fn exit_node(&mut self, node_id: usize) {
        let popped = self.stack.pop();
        debug_assert_eq!(popped, Some(node_id));
    }

    fn build(self) -> Result<AstTree, String> {
        if !self.stack.is_empty() {
            return Err("internal AST builder stack must be empty after traversal".to_string());
        }

        let root = self
            .root
            .ok_or_else(|| "parsed AST did not produce any nodes".to_string())?;

        Ok(AstTree {
            labels: self.labels,
            children: self.children,
            root,
        })
    }
}

#[derive(Debug, Default)]
struct AstTreeVisitor {
    builder: AstTreeBuilder,
}

impl AstTreeVisitor {
    fn begin_node(&mut self, label: String) -> usize {
        self.builder.enter_node(label)
    }

    fn end_node(&mut self, node_id: usize) {
        self.builder.exit_node(node_id);
    }

    fn finish(self) -> Result<AstTree, String> {
        self.builder.build()
    }
}

impl<'ast> Visit<'ast> for AstTreeVisitor {
    fn visit_file(&mut self, node: &'ast syn::File) {
        let node_id = self.begin_node("File".to_string());
        syn::visit::visit_file(self, node);
        self.end_node(node_id);
    }

    fn visit_item(&mut self, node: &'ast Item) {
        let node_id = self.begin_node(item_label(node).to_string());
        syn::visit::visit_item(self, node);
        self.end_node(node_id);
    }

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        let node_id = self.begin_node(stmt_label(node).to_string());
        syn::visit::visit_stmt(self, node);
        self.end_node(node_id);
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        let node_id = self.begin_node(expr_label(node).to_string());
        syn::visit::visit_expr(self, node);
        self.end_node(node_id);
    }

    fn visit_type(&mut self, node: &'ast Type) {
        let node_id = self.begin_node(type_label(node).to_string());
        syn::visit::visit_type(self, node);
        self.end_node(node_id);
    }

    fn visit_pat(&mut self, node: &'ast Pat) {
        let node_id = self.begin_node(pat_label(node).to_string());
        syn::visit::visit_pat(self, node);
        self.end_node(node_id);
    }

    fn visit_return_type(&mut self, node: &'ast ReturnType) {
        let node_id = self.begin_node(return_type_label(node).to_string());
        syn::visit::visit_return_type(self, node);
        self.end_node(node_id);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        let node_id = self.begin_node("Path".to_string());
        syn::visit::visit_path(self, node);
        self.end_node(node_id);
    }

    fn visit_path_segment(&mut self, node: &'ast syn::PathSegment) {
        let node_id = self.begin_node(format!("PathSegment:{}", node.ident));
        syn::visit::visit_path_segment(self, node);
        self.end_node(node_id);
    }

    fn visit_generic_argument(&mut self, node: &'ast GenericArgument) {
        let node_id = self.begin_node(generic_argument_label(node).to_string());
        syn::visit::visit_generic_argument(self, node);
        self.end_node(node_id);
    }

    fn visit_ident(&mut self, node: &'ast syn::Ident) {
        let node_id = self.begin_node(format!("Ident:{node}"));
        syn::visit::visit_ident(self, node);
        self.end_node(node_id);
    }

    fn visit_lifetime(&mut self, node: &'ast syn::Lifetime) {
        let node_id = self.begin_node(format!("Lifetime:'{}", node.ident));
        syn::visit::visit_lifetime(self, node);
        self.end_node(node_id);
    }

    fn visit_lit(&mut self, node: &'ast Lit) {
        let node_id = self.begin_node(lit_label(node));
        syn::visit::visit_lit(self, node);
        self.end_node(node_id);
    }

    fn visit_bin_op(&mut self, node: &'ast BinOp) {
        let node_id = self.begin_node(bin_op_label(node).to_string());
        syn::visit::visit_bin_op(self, node);
        self.end_node(node_id);
    }

    fn visit_un_op(&mut self, node: &'ast UnOp) {
        let node_id = self.begin_node(un_op_label(node).to_string());
        syn::visit::visit_un_op(self, node);
        self.end_node(node_id);
    }
}

fn item_label(item: &Item) -> &'static str {
    match item {
        Item::Const(_) => "Item::Const",
        Item::Enum(_) => "Item::Enum",
        Item::ExternCrate(_) => "Item::ExternCrate",
        Item::Fn(_) => "Item::Fn",
        Item::ForeignMod(_) => "Item::ForeignMod",
        Item::Impl(_) => "Item::Impl",
        Item::Macro(_) => "Item::Macro",
        Item::Mod(_) => "Item::Mod",
        Item::Static(_) => "Item::Static",
        Item::Struct(_) => "Item::Struct",
        Item::Trait(_) => "Item::Trait",
        Item::TraitAlias(_) => "Item::TraitAlias",
        Item::Type(_) => "Item::Type",
        Item::Union(_) => "Item::Union",
        Item::Use(_) => "Item::Use",
        Item::Verbatim(_) => "Item::Verbatim",
        _ => "Item::Other",
    }
}

fn stmt_label(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::Local(_) => "Stmt::Local",
        Stmt::Item(_) => "Stmt::Item",
        Stmt::Expr(_, _) => "Stmt::Expr",
        Stmt::Macro(_) => "Stmt::Macro",
    }
}

fn expr_label(expr: &Expr) -> &'static str {
    match expr {
        Expr::Array(_) => "Expr::Array",
        Expr::Assign(_) => "Expr::Assign",
        Expr::Async(_) => "Expr::Async",
        Expr::Await(_) => "Expr::Await",
        Expr::Binary(_) => "Expr::Binary",
        Expr::Block(_) => "Expr::Block",
        Expr::Break(_) => "Expr::Break",
        Expr::Call(_) => "Expr::Call",
        Expr::Cast(_) => "Expr::Cast",
        Expr::Closure(_) => "Expr::Closure",
        Expr::Const(_) => "Expr::Const",
        Expr::Continue(_) => "Expr::Continue",
        Expr::Field(_) => "Expr::Field",
        Expr::ForLoop(_) => "Expr::ForLoop",
        Expr::Group(_) => "Expr::Group",
        Expr::If(_) => "Expr::If",
        Expr::Index(_) => "Expr::Index",
        Expr::Infer(_) => "Expr::Infer",
        Expr::Let(_) => "Expr::Let",
        Expr::Lit(_) => "Expr::Lit",
        Expr::Loop(_) => "Expr::Loop",
        Expr::Macro(_) => "Expr::Macro",
        Expr::Match(_) => "Expr::Match",
        Expr::MethodCall(_) => "Expr::MethodCall",
        Expr::Paren(_) => "Expr::Paren",
        Expr::Path(_) => "Expr::Path",
        Expr::Range(_) => "Expr::Range",
        Expr::Reference(_) => "Expr::Reference",
        Expr::Repeat(_) => "Expr::Repeat",
        Expr::Return(_) => "Expr::Return",
        Expr::Struct(_) => "Expr::Struct",
        Expr::Try(_) => "Expr::Try",
        Expr::TryBlock(_) => "Expr::TryBlock",
        Expr::Tuple(_) => "Expr::Tuple",
        Expr::Unary(_) => "Expr::Unary",
        Expr::Unsafe(_) => "Expr::Unsafe",
        Expr::Verbatim(_) => "Expr::Verbatim",
        Expr::While(_) => "Expr::While",
        Expr::Yield(_) => "Expr::Yield",
        _ => "Expr::Other",
    }
}

fn type_label(ty: &Type) -> &'static str {
    match ty {
        Type::Array(_) => "Type::Array",
        Type::BareFn(_) => "Type::BareFn",
        Type::Group(_) => "Type::Group",
        Type::ImplTrait(_) => "Type::ImplTrait",
        Type::Infer(_) => "Type::Infer",
        Type::Macro(_) => "Type::Macro",
        Type::Never(_) => "Type::Never",
        Type::Paren(_) => "Type::Paren",
        Type::Path(_) => "Type::Path",
        Type::Ptr(_) => "Type::Ptr",
        Type::Reference(_) => "Type::Reference",
        Type::Slice(_) => "Type::Slice",
        Type::TraitObject(_) => "Type::TraitObject",
        Type::Tuple(_) => "Type::Tuple",
        Type::Verbatim(_) => "Type::Verbatim",
        _ => "Type::Other",
    }
}

fn pat_label(pat: &Pat) -> &'static str {
    match pat {
        Pat::Const(_) => "Pat::Const",
        Pat::Ident(_) => "Pat::Ident",
        Pat::Lit(_) => "Pat::Lit",
        Pat::Macro(_) => "Pat::Macro",
        Pat::Or(_) => "Pat::Or",
        Pat::Paren(_) => "Pat::Paren",
        Pat::Path(_) => "Pat::Path",
        Pat::Range(_) => "Pat::Range",
        Pat::Reference(_) => "Pat::Reference",
        Pat::Rest(_) => "Pat::Rest",
        Pat::Slice(_) => "Pat::Slice",
        Pat::Struct(_) => "Pat::Struct",
        Pat::Tuple(_) => "Pat::Tuple",
        Pat::TupleStruct(_) => "Pat::TupleStruct",
        Pat::Type(_) => "Pat::Type",
        Pat::Verbatim(_) => "Pat::Verbatim",
        Pat::Wild(_) => "Pat::Wild",
        _ => "Pat::Other",
    }
}

fn return_type_label(return_type: &ReturnType) -> &'static str {
    match return_type {
        ReturnType::Default => "ReturnType::Default",
        ReturnType::Type(_, _) => "ReturnType::Type",
    }
}

fn generic_argument_label(arg: &GenericArgument) -> &'static str {
    match arg {
        GenericArgument::Lifetime(_) => "GenericArgument::Lifetime",
        GenericArgument::Type(_) => "GenericArgument::Type",
        GenericArgument::Const(_) => "GenericArgument::Const",
        GenericArgument::AssocType(_) => "GenericArgument::AssocType",
        GenericArgument::AssocConst(_) => "GenericArgument::AssocConst",
        GenericArgument::Constraint(_) => "GenericArgument::Constraint",
        _ => "GenericArgument::Other",
    }
}

fn lit_label(lit: &Lit) -> String {
    match lit {
        Lit::Str(value) => format!("Lit::Str:{}", value.value()),
        Lit::ByteStr(value) => format!("Lit::ByteStr:len={}", value.value().len()),
        Lit::Byte(value) => format!("Lit::Byte:{}", value.value()),
        Lit::Char(value) => format!("Lit::Char:{}", value.value()),
        Lit::Int(value) => format!("Lit::Int:{}", value.base10_digits()),
        Lit::Float(value) => format!("Lit::Float:{}", value.base10_digits()),
        Lit::Bool(value) => format!("Lit::Bool:{}", value.value()),
        Lit::Verbatim(_) => "Lit::Verbatim".to_string(),
        _ => "Lit::Other".to_string(),
    }
}

fn bin_op_label(bin_op: &BinOp) -> &'static str {
    match bin_op {
        BinOp::Add(_) => "BinOp::Add",
        BinOp::Sub(_) => "BinOp::Sub",
        BinOp::Mul(_) => "BinOp::Mul",
        BinOp::Div(_) => "BinOp::Div",
        BinOp::Rem(_) => "BinOp::Rem",
        BinOp::And(_) => "BinOp::And",
        BinOp::Or(_) => "BinOp::Or",
        BinOp::BitXor(_) => "BinOp::BitXor",
        BinOp::BitAnd(_) => "BinOp::BitAnd",
        BinOp::BitOr(_) => "BinOp::BitOr",
        BinOp::Shl(_) => "BinOp::Shl",
        BinOp::Shr(_) => "BinOp::Shr",
        BinOp::Eq(_) => "BinOp::Eq",
        BinOp::Lt(_) => "BinOp::Lt",
        BinOp::Le(_) => "BinOp::Le",
        BinOp::Ne(_) => "BinOp::Ne",
        BinOp::Ge(_) => "BinOp::Ge",
        BinOp::Gt(_) => "BinOp::Gt",
        BinOp::AddAssign(_) => "BinOp::AddAssign",
        BinOp::SubAssign(_) => "BinOp::SubAssign",
        BinOp::MulAssign(_) => "BinOp::MulAssign",
        BinOp::DivAssign(_) => "BinOp::DivAssign",
        BinOp::RemAssign(_) => "BinOp::RemAssign",
        BinOp::BitXorAssign(_) => "BinOp::BitXorAssign",
        BinOp::BitAndAssign(_) => "BinOp::BitAndAssign",
        BinOp::BitOrAssign(_) => "BinOp::BitOrAssign",
        BinOp::ShlAssign(_) => "BinOp::ShlAssign",
        BinOp::ShrAssign(_) => "BinOp::ShrAssign",
        _ => "BinOp::Other",
    }
}

fn un_op_label(un_op: &UnOp) -> &'static str {
    match un_op {
        UnOp::Deref(_) => "UnOp::Deref",
        UnOp::Not(_) => "UnOp::Not",
        UnOp::Neg(_) => "UnOp::Neg",
        _ => "UnOp::Other",
    }
}

pub(crate) fn tokenize_rust_like(source: &str) -> Result<Vec<String>, EvalError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut idx = 0usize;

    while idx < chars.len() {
        let current = chars[idx];

        if current.is_whitespace() {
            idx += 1;
            continue;
        }

        if current == '/' && idx + 1 < chars.len() {
            let next = chars[idx + 1];
            if next == '/' {
                idx += 2;
                while idx < chars.len() && chars[idx] != '\n' {
                    idx += 1;
                }
                continue;
            }
            if next == '*' {
                idx += 2;
                let mut depth = 1usize;
                while idx < chars.len() && depth > 0 {
                    if idx + 1 < chars.len() && chars[idx] == '/' && chars[idx + 1] == '*' {
                        depth += 1;
                        idx += 2;
                        continue;
                    }
                    if idx + 1 < chars.len() && chars[idx] == '*' && chars[idx + 1] == '/' {
                        depth -= 1;
                        idx += 2;
                        continue;
                    }
                    idx += 1;
                }
                if depth != 0 {
                    return Err(EvalError::InvalidInput {
                        field: "source",
                        message: "unterminated block comment".to_string(),
                    });
                }
                continue;
            }
        }

        if is_identifier_start(current) {
            let start = idx;
            idx += 1;
            while idx < chars.len() && is_identifier_part(chars[idx]) {
                idx += 1;
            }
            tokens.push(chars[start..idx].iter().collect());
            continue;
        }

        if current.is_ascii_digit() {
            let start = idx;
            idx += 1;
            while idx < chars.len() && (chars[idx].is_ascii_alphanumeric() || chars[idx] == '_') {
                idx += 1;
            }
            tokens.push(chars[start..idx].iter().collect());
            continue;
        }

        if current == '"' || current == '\'' {
            if current == '\'' && idx + 1 < chars.len() && is_identifier_start(chars[idx + 1]) {
                // Rust lifetime token, e.g. `'a` or `'static`.
                let start = idx;
                idx += 1;
                while idx < chars.len() && is_identifier_part(chars[idx]) {
                    idx += 1;
                }
                tokens.push(chars[start..idx].iter().collect());
                continue;
            }

            let quote = current;
            let start = idx;
            idx += 1;
            let mut closed = false;
            while idx < chars.len() {
                let c = chars[idx];
                if c == '\\' {
                    idx += 2;
                    continue;
                }
                if c == quote {
                    idx += 1;
                    closed = true;
                    break;
                }
                idx += 1;
            }
            if !closed {
                return Err(EvalError::InvalidInput {
                    field: "source",
                    message: "unterminated string or character literal".to_string(),
                });
            }
            tokens.push(chars[start..idx].iter().collect());
            continue;
        }

        if idx + 1 < chars.len() {
            let pair = (current, chars[idx + 1]);
            let pair_token = match pair {
                ('-', '>') => Some("->"),
                (':', ':') => Some("::"),
                ('=', '=') => Some("=="),
                ('!', '=') => Some("!="),
                ('>', '=') => Some(">="),
                ('<', '=') => Some("<="),
                ('&', '&') => Some("&&"),
                ('|', '|') => Some("||"),
                ('=', '>') => Some("=>"),
                ('+', '=') => Some("+="),
                ('-', '=') => Some("-="),
                ('*', '=') => Some("*="),
                ('/', '=') => Some("/="),
                _ => None,
            };
            if let Some(token) = pair_token {
                tokens.push(token.to_string());
                idx += 2;
                continue;
            }
        }

        tokens.push(current.to_string());
        idx += 1;
    }

    Ok(tokens)
}

fn canonicalize_identifiers(tokens: &mut [String]) {
    let keywords = rust_keywords();
    let mut mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut next_id = 0usize;

    for token in tokens.iter_mut() {
        if !is_identifier(token) || keywords.contains(token.as_str()) || is_builtin_type(token) {
            continue;
        }

        let replacement = mapping.entry(token.clone()).or_insert_with(|| {
            let canonical = format!("id{next_id}");
            next_id += 1;
            canonical
        });

        *token = replacement.clone();
    }
}

fn render_tokens(tokens: &[String]) -> String {
    let mut output = String::new();
    let mut previous: Option<&str> = None;

    for token in tokens {
        let token_str = token.as_str();
        if let Some(prev) = previous {
            if should_insert_space(prev, token_str) {
                output.push(' ');
            }
        }
        output.push_str(token_str);
        previous = Some(token_str);
    }

    output
}

fn should_insert_space(previous: &str, next: &str) -> bool {
    let no_space_before = [
        ",", ";", ")", "]", "}", ":", ".", "!", "?", "->", "::", "=>",
    ];
    if no_space_before.contains(&next) {
        return false;
    }

    let no_space_after = ["(", "[", "{", "::", ".", "!", "?", "->", "=>"];
    if no_space_after.contains(&previous) {
        return false;
    }

    if next == "(" {
        return false;
    }

    true
}

fn rust_keywords() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ])
}

fn is_builtin_type(token: &str) -> bool {
    matches!(
        token,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "isize"
            | "usize"
            | "f32"
            | "f64"
            | "bool"
            | "char"
            | "str"
    )
}

fn is_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    match chars.next() {
        Some(first) if is_identifier_start(first) => chars.all(is_identifier_part),
        _ => false,
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_part(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
