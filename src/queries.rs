use crate::languages::Language;

/// Tree-sitter queries for one language.
///
/// `symbols` must capture `@item` (the whole declaration) and `@name` (the
/// identifier). It may also capture an enclosing name, as either `@parent` for
/// a type that owns the symbol as a method, or `@scope` for a namespace that
/// merely contains it. `imports` captures `@import`.
pub struct LanguageQueries {
    pub symbols: &'static str,
    pub imports: Option<&'static str>,
}

pub fn queries_for(lang: Language) -> LanguageQueries {
    match lang {
        Language::Rust => LanguageQueries {
            symbols: r#"
                (function_item name: (identifier) @name) @item
                (struct_item name: (type_identifier) @name) @item
                (enum_item name: (type_identifier) @name) @item
                (union_item name: (type_identifier) @name) @item
                (trait_item name: (type_identifier) @name) @item
                (type_item name: (type_identifier) @name) @item
                (const_item name: (identifier) @name) @item
                (static_item name: (identifier) @name) @item
                (mod_item name: (identifier) @name) @item
                (macro_definition name: (identifier) @name) @item
                (impl_item
                   type: (_) @parent
                   body: (declaration_list
                     (function_item name: (identifier) @name) @item))
                (impl_item
                   type: (_) @parent
                   body: (declaration_list
                     (const_item name: (identifier) @name) @item))
                (trait_item
                   name: (type_identifier) @parent
                   body: (declaration_list
                     (function_signature_item name: (identifier) @name) @item))
                (trait_item
                   name: (type_identifier) @parent
                   body: (declaration_list
                     (function_item name: (identifier) @name) @item))
                (let_declaration pattern: (identifier) @name) @item
                (let_declaration
                  pattern: (mut_pattern (identifier) @name)) @item
                (mod_item
                   name: (identifier) @scope
                   body: (declaration_list
                     (function_item name: (identifier) @name) @item))
            "#,
            imports: Some("(use_declaration argument: (_) @import)"),
        },
        Language::Python => LanguageQueries {
            symbols: r#"
                (function_definition name: (identifier) @name) @item
                (class_definition name: (identifier) @name) @item
                (class_definition
                   name: (identifier) @parent
                   body: (block (function_definition name: (identifier) @name) @item))
                (class_definition
                   name: (identifier) @parent
                   body: (block
                     (decorated_definition
                       definition: (function_definition name: (identifier) @name) @item)))
                ((module
                   (expression_statement
                     (assignment left: (identifier) @name) @item))
                 (#match? @name "^[A-Z][A-Z0-9_]*$"))
                (block
                  (expression_statement
                    (assignment left: (identifier) @name) @item))
                (class_definition
                   name: (identifier) @parent
                   body: (block
                     (expression_statement
                       (assignment left: (identifier) @name) @item)))
            "#,
            imports: Some(
                "(import_statement name: (dotted_name) @import)
                 (import_statement name: (aliased_import name: (dotted_name) @import))
                 (import_from_statement module_name: (dotted_name) @import)
                 (import_from_statement module_name: (relative_import) @import)",
            ),
        },
        Language::Go => LanguageQueries {
            symbols: r#"
                (function_declaration name: (identifier) @name) @item
                (type_spec name: (type_identifier) @name) @item
                (type_alias name: (type_identifier) @name) @item
                (method_declaration
                   receiver: (parameter_list (parameter_declaration type: (_) @parent))
                   name: (field_identifier) @name) @item
                (const_spec name: (identifier) @name) @item
                (var_spec name: (identifier) @name) @item
                (type_spec
                   name: (type_identifier) @parent
                   type: (interface_type
                     (method_elem name: (field_identifier) @name) @item))
            "#,
            imports: Some("(import_spec path: (interpreted_string_literal) @import)"),
        },
        Language::Javascript => LanguageQueries {
            symbols: JS_SYMBOLS,
            imports: Some(JS_IMPORTS),
        },
        Language::Typescript | Language::Tsx => LanguageQueries {
            symbols: TS_SYMBOLS,
            imports: Some(JS_IMPORTS),
        },
        Language::Java => LanguageQueries {
            symbols: r#"
                (class_declaration name: (identifier) @name) @item
                (interface_declaration name: (identifier) @name) @item
                (enum_declaration name: (identifier) @name) @item
                (record_declaration name: (identifier) @name) @item
                (annotation_type_declaration name: (identifier) @name) @item
                (class_declaration
                   name: (identifier) @parent
                   body: (class_body (method_declaration name: (identifier) @name) @item))
                (class_declaration
                   name: (identifier) @parent
                   body: (class_body (constructor_declaration name: (identifier) @name) @item))
                (interface_declaration
                   name: (identifier) @parent
                   body: (interface_body (method_declaration name: (identifier) @name) @item))
            "#,
            imports: Some(
                "(import_declaration (scoped_identifier) @import)
                 (import_declaration (identifier) @import)",
            ),
        },
        Language::C => LanguageQueries {
            symbols: C_SYMBOLS,
            imports: Some(C_IMPORTS),
        },
        Language::Cpp => LanguageQueries {
            symbols: r#"
                (function_definition
                   declarator: (function_declarator declarator: (identifier) @name)) @item
                (function_definition
                   declarator: (function_declarator
                     declarator: (qualified_identifier
                       scope: (namespace_identifier) @parent
                       name: (identifier) @name))) @item
                (declaration
                   declarator: (function_declarator declarator: (identifier) @name)) @item
                (struct_specifier name: (type_identifier) @name) @item
                (class_specifier name: (type_identifier) @name) @item
                (enum_specifier name: (type_identifier) @name) @item
                (union_specifier name: (type_identifier) @name) @item
                (namespace_definition name: (namespace_identifier) @name) @item
                (type_definition declarator: (type_identifier) @name) @item
                (class_specifier
                   name: (type_identifier) @parent
                   body: (field_declaration_list
                     (function_definition
                       declarator: (function_declarator
                         declarator: (field_identifier) @name)) @item))
                (class_specifier
                   name: (type_identifier) @parent
                   body: (field_declaration_list
                     (field_declaration
                       declarator: (function_declarator
                         declarator: (field_identifier) @name)) @item))
            "#,
            imports: Some(C_IMPORTS),
        },
        Language::CSharp => LanguageQueries {
            symbols: r#"
                (class_declaration name: (identifier) @name) @item
                (interface_declaration name: (identifier) @name) @item
                (struct_declaration name: (identifier) @name) @item
                (enum_declaration name: (identifier) @name) @item
                (record_declaration name: (identifier) @name) @item
                (delegate_declaration name: (identifier) @name) @item
                (namespace_declaration name: (identifier) @name) @item
                (class_declaration
                   name: (identifier) @parent
                   body: (declaration_list (method_declaration name: (identifier) @name) @item))
                (class_declaration
                   name: (identifier) @parent
                   body: (declaration_list (property_declaration name: (identifier) @name) @item))
                (class_declaration
                   name: (identifier) @parent
                   body: (declaration_list
                     (constructor_declaration name: (identifier) @name) @item))
                (interface_declaration
                   name: (identifier) @parent
                   body: (declaration_list (method_declaration name: (identifier) @name) @item))
            "#,
            imports: Some(
                "(using_directive (qualified_name) @import)
                 (using_directive (identifier) @import)",
            ),
        },
        Language::Ruby => LanguageQueries {
            symbols: r#"
                (class name: (constant) @name) @item
                (module name: (constant) @name) @item
                (method name: (identifier) @name) @item
                (singleton_method name: (identifier) @name) @item
                (class
                   name: (constant) @parent
                   body: (body_statement (method name: (identifier) @name) @item))
                (class
                   name: (constant) @parent
                   body: (body_statement
                     (singleton_method name: (identifier) @name) @item))
                (module
                   name: (constant) @parent
                   body: (body_statement (method name: (identifier) @name) @item))
            "#,
            imports: Some(
                r#"((call
                       method: (identifier) @_m
                       arguments: (argument_list (string (string_content) @import)))
                    (#any-of? @_m "require" "require_relative" "load"))"#,
            ),
        },
        Language::Php => LanguageQueries {
            symbols: r#"
                (class_declaration name: (name) @name) @item
                (interface_declaration name: (name) @name) @item
                (trait_declaration name: (name) @name) @item
                (enum_declaration name: (name) @name) @item
                (function_definition name: (name) @name) @item
                (class_declaration
                   name: (name) @parent
                   body: (declaration_list (method_declaration name: (name) @name) @item))
                (interface_declaration
                   name: (name) @parent
                   body: (declaration_list (method_declaration name: (name) @name) @item))
                (trait_declaration
                   name: (name) @parent
                   body: (declaration_list (method_declaration name: (name) @name) @item))
            "#,
            imports: Some(
                "(namespace_use_declaration
                   (namespace_use_clause (qualified_name) @import))
                 (namespace_use_declaration (namespace_use_clause (name) @import))",
            ),
        },
        Language::Swift => LanguageQueries {
            symbols: r#"
                (class_declaration name: (type_identifier) @name) @item
                (protocol_declaration name: (type_identifier) @name) @item
                (function_declaration name: (simple_identifier) @name) @item
                (typealias_declaration name: (type_identifier) @name) @item
                (class_declaration
                   name: (type_identifier) @parent
                   body: (class_body
                     (function_declaration name: (simple_identifier) @name) @item))
                (protocol_declaration
                   name: (type_identifier) @parent
                   body: (protocol_body
                     (protocol_function_declaration
                       name: (simple_identifier) @name) @item))
            "#,
            imports: Some("(import_declaration (identifier) @import)"),
        },
        Language::Kotlin => LanguageQueries {
            symbols: r#"
                (class_declaration name: (identifier) @name) @item
                (object_declaration name: (identifier) @name) @item
                (function_declaration name: (identifier) @name) @item
                (property_declaration (variable_declaration (identifier) @name)) @item
                (class_declaration
                   name: (identifier) @parent
                   (class_body
                     (function_declaration name: (identifier) @name) @item))
                (class_declaration
                   name: (identifier) @parent
                   (class_body
                     (property_declaration
                       (variable_declaration (identifier) @name)) @item))
                (object_declaration
                   name: (identifier) @parent
                   (class_body
                     (function_declaration name: (identifier) @name) @item))
            "#,
            imports: Some("(import (qualified_identifier) @import)"),
        },
        Language::Markdown => LanguageQueries {
            symbols: "(atx_heading) @item",
            imports: None,
        },
    }
}

/// Shared by JS and TS: arrow-function and function-expression consts are the
/// dominant idiom in modern code, so they are captured alongside `function`
/// and `class` declarations.
///
/// Plain `const` and `let` bindings are captured at any depth, including
/// inside function bodies. The item is the individual declarator, not the
/// whole statement, so `let a, b` reports two symbols rather than one symbol
/// twice. A local binding is reported with the enclosing
/// function as its breadcrumb, and `--no-locals` drops them for callers that
/// only want a file's outward surface.
const JS_SYMBOLS: &str = r#"
    (lexical_declaration
      (variable_declarator name: (identifier) @name) @item)
    (variable_declaration
      (variable_declarator name: (identifier) @name) @item)
    (function_declaration name: (identifier) @name) @item
    (generator_function_declaration name: (identifier) @name) @item
    (class_declaration name: (identifier) @name) @item
    (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: [(arrow_function) (function_expression)])) @item
    (variable_declaration
      (variable_declarator
        name: (identifier) @name
        value: [(arrow_function) (function_expression)])) @item
    (class_declaration
       name: (identifier) @parent
       body: (class_body (method_definition name: (property_identifier) @name) @item))
    (class_declaration
       name: (identifier) @parent
       body: (class_body (field_definition property: (property_identifier) @name) @item))
