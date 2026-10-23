//! The built-in packs' manifests, read once (ADR 0077): what the daemon needs of a language
//! comes from them, and the daemon names none.

use std::sync::OnceLock;

use timonier_core::gaps::CodeRules;
use timonier_index::pack::Manifest;

/// The built-in packs' manifests, in their order; none if they do not read, which the
/// indexer's tests rule out.
pub(crate) fn manifests() -> &'static [Manifest] {
    static MANIFESTS: OnceLock<Vec<Manifest>> = OnceLock::new();
    MANIFESTS.get_or_init(|| timonier_index::Packs::builtin_manifests().unwrap_or_default())
}

/// Every language's rules of public and test code, for the instrumentation gaps.
pub(crate) fn code_rules() -> &'static CodeRules {
    static RULES: OnceLock<CodeRules> = OnceLock::new();
    RULES.get_or_init(|| CodeRules {
        languages: manifests()
            .iter()
            .map(|m| (m.language.clone(), m.code.clone()))
            .collect(),
    })
}

/// The manifest of a file's language: by the language a span says, else by its extension.
pub(crate) fn of(language: Option<&str>, file: Option<&str>) -> Option<&'static Manifest> {
    let extension = file
        .and_then(|f| f.rsplit('/').next())
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension);
    manifests()
        .iter()
        .find(|m| language == Some(m.language.as_str()))
        .or_else(|| {
            let extension = extension?;
            manifests().iter().find(|m| {
                m.grammars
                    .iter()
                    .any(|g| g.extensions.iter().any(|e| e == extension))
            })
        })
}

/// The folders under the home where the packs' installers put their indexers.
pub(crate) fn bin_dirs() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for dir in manifests()
        .iter()
        .filter_map(|m| m.scip.as_ref())
        .flat_map(|s| s.bin_dirs.iter())
    {
        if !out.contains(&dir.as_str()) {
            out.push(dir.as_str());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_s_pack_comes_from_its_language_or_its_extension() {
        let by_extension = of(None, Some("src/main.rs")).expect("a pack");
        assert!(by_extension.code.function_ids);
        let said = of(Some(by_extension.language.as_str()), None).expect("a pack");
        assert_eq!(said.language, by_extension.language);
        assert!(of(None, Some("README")).is_none());
        assert!(!bin_dirs().is_empty());
        assert!(!code_rules().languages.is_empty());
    }
}
