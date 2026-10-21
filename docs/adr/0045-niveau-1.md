# 0045 — Niveau 1 : mesurer un run de tests sous Linux et macOS

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances » (niveau 1 : cgroups v2 / procfs, JUnit XML) ; H8 ; ADR 0034
  (runners de tests et JUnit XML), 0043 (stockage), 0046 (compteurs) ; tâches PLAN.md 8.1, 8.2,
  8.7

## Contexte

`timonier test -- <commande>` doit mesurer la commande et tous ses descendants (un runner lance
souvent un processus par binaire de test) : durée, CPU, mémoire max, I/O. Vérifié le 2026-10-21
sur la machine de développement (Ubuntu, systemd 259) : sans root, un scope systemd utilisateur
reçoit les contrôleurs cpu et memory, pas io ; créer un sous-cgroup dans le scope courant
échoue (règle des cgroups v2 sur les nœuds non feuilles) ; `getrusage(RUSAGE_CHILDREN)` dans un
gros processus rapporte la mémoire du parent (119 Mo pour un `ls`). macOS n'a pas de cgroups.

## Décision

**Un relais mesureur.** `timonier test` se relance lui-même en relais caché (`timonier
__measure`), petit processus dont la commande est le seul enfant. Le relais lance la commande
avec ses entrées et sorties, attend sa fin, mesure, écrit le résultat sur un tube vers
`timonier test`, et rend son code de sortie. Ce que le relais consomme lui-même (quelques Mo, peu
de CPU) est compté avec la commande : c'est dit dans les vues.

**Partout (Linux et macOS)** : `getrusage(RUSAGE_CHILDREN)` dans le relais, une fois l'enfant
attendu, par `nix` 0.31.3 (MIT, vérifiée le 2026-10-21 ; `getrusage`, `user_time`,
`system_time`, `max_rss`, `block_reads`, `block_writes`). Il donne le temps CPU utilisateur et
système de l'arbre, la mémoire max du plus gros processus de l'arbre, et les blocs lus et écrits.
La durée est l'horloge monotone du relais.

**Sous Linux, en plus** : si `systemd-run --user` existe et que le gestionnaire utilisateur
délègue cpu et memory, le relais tourne dans un scope transitoire (`systemd-run --user --scope
--quiet --collect`). Il lit son propre cgroup (`/proc/self/cgroup`) : `memory.peak`, la mémoire
max de tout l'arbre, et `cpu.stat` ; `io.stat` si io est délégué. Il échantillonne
`memory.current` et `cpu.stat` toutes les 100 ms pour la courbe des ressources sous la timeline
(8.7). Le temps CPU du run vient alors de `user_usec` et `system_usec` de `cpu.stat`, qui
comptent aussi les descendants détachés (un démon lancé par un test), que `getrusage` ignore
(relecture n° 1, m8). Les échantillons sont datés depuis le lancement de la commande ;
`timonier test` les ramène au début du run, l'origine des spans, avec l'heure murale du relais
(`Measured::started_at_ms`, relecture n° 1, I3). Sans systemd ni délégation (conteneur, certaines CI), seul `getrusage` reste, et le run le
note.

**Sous macOS** : `getrusage` seul ; ni courbe des ressources ni mémoire de l'arbre entier. Le
run le note, la vue le dit (« mémoire max : du plus gros processus »).

**La durée par test** vient du JUnit XML que le runner écrit (ADR 0034), lu par `junit-parser`
1.7.0 (BSD-2-Clause, vérifiée le 2026-10-21) : il lit une racine `testsuites`, un `testsuite`
seul ou `testrun`, et le fichier et la ligne de chaque cas quand le runner les donne.
`quick-junit` est écartée : sa lecture est « expérimentale » et refuse un `testsuite` seul.

## Alternatives écartées

- **Mesurer depuis `timonier test` ou le démon** : `getrusage` y mélangerait leur propre mémoire
  et les autres enfants (serveurs de langage du démon).
- **Écrire directement sous `app.slice`** : ça marche sans root, mais rompt le contrat de
  délégation de systemd ; `systemd-run --scope` est la voie prévue.
- **`systemd-run --wait --pipe` (service)** : le service n'a ni notre environnement ni notre
  répertoire.
- **`proc_pid_rusage` sous macOS** : un seul processus, sans ses enfants.
- **`wait4` par enfant** (crate `wait4`) : pas d'I/O, et rien de plus que `getrusage` dans le
  relais.

## Conséquences

- Les mêmes chiffres de base existent sur les deux plateformes ; Linux avec systemd en a plus.
- I/O : blocs lus et écrits partout, octets par `io.stat` seulement si io est délégué.
- Windows (H8) n'est pas couvert.