"#;

const TS_SYMBOLS: &str = r#"
    (lexical_declaration
      (variable_declarator name: (identifier) @name) @item)
    (variable_declaration
      (variable_declarator name: (identifier) @name) @item)
    (function_declaration name: (identifier) @name) @item
    (generator_function_declaration name: (identifier) @name) @item
    (class_declaration name: (type_identifier) @name) @item
    (abstract_class_declaration name: (type_identifier) @name) @item
    (interface_declaration name: (type_identifier) @name) @item
    (type_alias_declaration name: (type_identifier) @name) @item
    (enum_declaration name: (identifier) @name) @item
    (module name: (identifier) @name) @item
    (lexical_declaration
      (variable_declarator
        name: (identifier) @name
        value: [(arrow_function) (function_expression)])) @item
    (variable_declaration
      (variable_declarator
        name: (identifier) @name
        value: [(arrow_function) (function_expression)])) @item
    (class_declaration
       name: (type_identifier) @parent
       body: (class_body (method_definition name: (property_identifier) @name) @item))
    (class_declaration
       name: (type_identifier) @parent
       body: (class_body (public_field_definition name: (property_identifier) @name) @item))
    (abstract_class_declaration
       name: (type_identifier) @parent
       body: (class_body (method_definition name: (property_identifier) @name) @item))
    (interface_declaration
       name: (type_identifier) @parent
       body: (interface_body
         (method_signature name: (property_identifier) @name) @item))
