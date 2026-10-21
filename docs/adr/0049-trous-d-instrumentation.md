# 0049 — Les trous d'instrumentation et le surcoût

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances » (« Vérification de l'instrumentation : fonctions publiques jamais
  vues, spans sans attributs `code.*`, surcoût estimé contre le niveau 1, lisibles via
  `get_instrumentation_gaps` ») ; skill `mcp-tools` ; ADR 0044, 0045, 0048 ; tâche PLAN.md 8.5

## Contexte

Après un run, le démon doit dire ce que l'instrumentation ne couvre pas. Il faut décider ce qu'est
une fonction publique pour le graphe, sur quel run on juge, et comment estimer un surcoût. Le
graphe a la signature de chaque fonction : `pub fn …` pour Rust ; pour TypeScript, la signature
ne porte pas `export` (l'extraction prend la déclaration, pas l'`export_statement`).

## Décision

**Le run jugé** : le dernier run fini avec traces, hors tests de fumée de `doctor` (relecture
n° 2), ou
celui que l'outil nomme ; à la tête du graphe.

**Fonctions publiques** : les fonctions et méthodes du graphe, hors code de test (cibles
`@tests`, `@examples`, `@benches`, `@build`, modules `tests`, fichiers `*.test.*`, `*.spec.*` et
dossiers `test/`, `tests/`). Rust : la signature commence par `pub` (`pub(crate)` compris : une
frontière entre modules). TypeScript : toutes, faute d'`export` dans le graphe, sauf les méthodes
`private` ou `#` ; le rapport le dit (`approximations`). **Jamais vue** : aucun span du run n'y
est rattaché par l'ID, le nom ou la ligne (ADR 0048 ; le rattachement au seul fichier ne compte
pas).

**Spans sans `code.*`** : les spans reçus (pas les racines synthétisées) à qui manque
`code.function.name` (déduit compris, ADR 0044), `code.file.path` ou `code.line.number`, groupés
par nom avec les attributs manquants. S'y ajoutent les spans rattachés à aucun nœud et ceux
comptés au-delà du plafond du run.

**Surcoût** : `timonier test --without-traces` lance la commande avec `OTEL_SDK_DISABLED=true`
(l'instrumentation coupée par la configuration, ADR 0044) et marque le run sans traces ; le
récepteur ne garde rien d'un tel run. Le SDK Rust 0.33.1 ne lit pas cette variable (vérifié le
2026-10-21 dans ses sources) : le module d'initialisation des conventions la lit lui-même (8.6,
qui vérifiera aussi le SDK de TypeScript). Le surcoût compare la médiane des runs avec traces à
celle des runs sans, de même commit, même commande et même empreinte de machine (la même source
CPU donc : instructions, sinon temps CPU, ADR 0046) : `(avec − sans) / sans`. Sans run sans
traces comparable, le rapport le dit et donne la commande.

**Outils MCP** : `get_instrumentation_gaps { run?, cursor?, limit? }` rend le rapport, les
fonctions jamais vues par pages (500 au plus, 100 par défaut) ; `get_instrumentation_conventions
{ language }` rend `.timonier/conventions/<langage>/instrumentation.md`, ce que `timonier init` a
copié (ADR 0034). Tous deux en lecture seule, avec l'en-tête de boîte aux lettres (ADR 0028).

## Alternatives écartées

- **Estimer le surcoût par un coût fixe par span** : un chiffre inventé, qui ignore le langage
  et l'exporteur.
- **Lancer deux fois à chaque `timonier test`** : double le temps des tests sans qu'on l'ait
  demandé.
- **Changer l'extraction TypeScript pour garder `export`** : une nouvelle version du pack et du
  cache, pour une approximation que le rapport nomme déjà ; à faire avec le pack.
- **Juger sur l'union de plusieurs runs** : un run qui couvre moins masquerait ce qu'il a perdu.

## Conséquences

- Une fonction jamais vue peut être non instrumentée ou non testée : les deux sont des trous
  pour la carte annotée.
- Le surcoût n'existe qu'après un run sans traces du même commit.
