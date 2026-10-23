//! The SBOM of the inventory (ADR 0055): CycloneDX 1.5 in JSON, by `cyclonedx-bom`, validated
//! before it is given. One `application` component per package of the repository, one
//! `library` per dependency with its licence and its package URL, and the vulnerabilities OSV
//! gave, each on the components it affects.

use cyclonedx_bom::external_models::normalized_string::NormalizedString;
use cyclonedx_bom::external_models::spdx::SpdxExpression;
use cyclonedx_bom::external_models::uri::Purl;
use cyclonedx_bom::models::bom::{Bom, SpecVersion};
use cyclonedx_bom::models::component::{Classification, Component, Components};
use cyclonedx_bom::models::license::{LicenseChoice, Licenses};
use cyclonedx_bom::models::metadata::Metadata;
use cyclonedx_bom::models::vulnerability::{Vulnerabilities, Vulnerability};
use cyclonedx_bom::models::vulnerability_target::{VulnerabilityTarget, VulnerabilityTargets};
use cyclonedx_bom::validation::Validate;
use timonier_core::inventory::{Dependency, Ecosystem, Vulnerability as Found};

/// An ecosystem's type in a package URL.
fn purl_type(ecosystem: Ecosystem) -> &'static str {
    timonier_index::tooling::packages(ecosystem).map_or("generic", |p| p.purl)
}

/// A package URL. Some names carry a namespace, which the purl writes with `/`
/// (`pkg:maven/io.opentelemetry/opentelemetry-api@1.67.0`); the ecosystem says how.
fn purl(ecosystem: Ecosystem, name: &str, version: &str) -> Option<Purl> {
    use timonier_index::tooling::PurlName;
    let namespaced = match timonier_index::tooling::packages(ecosystem).map(|p| p.purl_name) {
        Some(PurlName::GroupArtifact) => name.split_once(':').map(|(g, a)| format!("{g}/{a}")),
        Some(PurlName::Path) => Some(name.to_owned()),
        Some(PurlName::Plain) | None => None,
    };
    match namespaced {
        Some(path) => format!("pkg:{}/{path}@{version}", purl_type(ecosystem))
            .parse()
            .ok(),
        None => Purl::new(purl_type(ecosystem), name, version).ok(),
    }
}

