# 0014 — Vitesse 2 : affinage LSP en arrière-plan

- Statut : Accepté (2026-10-16, validation de la phase par l'humain) ; complété par 0022
  (« Ce qui est vérifié » et « Dépendants », Q19 ; « Après un redémarrage » : un appel vers un
  fichier changé pendant l'absence ne tient plus, et ce que l'affinage devait survit dans la
  base)
- Date : 2026-10-16
- Phase : 3
- Liens : spec « Indexation et modèle de graphe », « Décisions techniques » (trois vitesses,
  granularité des `Calls`) ; Q5 ; ADR 0009, 0013 ; tâche PLAN.md 3.6

## Contexte

La spec veut un affinage LSP paresseux :
- en file basse priorité, en commençant par ce qui est affiché ;
- quand on ouvre un fichier ;
- pour les fichiers dépendants, quand une signature publique change.

Les arêtes fonction → fonction ne viennent que du LSP ou de SCIP, et chaque fichier mémorise
« vérifié par LSP au hash X ». L'humain a choisi (Q5) une interface commune à deux
implémentations : d'abord le relais des serveurs de l'IDE par l'extension, plus tard des
serveurs lancés par le démon. Le relais passe par le lien démon ↔ extension, qui arrive en
Phase 4. La Phase 3 livre donc l'interface, la file et la conversion en observations, testées
avec un double conforme au protocole.

## Décision

**Interface** : un trait synchrone `LanguageServer`, appelé depuis un fil dédié. Il ne
porte que ce que l'affinage utilise :
- `outgoing_calls(fichier, position)` ;
- c'est `textDocument/prepareCallHierarchy` puis `callHierarchy/outgoingCalls` en LSP, ou
  `vscode.prepareCallHierarchy` et `vscode.provideOutgoingCalls` dans l'IDE.

Les positions sont celles du modèle : lignes à partir de 0, colonnes en octets UTF-8
(ADR 0008). L'implémentation convertit : l'extension depuis les colonnes UTF-16 de VSCode, un
serveur lancé par le démon en négociant `positionEncoding`.

**Ce qui est vérifié** : les `Calls`, et eux seuls.
- Pour chaque fonction et méthode du fichier, on demande ses appels sortants à la position de
  son nom. L'extraction relève cette position.
- Chaque appelé est rattaché au plus petit symbole de son fichier qui contient sa position,
  d'après la tête du graphe. Un appelé hors du dépôt (bibliothèque standard, dépendances) est
  écarté.
- Les observations partent en provenance `Lsp`, avec `verifying([Calls])` et `computed_on` au
  hash lu au moment de la demande. Si le fichier change entre-temps, le stockage les ignore
  (ADR 0009).
- Les `References`, `Implements` et `Inherits` restent ceux de tree-sitter. Les vérifier
  demanderait une requête `definition` par site de référence, ce qui est trop coûteux en masse
  (spec).

**File d'affinage**, par fichier, sans doublon. Trois priorités, servies dans cet ordre :
1. affiché sur la carte ;
2. ouvert dans l'éditeur ;
3. dépendant.

Une nouvelle demande peut relever la priorité d'un fichier déjà en file. Il n'y a pas de passe
d'arrière-plan sur tout le dépôt : l'affinage reste paresseux, comme le veut la spec.

**Dépendants** : quand une passe d'indexation change la signature d'un symbole, ou en ajoute
ou en retire un, les fichiers qui citent son nom sont marqués. Seuls ceux qui ont déjà été
vérifiés retournent en file, avec la priorité « dépendant ». La comparaison porte sur toutes
les signatures, pas seulement les publiques : tree-sitter ne sait pas la visibilité de façon
générique. Cela marque un peu trop de fichiers, mais n'en oublie aucun.

**Après un redémarrage** : la vérification vit dans la base, et l'état d'affinage en est relu.
- Chaque fichier vérifié revient comme vérifié, avec la priorité « dépendant ». Ses appels
  sont gardés en détail ou compactés (ADR 0018). Les dépendants et le budget fonctionnent donc
  comme avant le redémarrage.
- Un fichier dont les appels partent de symboles qui n'existent plus perd sa vérification et
  retourne en file. C'est le cas d'une crate renommée pendant l'absence.
- Quand les paquets changent pendant que le démon tourne, les identifiants peuvent changer
  aussi : toutes les vérifications tombent, et les fichiers vérifiés retournent en file.

## Alternatives écartées

- **Vérifier aussi les références** par `textDocument/definition` sur chaque site : un fichier
  de taille moyenne compte des centaines de sites. Ce sera à revoir avec SCIP (v2).
- **`textDocument/references` sur chaque symbole** (appels entrants) : il faut interroger le
  fichier appelé pour connaître les appelants, ce qui inverse la logique « on affine ce qu'on
  regarde ».
- **Une passe d'arrière-plan sur tout le dépôt** : la spec la veut paresseuse, et la mémoire
  du serveur de l'IDE est partagée avec l'utilisateur.
- **Un trait asynchrone** : le cœur de l'indexation est synchrone. Le relais asynchrone de la
  Phase 4 attendra sa réponse sur le fil d'affinage.

## Conséquences

- Les appels faits hors de toute fonction disparaissent quand le fichier est vérifié : code de
  niveau module en TypeScript, initialiseurs de constantes en Rust. La fusion écarte les
  `Calls` tree-sitter d'un fichier dès que le LSP vérifie ce type d'arête (ADR 0009). Si la
  carte en souffre, il faudra relever la position de ces sites à l'extraction.
- Le relais de l'IDE (Phase 4) n'aura qu'à implémenter `LanguageServer`. Un serveur lancé par
  le démon, pour la CI, aussi.
