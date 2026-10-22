# 0058 — Le coût de l'IA : la télémétrie de Claude Code, reçue sur un port de la machine

- Statut : Accepté (2026-10-22, Phase 9 validée)
- Date : 2026-10-21
- Phase : 9
- Liens : spec « Steering » (coût de l'IA) et « Technical decisions » (mesurer le coût de l'IA) ;
  Q7, Q85 ; ADR 0033 (init), 0048 (récepteur OTLP), 0057 (sécurité locale) ; tâche PLAN.md 9.3

## Contexte

La spec décide la source : la télémétrie OpenTelemetry native de l'agent, pointée vers le
récepteur du démon par `timonier init` ; sans source, « non mesuré », jamais une estimation.
Claude Code n'active sa télémétrie que depuis les réglages utilisateur, les réglages gérés ou
l'environnement (code.claude.com, « Monitoring », lu le 2026-10-21) : un projet ne peut pas
l'allumer, ni choisir sa destination. Les variables sont donc les mêmes pour tous les projets
de la machine, alors que chaque projet a son démon, et que les ports du récepteur des runs
changent quand 4317 et 4318 sont pris (ADR 0048). Q85 : `init` écrit ces variables dans
`~/.claude/settings.json`, avec l'accord du développeur.

## Décision

- **Source** : les événements `claude_code.api_request` de Claude Code (une requête à l'API :
  `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens`, `cost_usd`,
  `model`, `query_source`, `session.id`, `event.timestamp`), en logs OTLP sur HTTP. Les
  métriques ne sont pas lues : un événement porte son heure, ce qui permet de l'attribuer.
- **Port de la machine** : `127.0.0.1:4319`, fixe, à part des ports des runs. `init` écrit dans
  le bloc `env` de `~/.claude/settings.json` : `CLAUDE_CODE_ENABLE_TELEMETRY=1`,
  `OTEL_LOGS_EXPORTER=otlp`, `OTEL_EXPORTER_OTLP_LOGS_PROTOCOL=http/protobuf`,
  `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT=http://127.0.0.1:4319/v1/logs`. Les variables par signal
  laissent intacte une configuration des métriques ou des traces que le développeur aurait
  déjà. Si l'une de ces quatre variables y vaut déjà autre chose, `init` n'y touche pas et le
  dit. `--dry-run` montre le changement ; la machine est dite à part du projet, et rien de son
  chemin n'entre dans `.timonier/init.json`.
- **Un seul démon reçoit pour la machine** : chaque démon essaie de prendre le port au démarrage,
  puis toutes les 60 s tant qu'il ne l'a pas. Celui qui le tient garde chaque requête de Claude
  Code dans `<données>/agents/claude/<session.id>.jsonl` (`$TIMONIER_DATA_DIR`, sinon
  `$XDG_DATA_HOME/timonier`, sinon `~/.local/share/timonier`, comme les commandes approuvées de
  l'ADR 0057). Une ligne : heure, modèle, `query_source`, les quatre comptes de tokens, le coût.
  Les fichiers de plus de 90 jours sont effacés. Les autres événements sont ignorés.
- **Attribution** : le hook d'arrêt de Claude Code reçoit son `session_id` sur stdin et le passe
  au démon de son projet (`agent/stop`). Le démon l'inscrit dans les sessions Timonier d'un agent
  Claude ouvertes, ou fermées depuis le dernier arrêt de cette session de Claude Code
  (`agent_sessions` du journal). Les tokens d'une session Timonier sont ceux des requêtes de ses
  sessions de Claude Code dont l'heure tombe entre son début et sa fin. Le démon les recalcule à
  chaque arrêt, pour les sessions ouvertes et celles fermées depuis moins d'une heure (les
  événements arrivent par lots de 5 s), et les écrit dans le journal (`usage`).
- **Par tâche** : la somme de ses sessions. Une session qui sert plusieurs tâches compte pour
  chacune, et la tâche dit que ses tokens sont partagés : le journal ne sait pas comment la
  session s'est répartie, et Timonier n'invente pas de clé de partage.
- **Non mesuré** : une session sans session de Claude Code inscrite, ou dont les requêtes n'ont
  pas été reçues, a `usage` absent ; les vues disent « non mesuré ».
- **Les tokens d'abord** : le coût en dollars de Claude Code est montré comme une indication.
- **`uninit`** retire les quatre variables quand plus aucun projet de la machine n'en a besoin :
  `<données>/agents/projects.json` liste les racines des projets configurés ainsi.

## Alternatives écartées

- **Les ports des runs (4317/4318)** : ils changent quand ils sont pris, et chaque projet a les
  siens ; la destination de Claude Code est fixe pour toute la machine.
- **Un port par projet** : les réglages utilisateur valent pour tous les projets ; le projet ne
  peut pas choisir.
- **Les métriques `claude_code.token.usage`** : agrégées sur 60 s, sans heure précise par
  requête ; une session Timonier ouverte et fermée dans une même minute ne s'y retrouve pas.
- **Lire les transcripts de Claude Code** : format non documenté ; écarté par Q85.
- **Partager les tokens d'une session entre ses tâches à parts égales** : une clé inventée.

## Conséquences

- Les sessions de Claude Code de toute la machine écrivent sur le port, même hors d'un projet
  Timonier ; leurs lignes restent dans le dossier de données et sont effacées après 90 jours.
- Le port n'exige pas de jeton : un processus local peut y ajouter des requêtes fausses. Le coût
  est une information, il ne décide rien (même modèle de confiance que l'ADR 0057).
- Les requêtes faites quand aucun démon Timonier ne tourne sont perdues : la session dit ce
  qui a été reçu.
- Claude Code lit ses réglages au démarrage : après `init`, il faut le relancer.
- Seul Claude Code est mesuré ; Copilot, Cursor et Codex restent « non mesurés » tant qu'ils
  n'exportent pas leurs tokens.
