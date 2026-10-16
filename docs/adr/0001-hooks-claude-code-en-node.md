# 0001 — Hooks Claude Code écrits en Node.js, sans dépendance

- Statut : Accepté (2026-10-14, validation de la phase par l'humain)
- Date : 2026-10-14
- Phase : 0
- Liens : prompt de démarrage, Phase 0 ; documentation officielle consultée le 2026-10-14 :
  code.claude.com/docs/en/hooks, /hooks-guide, /settings, /permissions, /skills, /sub-agents

## Contexte

La Phase 0 demande quatre comportements automatiques :

- formater chaque fichier édité ;
- refuser toute modification de `.timonier/intent.json` ;
- lancer des vérifications rapides en fin de tour, avec un rappel quand `PLAN.md` n'a pas suivi
  les changements de code ;
- afficher la phase en cours au démarrage.

Les hooks reçoivent du JSON sur l'entrée standard et répondent en JSON sur la sortie. Ils
doivent tourner sur Linux et macOS, les deux cibles de la CI, et être testables.

## Décision

**Langage et tests**

- Scripts en modules Node.js (`.claude/hooks/*.mjs`), sans dépendance npm, avec des helpers
  communs dans `lib.mjs`.
- Tests avec `node:test` (`hooks.test.mjs`).
- Node est de toute façon requis par l'extension. Vérifié en local : Node 24.18.

**Invocation** en forme exec (`"command": "node", "args": ["${CLAUDE_PROJECT_DIR}/…"]`), que
la documentation recommande pour les chemins contenant un placeholder.

**Événements utilisés**

| Événement | Rôle |
| --------- | ---- |
| `SessionStart` | `systemMessage` d'une ligne pour l'humain, `additionalContext` avec la liste des tâches restantes pour Claude |
| `UserPromptSubmit` | Marque le début du tour. Le tour reste ouvert jusqu'au `Stop`, pour que les prompts injectés en cours de tour ne déplacent pas son début |
| `PreToolUse` (`Edit\|Write\|NotebookEdit\|Bash`) | Décision `deny` via `hookSpecificOutput.permissionDecision`, avec `onFailure: "block"` : un hook en panne bloque au lieu de laisser passer |
| `PostToolUse` (`Edit\|Write`) | Formatage avec `rustfmt` ou `prettier`. Un échec remonte à Claude comme contexte et ne bloque pas |
| `Stop` | Un échec de `cargo check` ou du typecheck bloque une seule fois par tour (`decision: "block"`). Le rappel `PLAN.md` passe par `additionalContext`, le canal documenté pour un retour qui n'est pas une erreur |

**Détection des fichiers changés pendant le tour** : `git status` plus une date de
modification postérieure au début du tour. Les éditions faites par Bash sont ainsi couvertes.

**Défense en profondeur** : la règle de permission `deny` `Edit(**/.timonier/intent.json)`
s'ajoute au hook. Les règles `Edit` s'appliquent à tous les outils d'édition.

**État** par session dans `.claude/state/`, ignoré par git et purgé après sept jours.

## Alternatives écartées

- **Bash + jq** : jq n'est pas garanti sur macOS ni en CI, le quoting est fragile et les
  scripts sont plus difficiles à tester.
- **Python** : absent par défaut sur macOS sans les Command Line Tools, et le projet n'en a
  pas besoin par ailleurs.
- **Suivre les éditions avec `PostToolUse` seul** : rate les fichiers modifiés par Bash
  (`sed`, générateurs, `cargo fmt`).
- **Bloquer l'arrêt pour le rappel `PLAN.md`** : force une continuation présentée comme une
  erreur, même quand aucune tâche n'a changé d'état.

## Conséquences

- `node` doit être dans le `PATH` de Claude Code.
- `onFailure: "block"` exige Claude Code ≥ 2.1.295 (2.1.299 installé).
- Le typecheck de fin de tour dépend d'un script `typecheck` à la racine du `package.json`,
  à créer en Phase 1.
- Le formatage prettier reste inactif tant que prettier n'est pas installé localement
  (Phase 1).
- Les hooks ne visent pas Windows, ce qui est cohérent avec les cibles de la CI.
- Le rappel `PLAN.md` peut se déclencher quand un tour touche du code hors plan. C'est
  accepté : une ligne de réponse suffit.
