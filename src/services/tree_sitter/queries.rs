use crate::services::tree_sitter::SupportedLanguage;
use lazy_static::lazy_static;
use std::collections::HashMap;
use tree_sitter::Query;

lazy_static! {
    static ref QUERIES: HashMap<SupportedLanguage, LanguageQueries> = {
        let mut m = HashMap::new();

        // JavaScript Queries
        m.insert(SupportedLanguage::JavaScript, LanguageQueries {
            definitions: Query::new(
                tree_sitter_javascript::language(),
                r#"
                (function_declaration
                    name: (identifier) @function.name) @function.definition

                (class_declaration
                    name: (identifier) @class.name) @class.definition

                (method_definition
                    name: (property_identifier) @method.name) @method.definition

                (export_statement
                    (function_declaration
                        name: (identifier) @export.function.name) @export.function.definition)

                (export_statement
                    (class_declaration
                        name: (identifier) @export.class.name) @export.class.definition)
                "#,
            ).unwrap(),

            components: Query::new(
                tree_sitter_javascript::language(),
                r#"
                (jsx_element
                    open_tag: (jsx_opening_element
                        name: (_) @component.name)) @component.definition

                (variable_declaration
                    (variable_declarator
                        name: (identifier) @component.name
                        value: (arrow_function))) @component.definition
                "#,
            ).unwrap(),
        });

        // TypeScript Queries
        m.insert(SupportedLanguage::TypeScript, LanguageQueries {
            definitions: Query::new(
                tree_sitter_typescript::language_typescript(),
                r#"
                (function_declaration
                    name: (identifier) @function.name) @function.definition

                (class_declaration
                    name: (type_identifier) @class.name) @class.definition

                (method_definition
                    name: (property_identifier) @method.name) @method.definition

                (interface_declaration
                    name: (type_identifier) @interface.name) @interface.definition

                (type_alias_declaration
                    name: (type_identifier) @type.name) @type.definition
                "#,
            ).unwrap(),

            components: Query::new(
                tree_sitter_typescript::language_typescript(),
                r#"
                (jsx_element
                    open_tag: (jsx_opening_element
                        name: (_) @component.name)) @component.definition

                (variable_declaration
                    (variable_declarator
                        name: (identifier) @component.name
                        value: (arrow_function))) @component.definition
                "#,
            ).unwrap(),
        });

        // Python Queries
        m.insert(SupportedLanguage::Python, LanguageQueries {
            definitions: Query::new(
                tree_sitter_python::language(),
                r#"
                (function_definition
                    name: (identifier) @function.name) @function.definition

                (class_definition
                    name: (identifier) @class.name) @class.definition

                (decorated_definition
                    definition: (function_definition
                        name: (identifier) @decorated.function.name)) @decorated.definition

                (decorated_definition
                    definition: (class_definition
                        name: (identifier) @decorated.class.name)) @decorated.definition
                "#,
            ).unwrap(),

            components: Query::new(
                tree_sitter_python::language(),
                r#"
                (class_definition
                    (identifier) @component.name
                    (argument_list
                        (identifier) @base.component)) @component.definition
                "#,
            ).unwrap(),
        });

        // Rust Queries
        m.insert(SupportedLanguage::Rust, LanguageQueries {
            definitions: Query::new(
                tree_sitter_javascript::language(),
                r#"
                // Using JavaScript language temporarily due to version issues
                (function_declaration
                    name: (identifier) @function.name) @function.definition

                (class_declaration
                    name: (identifier) @class.name) @class.definition
                "#,
            ).unwrap(),

            components: Query::new(
                tree_sitter_javascript::language(),
                r#"
                // Using JavaScript language temporarily due to version issues
                (jsx_element
                    open_tag: (jsx_opening_element
                        name: (_) @component.name)) @component.definition
                "#,
            ).unwrap(),
        });

        // Markdown Queries
        m.insert(SupportedLanguage::Markdown, LanguageQueries {
            definitions: Query::new(
                tree_sitter_javascript::language(),
                r#"
                // Using JavaScript language temporarily due to version issues
                (comment) @comment.content
                "#,
            ).unwrap(),

            components: Query::new(
                tree_sitter_javascript::language(),
                r#"
                // Using JavaScript language temporarily due to version issues
                (comment) @comment.content
                "#,
            ).unwrap(),
        });

        m
    };
}

pub struct LanguageQueries {
    pub definitions: Query,
    pub components: Query,
}

impl LanguageQueries {
    pub fn get(language: SupportedLanguage) -> Option<&'static Self> {
        QUERIES.get(&language)
    }
}

pub fn get_query_matches(query: &Query, node: tree_sitter::Node, source: &str) -> Vec<QueryMatch> {
    let mut cursor = tree_sitter::QueryCursor::new();
    let matches = cursor.matches(query, node, source.as_bytes());

    matches
        .map(|m| QueryMatch {
            pattern_index: m.pattern_index,
            captures: m
                .captures
                .iter()
                .map(|c| QueryCapture {
                    name: c.node.kind().to_string(),
                    text: source[c.node.byte_range()].to_string(),
                    start_line: c.node.start_position().row + 1,
                    end_line: c.node.end_position().row + 1,
                })
                .collect(),
        })
        .collect()
}

#[derive(Debug)]
pub struct QueryMatch {
    pub pattern_index: usize,
    pub captures: Vec<QueryCapture>,
}

#[derive(Debug)]
pub struct QueryCapture {
    pub name: String,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
}
