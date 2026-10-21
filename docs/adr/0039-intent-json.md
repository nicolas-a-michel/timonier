# 0039 — `.timonier/intent.json` : le format du tableau blanc et son enregistrement

- Statut : Accepté (2026-10-21, validation de la Phase 7 par l'humain)
- Date : 2026-10-20
- Phase : 7
- Liens : spec « Tableau blanc » (fichier `intent.json`), « Initialisation » (6) ; Q57, Q66 ;
  ADR 0019 (protocole), 0029 (formats du suivi), 0033 (`init`), 0037, 0038 ; tâche PLAN.md 7.3

## Contexte

La spec fixe le fichier : un seul auteur, le développeur ; sémantique et géométrie séparées
(`shape` ne sert qu'au rendu) ; IDs locaux stables ; versionné dans le dépôt ; un exemple. Le plan
demande un ordre stable pour des diffs git lisibles et un schéma JSON. Ta réponse Q66 : le dessin
doit toujours persister, chaque geste est enregistré tout seul dans `intent.json`. Q57 : `init`
crée le tableau vide quand le format existe.

## Décision

**Format** (`timonier_core::intent`, version 1, `docs/schemas/intent.schema.json`) : `layers`
(`id`, `name`), `components` (`id`, `name`, `layer`, `role`), `edges` (`id`, `src`, `dst`,
`kind: "depends_on"`, `policy`), `notes` (`id`, `text`, `attached_to`), `budgets` (`id`,
`attached_to`, et parmi `p50_ms`, `p95_ms`, `memory_mb`, `allocations`, `calls` celles qui sont
posées), puis `sketch` : `shapes` (le coin, et la taille des couches et composants, par ID),
`strokes` (les traits, dans l'ordre du dessin) et `next` (le prochain numéro de chaque préfixe,
pour ne jamais réutiliser un ID). Les clés en `snake_case` comme dans l'exemple de la spec ; une
note budget a son tableau, `budgets`, pour que l'agent la lise sans la deviner. La couche d'un
composant est écrite, calculée depuis le dessin (Q62).

**Ordre et mise en page** : chaque liste par numéro d'ID ; les traits dans l'ordre du dessin ;
les champs dans l'ordre du type ; deux espaces ; les points d'un trait sur une seule ligne, pour
qu'un trait ne fasse qu'une ligne de diff. Le même tableau donne toujours le même texte.

**IDs** : le tableau blanc numérote ce qu'il crée (`C-1`) ; un fichier écrit à la main peut
nommer ses IDs comme l'exemple de la spec (`C-parser`, `L-domain` : après le tiret, jusqu'à 64
lettres ASCII, chiffres, tirets et soulignés, une lettre ou un chiffre d'abord ; des chiffres seuls
sont un numéro, au plus 2⁵³ − 1), ta réponse Q69. Les IDs nommés viennent après les numérotés, par
nom. Le démon et la page appliquent les mêmes règles (bornes des limites à 10¹², longueurs en
caractères), vérifiées par un jeu de fichiers commun (`crates/timonier-core/tests/intent/`) ; un
fichier que la page ne sait pas dessiner est signalé, jamais montré vide puis écrasé.

**Validation** : version, IDs au bon préfixe et uniques, références vers ce qui peut les prendre
(une flèche joint deux composants ou couches, une note s'attache à un composant, une couche ou une
flèche), textes et nombres bornés, pas de champ inconnu. Un fichier qui ne tient pas est refusé en
entier, avec la raison.

**Qui écrit** : le démon, seul, par `intent/write` (vérifié, mis en ordre, écrit seulement s'il a
changé, par un fichier temporaire et un renommage) ; `intent/read` rend le tableau, rien, ou la
raison d'un fichier illisible. Ce sont des méthodes de l'extension : aucun outil MCP n'écrit le
tableau (les outils de 7.5 le lisent). Le démon ne vérifie pas qui appelle `intent/write` : un
programme qui ouvre le socket (le tien, en 0700) le pourrait, comme il pourrait écrire le fichier ;
le proxy MCP ne relaie que `tools/list` et `tools/call`. Protocole version 9.

**Quand** (Q66) : la page envoie le tableau après chaque geste ; l'extension l'écrit 250 ms après le
dernier, réessaie toutes les 2 s tant que le démon ne répond pas (pas quand il refuse le tableau),
écrit ce qui reste à la fermeture du panneau, et dit à la page « Enregistré » ou « Non enregistré :
raison ». À l'ouverture, la page reçoit le fichier ; si le fichier change ailleurs (un
`git checkout`, une édition à la main), la page le reçoit et repart de lui, avec une nouvelle
histoire d'annulation, sauf si un geste attend d'être écrit. Un fichier illisible n'est pas
écrasé : la page le dit, et attend « Repartir d'un tableau blanc vide ». L'état de la webview
(`setState`) ne sert plus : l'extension garde le dernier tableau pour une page rechargée.

**`init`** (Q57) : écrit le tableau vide s'il n'y en a pas ; `uninit` le reprend s'il est encore
vide, et le garde, comme le reste du suivi, dès qu'il a été dessiné (sauf `--purge`).

## Alternatives écartées

- **Écrire depuis l'extension** (`workspace.fs`) : deux écrivains de `.timonier/` et la validation
  en TypeScript ; le démon lira le fichier pour l'agent (7.5) avec les mêmes types.
- **Un brouillon gardé à part et « Enregistrer »** : écarté par ta réponse Q66.
- **Les budgets parmi les notes** : l'agent devrait distinguer deux sortes dans une liste.
- **Un JSON joli ordinaire** : un trait de 200 points ferait 800 lignes de diff.

## Conséquences

- Le tableau blanc et ce que lit l'agent sont toujours le même fichier, au geste près.
- La mise à jour du démon est nécessaire : une extension de protocole 9 refuse un démon de 8
  (`cargo install --path crates/timonier-cli`).
- **Complète l'ADR 0019** sur un point (relecture n° 1 de la Phase 7, m8, et n° 5, I-1) :
  `daemon/shutdown` est admis avant `initialize`, toute autre méthode reçoit encore `-32000`. Un
  client d'une autre version, que le démon refuse (`-32001`), peut ainsi l'arrêter, pour que le
  démarrage suivant lance la version installée ; `timonier daemon stop` s'en sert, et la CLI dit
  quoi faire face à un démon plus ancien, qui ne l'admet pas. La garde reste celle de l'ADR 0019 :
  le socket est dans un dossier de l'utilisateur, en 0700 ; ce client pouvait déjà arrêter le
  démon après la poignée de main.
- Un dessin en cours se voit dans `git status` : c'est ton fichier, tu le commites quand tu veux.
