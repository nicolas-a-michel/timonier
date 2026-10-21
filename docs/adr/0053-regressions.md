# 0053 — La détection des régressions de performances

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Seuil de régression » (décidée : instructions CPU quand c'est possible, temps
  réel affiché ; 5 exécutions, jusqu'à 15 si la variance est forte ; médiane et MAD ; référence =
  médiane des 5 derniers snapshots de la branche principale ; régression si Mann-Whitney p < 0,01
  et écart ≥ max(5 %, 3 × MAD) ; « probable » puis « confirmée » après un nouveau run ; référence
  réinitialisée si l'empreinte de la machine change) ; ADR 0043, 0046, 0051 ; tâche PLAN.md 8.9

## Contexte

La spec décide la méthode. Restent à fixer : ce qu'est un lot d'exécutions, quelles mesures on
compare, ce qu'est la branche principale et son snapshot, quel test exact, et où le résultat se
lit.

## Décision

**Le lot** : `timonier test --runs N` lance la commande N fois (1 par défaut), `--runs auto` 5
fois, puis jusqu'à 15 tant que la MAD du coût CPU des runs dépasse 5 % de leur médiane. Les runs
d'un lot partagent un nom (`B-…`, colonne `batch`). Un run qui échoue arrête le lot : ses temps
ne se comparent pas.

**Les mesures comparées**, chacune sur les runs du lot : le coût du run (instructions quand le
compteur est permis, sinon temps CPU, ADR 0046), la durée de chaque test (JUnit), le temps propre
de chaque nœud (ADR 0051). Par test et par nœud, faute de compteur par processus, c'est le temps.

**La référence** : les runs finis de même commande et même empreinte de machine, sur un arbre de
travail propre (Q77 : un run sur un arbre modifié garde l'empreinte de ses changements, hors
`.timonier/`, et ne représente jamais son commit), sur la branche
principale (celle que vise `origin/HEAD`, sinon `main`), des 5 derniers commits autres que celui
du lot. Valeur de référence : la médiane des médianes de ces commits ; dispersion : la MAD de
toutes leurs valeurs. Une autre empreinte n'en fait pas partie : la référence repart avec la
nouvelle machine, et le rapport le dit.

**Le jugement** (`timonier-core::regression`, pur) : Mann-Whitney unilatéral (le lot plus lent),
exact par la récurrence de dénombrement sans ex æquo et jusqu'à 40 valeurs, sinon approximation
normale avec correction des ex æquo et de continuité ; régression si p < 0,01 et si l'écart des
médianes atteint max(5 % de la référence, 3 × MAD). Vérifié contre SciPy 1.18.1 sur des jeux
connus (valeurs exactes à 1e-12, asymptotiques à 1e-4 ; SciPy installé à part, hors du projet).
« Probable » au premier lot qui la voit ; « confirmée » quand le lot précédent du même commit,
et de la même empreinte d'arbre modifié (Q77), la voyait aussi. Un lot d'un seul run ne peut
rien montrer : le rapport le dit (relecture n° 1, m2).

**Où ça se lit** : `perf/regressions { batch? }` ; à la fin de `timonier test` quand le lot a au
moins deux runs (une ligne par régression, « relancez pour confirmer » si probable) ; l'outil MCP
`get_regressions`, nouveau (la spec ne nomme pas d'outil pour les régressions ; ses noms sont
provisoires) ; l'onglet Évolution du panneau (probable au ton d'attention, confirmée au ton de
violation, forme et mot) ; le bloc géré d'`AGENTS.md`.

## Alternatives écartées

- **`statrs`** : son Mann-Whitney exact est faux (queues inversées, vérifié en 8.1).
- **Comparer à la seule médiane de la référence (test de Student, seuil fixe)** : la spec décide
  Mann-Whitney et la MAD, robustes au bruit.
- **Une régression confirmée par n'importe quel lot suivant** : un lot sur un autre commit
  mesure un autre code.
- **Relancer automatiquement à chaque `timonier test`** : le développeur choisit `--runs`.

## Conséquences

- Il faut des runs sur la branche principale avant qu'une branche se compare : le rapport le dit.
- Une fonction ralentie de beaucoup est vue dès 5 runs contre 5 ; une petite dérive demande plus
  de runs ou n'atteint pas 5 %.
- Les sorties des tests (JUnit, rapports) doivent être ignorées par git, comme `target/` : sinon
  chaque run après le premier voit un arbre modifié et sort de la référence (Q77).
- Des centaines de mesures sont jugées chacune à p < 0,01, sans correction pour comparaisons
  multiples, comme la spec le décide : quelques « probables » parasites sont attendus, que le
  lot suivant ne confirme pas (relecture n° 1, m7).
