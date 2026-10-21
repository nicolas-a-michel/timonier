# 0046 — Les compteurs d'instructions CPU

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Seuil de régression » (instructions CPU par compteurs matériels quand c'est
  possible, temps réel affiché) ; Q70 ; ADR 0045 (relais mesureur), 0043 (empreinte) ; tâches
  PLAN.md 8.1, 8.6, 8.9

## Contexte

La spec détecte les régressions d'abord sur le nombre d'instructions CPU, moins bruité que le
temps. Sous Linux, `perf_event_open` les compte, mais Ubuntu et Debian règlent
`perf_event_paranoid` à 4 : sans CAP_PERFMON, c'est refusé (vérifié le 2026-10-21 : `EACCES`).
À 2 ou moins, compter les instructions utilisateur de ses propres processus est permis. Sous
macOS, la voie publique (`proc_pid_rusage`) ne compte qu'un processus sans ses enfants ; `kpc`
est privé et demande root. Ta réponse Q70 : repli sur le temps CPU, et `timonier doctor` dit
comment les activer.

## Décision

**Sous Linux**, le relais mesureur (ADR 0045) ouvre, avant de lancer la commande, un compteur
d'instructions sur lui-même avec `perf-event` 0.4.9 (MIT OR Apache-2.0, vérifiée le 2026-10-21 ;
`Builder::new()`, `observe_self`, `inherit(true)`, `exclude_kernel(true)`, `exclude_hv(true)`,
`build`, `Counter::enable`, `read`) : `inherit` compte aussi tous les descendants, ajoutés à
leur fin. Il le lit après avoir attendu l'enfant.

**Ce que comptent les instructions** : le run entier. Un processus de test ne peut pas être
isolé de l'extérieur ; par test et par composant, la détection compare les durées (JUnit,
spans), avec la même méthode statistique.

**Repli** (Q70) : compteur refusé, ou macOS, le run compte en temps CPU (utilisateur + système,
`getrusage`), et le dit : `run` garde la source (ADR 0043), la vue et `get_perf` l'affichent,
l'empreinte de la machine l'inclut (changer de source réinitialise la référence). `timonier
doctor` signale un refus et donne la commande, sans la lancer : `sudo sysctl
kernel.perf_event_paranoid=2`, et la ligne d'un fichier de `/etc/sysctl.d/` pour la garder.

## Alternatives écartées

- **`perf-event2`** : pas de version depuis 2024, et sa bibliothèque porte le même nom.
- **`perf-event-open`** : récente, peu utilisée.
- **Lancer `perf stat`** : un outil de plus à installer, et le même refus.
- **Instructions par test** (un compteur par processus de test) : les processus naissent dans
  le runner, sans qu'on sache leurs PID à temps.

## Conséquences

- Sur une Ubuntu par défaut, la détection tourne en temps CPU tant que le réglage n'est pas
  fait.
- Le temps réel reste affiché dans tous les cas (spec).
