//! Tokens of a syntax tree, for fingerprints and canonical signatures (ADR 0013): the leaves,
//! without comments, so that layout does not matter.

use timonier_core::ContentHash;
use tree_sitter::Node;

use crate::pack::{FingerprintRules, SignatureRules};

/// The tokens of `node` that start before `end`, after the pack's drop rules.
pub(crate) fn tokens<'s>(
    node: Node<'_>,
    source: &'s [u8],
    end: usize,
    rules: &FingerprintRules,
) -> Vec<&'s str> {
    let mut raw = Vec::new();
    collect(node, source, end, rules, &mut raw);
    let dropped = |t: &str| rules.drop.iter().any(|d| d == t);
    let raw: Vec<&str> = raw.into_iter().filter(|t| !dropped(t)).collect();
    let mut kept: Vec<&str> = Vec::with_capacity(raw.len());
    for (i, token) in raw.iter().enumerate() {
        // A trailing separator, before a closing delimiter or at the end (a signature stops
        // where the body starts): `f(a, b,)`, `where T: Clone,`.
        if rules.drop_before_closing.iter().any(|d| d == token)
            && raw
                .get(i + 1)
                .is_none_or(|n| rules.closing.iter().any(|c| c == n))
        {
            continue;
        }
        // A leading operator, at the start or after an opening token: `= | a | b`.
        if rules.drop_leading.iter().any(|d| d == token)
            && kept
                .last()
                .is_none_or(|p| rules.opening.iter().any(|o| o == p))
        {
            continue;
        }
        kept.push(token);
    }
    kept
}

fn collect<'s>(
    node: Node<'_>,
    source: &'s [u8],
    end: usize,
    rules: &FingerprintRules,
    out: &mut Vec<&'s str>,
) {
    if node.start_byte() >= end || node.is_extra() {
        return;
    }
    if node.child_count() == 0 || rules.atoms.iter().any(|a| a == node.kind()) {
        let range = node.start_byte()..node.end_byte().min(end);
        if let Ok(text) = std::str::from_utf8(&source[range]) {
            let text = text.trim();
            if !text.is_empty() {
                out.push(text);
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, end, rules, out);
    }
}

/// BLAKE3 of the tokens, separated by a byte that no token contains.
pub(crate) fn hash(tokens: &[&str]) -> ContentHash {
    let mut hasher = blake3::Hasher::new();
    for token in tokens {
        hasher.update(token.as_bytes());
        hasher.update(&[0x1f]);
    }
    ContentHash::from_bytes(*hasher.finalize().as_bytes())
}

/// The tokens joined with one space, except around the pack's tight tokens:
/// `pub fn from_hex(hex: &str) -> Result<Self>`. Whitespace inside a token collapses.
pub(crate) fn canonical(tokens: &[&str], rules: &SignatureRules) -> String {
    let mut text = String::new();
    let mut previous: Option<&str> = None;
    for token in tokens {
        if let Some(prev) = previous {
            let tight = rules.no_space_after.iter().any(|t| t == prev)
                || rules.no_space_before.iter().any(|t| t == token)
                || (rules.tight_after_word.iter().any(|t| t == token) && ends_a_word(prev));
            if !tight {
                text.push(' ');
            }
        }
        let mut words = token.split_whitespace();
        if let Some(first) = words.next() {
            text.push_str(first);
            for word in words {
                text.push(' ');
                text.push_str(word);
            }
        }
        previous = Some(token);
    }
    text
}

/// A name or a closing bracket: `f(`, `Vec<u8>`, `f<T>(` stay tight, `= (x)` and `-> (u8)` do
/// not.
fn ends_a_word(token: &str) -> bool {
    matches!(token, ">" | ")" | "]")
        || token
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_rules() -> (FingerprintRules, SignatureRules) {
        let packs = crate::Packs::builtin().unwrap();
        let rust = packs.iter().next().unwrap();
        (
            rust.manifest.fingerprint.clone(),
            rust.manifest.signature.clone(),
        )
    }

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn all_tokens(source: &str) -> Vec<String> {
        let (rules, _) = rust_rules();
        let tree = parse(source);
        tokens(tree.root_node(), source.as_bytes(), source.len(), &rules)
            .into_iter()
            .map(String::from)
            .collect()
    }

    #[test]
    fn layout_and_comments_do_not_change_the_tokens() {
        let a = all_tokens("fn f(a: u32, b: u32) -> u32 { a + b }");
        let b = all_tokens(
            "fn f(\n    a: u32, // first\n    b: u32,\n) -> u32 {\n    /* sum */ a + b\n}\n",
        );
        assert_eq!(a, b);
        assert_eq!(hash(&a.iter().map(String::as_str).collect::<Vec<_>>()), {
            let b: Vec<&str> = b.iter().map(String::as_str).collect();
            hash(&b)
        });
        let c = all_tokens("fn f(a: u32, b: u32) -> u32 { a - b }");
        assert_ne!(a, c);
    }

    #[test]
    fn literals_are_one_token_and_keep_their_content() {
        let tokens = all_tokens("const S: &str = \"a  b\";");
        assert!(tokens.contains(&"\"a  b\"".to_owned()), "{tokens:?}");
        assert_ne!(all_tokens("const S: &str = \"a b\";"), tokens);
    }

    #[test]
    fn trailing_commas_only_go_before_a_closing_delimiter() {
        assert_eq!(
            all_tokens("fn x() { f(a, b,) }"),
            all_tokens("fn x() { f(a, b) }")
        );
        assert_ne!(
            all_tokens("fn x() { f(a, b) }"),
            all_tokens("fn x() { f(a; b) }")
        );
        assert!(all_tokens("fn x() { f(a, b) }").contains(&",".to_owned()));
    }

    #[test]
    fn signatures_are_canonical() {
        let (_, rules) = rust_rules();
        for (source, expected) in [
            (
                "pub fn from_hex(hex: &str) -> Result<Self>",
                "pub fn from_hex(hex: &str) -> Result<Self>",
            ),
            (
                "pub fn f(\n  a: &'a mut Vec<u8>,\n) -> !",
                "pub fn f(a: &'a mut Vec<u8>) -> !",
            ),
            (
                "impl<T: Clone> Foo<T> where T: Send",
                "impl<T: Clone> Foo<T> where T: Send",
            ),
            (
                "fn g(x: *const u8, y: std::fmt::Arguments<'_>)",
                "fn g(x: *const u8, y: std::fmt::Arguments<'_>)",
            ),
        ] {
            let tokens = all_tokens(source);
            let tokens: Vec<&str> = tokens.iter().map(String::as_str).collect();
            assert_eq!(canonical(&tokens, &rules), expected, "{source}");
        }
    }
}
