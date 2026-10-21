# 0048 — Le récepteur OTLP : ports, protocoles, ce qu'il garde, et le rattachement au graphe

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances » (récepteur OTLP local, gRPC 4317 / HTTP 4318 ; attributs
  `code.*` pour le rattachement) ; ADR 0043 (stockage), 0044 (`code.function.name`), 0047 (run,
  span racine des tests) ; tâche PLAN.md 8.4

## Contexte

Il y a un démon par projet, et deux projets ouverts ne peuvent pas écouter tous deux 4317 et
4318 ; un collecteur OpenTelemetry du développeur peut aussi les tenir. Un SDK choisit son
protocole (gRPC ou HTTP) selon sa configuration, que Timonier ne connaît pas. Les spans doivent
se rattacher à un nœud du graphe : les ID Rust du graphe sont `rust:<crate>::<module>::<nom>`
(ADR 0013), et `code.function.name` déduit d'un span `tracing` est `<module>::<nom>` (ADR 0044),
sans le type d'une méthode.

## Décision

**Où et quand** : dans le démon (`timonier-otlp`, appelé par lui), démarré au premier
`run/start` et gardé jusqu'à l'arrêt du démon. Il écoute sur `127.0.0.1` seulement : 4317 et
4318 s'ils sont libres, sinon un port choisi par le système pour celui qui est pris. Chaque port
sert les deux protocoles (gRPC sur HTTP/2 sans TLS, HTTP/1.1 en protobuf ou en JSON sur
`/v1/traces` et `/v1/metrics`), si bien que l'adresse injectée (`OTEL_EXPORTER_OTLP_ENDPOINT`,
celle du port HTTP) convient à tout exporteur. Pas de compression pour l'instant (les
exporteurs n'en font pas par défaut) ; un message est borné à 16 Mio. Bibliothèques, vérifiées
le 2026-10-21 : `tonic` 0.14.6 (MIT ; `server`, `router`, `codegen`), `opentelemetry-proto`
0.33.1 (Apache-2.0 ; `gen-tonic`, `trace`, `metrics`, `with-serde`), `prost` 0.14.4
(Apache-2.0), `axum` 0.8.9 (MIT ; `http1`, `http2`, `tokio`).

**Ce qu'il garde** : un span ou un point de métrique dont la ressource (ou lui-même) porte
`timonier.run_id` d'un run de ce démon, commencé, et fini depuis moins de 60 s (les exporteurs
vident leurs files à la sortie du processus). Le reste est compté et jeté : ce n'est pas un run.
Au-delà de 200 000 spans par run (ADR 0043), les suivants sont comptés dans le run. Les spans et
métriques des runs plus anciens que les 50 derniers sont effacés au début d'un run.

**À quel test** : `timonier.test.name` et `timonier.test.classname` (du span, sinon de sa
ressource) le rattachent au cas du JUnit de même nom (et de même classe quand elle est donnée),
au moment où le run finit, ou à l'arrivée si le run est déjà fini. Un span sans parent dans
son test est montré sous la racine du test (ADR 0047) ; le parent reçu, lui, est gardé tel quel.

**À quel nœud**, dans cet ordre, à la tête du graphe à l'arrivée :
1. `code.function.name` est un ID du graphe, une fois le langage préfixé (`rust:` +
   `timonier_daemon::server::start`) ;
2. dans le fichier `code.file.path` (relatif à la racine, ou absolu sous elle), la fonction ou
   méthode dont l'ID finit par le dernier segment du nom (`::start`, `#start`, `.start`) ;
   s'il y en a plusieurs, la plus proche de `code.line.number` ;
3. le symbole le plus intérieur du fichier qui contient la ligne ;
4. le fichier.
Le nœud et la règle qui l'a donné sont gardés ; sans fichier ni nom, rien n'est rattaché (un trou
d'instrumentation, 8.5).

## Alternatives écartées

- **Toujours un port choisi par le système** : la spec nomme 4317 et 4318, et un outil du
  développeur pointé sur eux trouve Timonier.
- **Un port par protocole seulement** : il faudrait savoir quel protocole l'exporteur emploie,
  ou poser aussi `OTEL_EXPORTER_OTLP_PROTOCOL`, que le code du projet peut fixer autrement.
- **Écouter sur toutes les interfaces** : les traces d'un projet n'ont pas à sortir de la
  machine.
- **Réécrire le parent d'un span orphelin** : le fait reçu serait perdu ; la vue fait le
  rattachement.
- **Rattacher par la ligne d'abord** : la ligne d'un `#[instrument]` est souvent celle de
  l'attribut, au-dessus de la fonction.

## Conséquences

- Plusieurs démons coexistent : le premier a 4317 et 4318, les autres des ports à eux, que
  `timonier test` donne aux tests.
- Une trace venue d'ailleurs que d'un run (une application lancée à la main vers 4318) n'est
  pas gardée.
- Le rattachement d'une méthode Rust passe par la règle 2 : le module ne dit pas son type.
