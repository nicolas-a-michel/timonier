//! The compiled-in grammars (ADR 0012): a grammar is compiled C, so this table is the one place
//! where adding a language touches code.

use tree_sitter::Language;

/// The grammar a pack names, if this build contains it.
pub(crate) fn by_name(name: &str) -> Option<Language> {
    match name {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grammar_loads_into_a_parser() {
        for name in ["rust", "typescript", "tsx", "javascript"] {
            let language = by_name(name).unwrap();
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&language).unwrap();
            assert!(parser.parse("x", None).is_some(), "{name}");
        }
        assert!(by_name("cobol").is_none());
    }
}