"#;

const JS_IMPORTS: &str = "(import_statement source: (string) @import)
     (export_statement source: (string) @import)
     (call_expression
       function: (import)
       arguments: (arguments (string) @import))";

const C_SYMBOLS: &str = r#"
    (function_definition
       declarator: (function_declarator declarator: (identifier) @name)) @item
    (declaration
       declarator: (function_declarator declarator: (identifier) @name)) @item
    (struct_specifier name: (type_identifier) @name) @item
    (enum_specifier name: (type_identifier) @name) @item
    (union_specifier name: (type_identifier) @name) @item
    (type_definition declarator: (type_identifier) @name) @item
"#;

const C_IMPORTS: &str = r#"(preproc_include path: (string_literal) @import)
     (preproc_include path: (system_lib_string) @import)"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::{self, Language};
    use clap::ValueEnum;
    use tree_sitter::Query;

    /// Every query must compile against its own grammar. Without this, a typo
    /// in a node name only shows up as silently missing symbols at runtime.
    #[test]
    fn test_all_queries_compile() {
        let mut failures = Vec::new();

        for lang in Language::value_variants() {
            let ts_lang = languages::get_ts_language(*lang);
            let q = queries_for(*lang);

            if let Err(e) = Query::new(&ts_lang, q.symbols) {
                failures.push(format!("{:?} symbols: {}", lang, e));
            }
            if let Some(imports) = q.imports
                && let Err(e) = Query::new(&ts_lang, imports)
            {
                failures.push(format!("{:?} imports: {}", lang, e));
            }
        }

        assert!(
            failures.is_empty(),
            "queries failed:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn test_every_query_captures_item_and_name() {
        for lang in Language::value_variants() {
            let q = queries_for(*lang);
            assert!(
                q.symbols.contains("@item"),
                "{:?} symbol query must capture @item",
                lang
            );
            if *lang != Language::Markdown {
                assert!(
                    q.symbols.contains("@name"),
                    "{:?} symbol query must capture @name",
                    lang
                );
            }
        }
    }
}