/// The SBOM as JSON, or why it is not valid.
pub fn sbom(
    packages: &[(String, Option<String>, Ecosystem)],
    dependencies: &[Dependency],
    vulnerabilities: &[Found],
) -> Result<serde_json::Value, String> {
    let reference = |ecosystem: Ecosystem, name: &str, version: &str| {
        format!("pkg:{}/{name}@{version}", purl_type(ecosystem))
    };
    let mut components = Vec::new();
    for (name, version, ecosystem) in packages {
        let version = version.as_deref().unwrap_or("0.0.0");
        let mut component = Component::new(
            Classification::Application,
            name,
            version,
            Some(format!("repo:{}", reference(*ecosystem, name, version))),
        );
        component.purl = purl(*ecosystem, name, version);
        components.push(component);
    }
    for dependency in dependencies {
        let version = dependency.version.as_deref().unwrap_or("0.0.0");
        let mut component = Component::new(
            Classification::Library,
            &dependency.name,
            version,
            Some(reference(dependency.ecosystem, &dependency.name, version)),
        );
        component.purl = purl(dependency.ecosystem, &dependency.name, version);
        component.licenses = dependency.license.as_deref().map(|license| {
            // `MIT/Apache-2.0`, as older crates write it, is `MIT OR Apache-2.0`; an expression
            // SPDX cannot read stays a named licence.
            Licenses(vec![match SpdxExpression::parse_lax(license.to_owned()) {
                Ok(expression) => LicenseChoice::Expression(expression),
                Err(_) => LicenseChoice::license(license),
            }])
        });
        components.push(component);
    }
    // One component per reference: two lockfiles may name the same package.
    let mut seen = std::collections::BTreeSet::new();
    components.retain(|c| seen.insert(c.bom_ref.clone()));
    let vulnerabilities: Vec<Vulnerability> = vulnerabilities
        .iter()
        .map(|found| {
            let mut vulnerability = Vulnerability::new(None);
            vulnerability.id = Some(NormalizedString::new(&found.id));
            vulnerability.description = found.summary.clone();
            vulnerability.vulnerability_targets =
                Some(VulnerabilityTargets(vec![VulnerabilityTarget::new(
                    reference(found.ecosystem, &found.package, &found.version),
                )]));
            vulnerability
        })
        .collect();
    let bom = Bom {
        spec_version: SpecVersion::V1_5,
        metadata: Some(Metadata::new().map_err(|e| e.to_string())?),
        components: Some(Components(components)),
        vulnerabilities: (!vulnerabilities.is_empty()).then_some(Vulnerabilities(vulnerabilities)),
        ..Bom::default()
    };
    let checked = bom.validate_version(SpecVersion::V1_5);
    if checked.has_errors() {
        return Err(format!(
            "the SBOM is not valid: {:?}",
            checked.errors().collect::<Vec<_>>()
        ));
    }
    let mut out = Vec::new();
    bom.output_as_json(&mut out, SpecVersion::V1_5)
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use timonier_core::inventory::Scope;

    use super::*;

    /// T-123: Maven's group and Go's module path are the purl's namespace.
    #[test]
    fn purls_carry_the_namespace_of_maven_and_go() {
        let shown = |e, name, version| purl(e, name, version).map(|p| p.to_string());
        assert_eq!(
            shown(
                Ecosystem::new("maven", 4),
                "io.opentelemetry:opentelemetry-api",
                "1.67.0"
            )
            .as_deref(),
            Some("pkg:maven/io.opentelemetry/opentelemetry-api@1.67.0")
        );
        assert_eq!(
            shown(
                Ecosystem::new("go", 3),
                "github.com/gin-gonic/gin",
                "v1.11.0"
            )
            .as_deref(),
            Some("pkg:golang/github.com/gin-gonic/gin@v1.11.0")
        );
        assert_eq!(
            shown(Ecosystem::new("cargo", 0), "serde", "1.0.228").as_deref(),
            Some("pkg:cargo/serde@1.0.228")
        );
    }

    #[test]
    fn the_sbom_is_valid_cyclonedx_with_licences_purls_and_vulnerabilities() {
        let dependency = |name: &str, license: Option<&str>| Dependency {
            ecosystem: Ecosystem::new("cargo", 0),
            name: name.to_owned(),
            version: Some("1.0.0".to_owned()),
            license: license.map(str::to_owned),
            direct: true,
            scope: Scope::Normal,
            used_by: vec!["app".to_owned()],
            function: None,
        };
        let json = sbom(
            &[(
                "app".to_owned(),
                Some("0.1.0".to_owned()),
                Ecosystem::new("cargo", 0),
            )],
            &[
                dependency("serde", Some("MIT OR Apache-2.0")),
                dependency("old", Some("MIT/Apache-2.0")),
                dependency("odd", Some("Some custom licence")),
                dependency("bare", None),
            ],
            &[Found {
                id: "RUSTSEC-2020-0071".to_owned(),
                ecosystem: Ecosystem::new("cargo", 0),
                package: "old".to_owned(),
                version: "1.0.0".to_owned(),
                aliases: vec![],
                summary: Some("Potential segfault".to_owned()),
                severity: None,
            }],
        )
        .unwrap();
        assert_eq!(json["bomFormat"], "CycloneDX");
        assert_eq!(json["specVersion"], "1.5");
        let components = json["components"].as_array().unwrap();
        assert_eq!(components.len(), 5);
        assert_eq!(components[0]["type"], "application");
        assert_eq!(components[1]["purl"], "pkg:cargo/serde@1.0.0");
        assert_eq!(
            components[2]["licenses"][0]["expression"],
            "MIT OR Apache-2.0"
        );
        assert_eq!(
            components[3]["licenses"][0]["license"]["name"],
            "Some custom licence"
        );
        assert_eq!(
            json["vulnerabilities"][0]["affects"][0]["ref"],
            "pkg:cargo/old@1.0.0"
        );
        // It reads back as CycloneDX.
        let text = serde_json::to_vec(&json).unwrap();
        assert!(Bom::parse_from_json_v1_5(text.as_slice()).is_ok());
    }
}
