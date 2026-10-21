# 0044 — L'instrumentation de Timonier : la feature `otel` et `code.function.name`

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances » (conventions imposées à l'IA, instrumentation désactivable) ;
  skill `otel-instrumentation` ; ADR 0003 (crates) ; tâches PLAN.md 8.1, 8.4, 8.6

## Contexte

Timonier s'instrumente avec les règles qu'il impose : un span par frontière de composant, les
attributs `code.function.name`, `code.file.path` et `code.line.number`, une instrumentation qu'on
coupe à la compilation ou par la configuration. Les bibliothèques n'utilisent que `tracing`. Il
reste à nommer la feature, à dire quand l'export s'allume, et à combler un manque vérifié :
`tracing-opentelemetry` 0.34.0 (MIT, vérifiée le 2026-10-21, source `src/layer.rs`) écrit
`code.file.path`, `code.line.number` et `code.module.name`, jamais `code.function.name`.

## Décision

**La feature** s'appelle `otel`. Elle n'existe que dans `timonier-cli`, le binaire, et y est
active par défaut ; `cargo build --no-default-features` donne un binaire sans aucune dépendance
OpenTelemetry. Elle apporte `tracing-opentelemetry` 0.34.0 et `opentelemetry`,
`opentelemetry_sdk`, `opentelemetry-otlp` 0.33.1 (Apache-2.0, vérifiées le 2026-10-21), celui-ci
sans ses features par défaut, avec `grpc-tonic` : tonic est déjà là pour le récepteur.

**À l'exécution**, l'export ne s'allume que si `OTEL_EXPORTER_OTLP_ENDPOINT` est posée, comme
le fait `timonier test`, et `OTEL_SDK_DISABLED=true` l'éteint (variables standard
d'OpenTelemetry ; le SDK Rust 0.33.1 ne lisant pas la seconde, le binaire la lit lui-même). Sans
elles, aucun exporteur n'est créé et aucune connexion n'est tentée.

**`code.function.name`** : pour un span Rust venu de `tracing` (il a `code.module.name` et pas
`code.function.name`), le récepteur le déduit à l'arrivée : `<code.module.name>::<nom du span>`,
le nom que `#[tracing::instrument]` donne par défaut à la fonction, et le marque comme déduit.
Un span qui a déjà l'attribut le garde. La convention Rust de `timonier init` (8.6) dit de ne pas
renommer le span d'une frontière, pour que la déduction tienne.

## Alternatives écartées

- **Une feature par crate** : les bibliothèques n'ont que `tracing`, qui ne coûte presque rien
  sans abonné ; c'est le binaire qui choisit l'exporteur.
- **Feature désactivée par défaut** : `timonier doctor` doit produire une trace (8.6) avec le
  binaire qu'on installe.
- **Écrire `code.function.name` à la main dans chaque `#[instrument]`** : à répéter, et faux au
  premier renommage.
- **Une couche `tracing` qui ajoute l'attribut dans le processus** : elle dépend des structures
  internes de `tracing-opentelemetry`, encore instables (API « Beta »).

## Conséquences

- La règle de déduction vaut pour tout projet Rust instrumenté avec `tracing`, pas seulement
  pour Timonier.
- La vérification de l'instrumentation (8.5) ne compte pas comme trou un attribut déduit.
- Si une version de `tracing-opentelemetry` écrit un jour `code.function.name`, elle passe
  devant la déduction sans rien changer.
