# 0041 — Les outils `get_intended_architecture` et `get_drift`, et l'ancrage

- Statut : Accepté (2026-10-21, validation de la Phase 7 par l'humain)
- Date : 2026-10-21
- Phase : 7
- Liens : spec « Interface MCP » (Intention), « Comparer à l'intention » ; Q6, Q67, Q68 ;
  ADR 0027 (serveur MCP), 0028 (en-tête), 0033 (`actual.json`), 0039, 0040 ; tâches PLAN.md
  7.5, 7.6

## Contexte

La spec nomme deux outils de l'intention : `get_intended_architecture` et `get_drift`. Q6 et Q67 :
l'agent lit l'image du croquis par le premier. La comparaison ancre chaque boîte dessinée sur un
composant réel « par nom et emplacement », sans écrire dans le fichier ; ta réponse Q68 fixe la
règle. La carte (7.6) fera la même comparaison.

## Décision

**`get_intended_architecture`** (lecture seule ; argument `image`, vrai par défaut) : les couches,
les composants (nom, couche, rôle), les arêtes et leur politique, les notes, les budgets ; où est
chaque forme (`places.shapes`, rectangles) et chaque trait (`places.strokes`, ADR 0040) ; et
l'image du croquis comme contenu `image/png` de MCP, après le texte. Les points des traits ne sont
pas rendus : l'image les montre. Rien de dessiné : `drawn: false` et ce qu'il faut faire.

**`get_drift`** (lecture seule, sans argument) : `anchors` (chaque composant dessiné, la boîte
réelle qu'il désigne ou `null`, et le nombre de candidats), `missing`, `forbidden` (chaque arête
interdite franchie et les dépendances réelles qui la franchissent), `required` (chaque arête
requise sans dépendance), `unchecked` (une extrémité ne désigne rien). Calculée par
`timonier_core::drift` depuis `intent.json` et l'architecture de la tête (`actual`, comme
`actual.json`), à chaque appel ; jamais écrite.

**Ancrage** (Q68) : les noms se comparent en minuscules, sans espaces, tirets ni soulignés (les
lettres accentuées et non latines restent : « Données » n'est pas « Donnes ») ; à défaut, un
paquet dont le nom finit par le nom dessiné (« Daemon » ↔ `timonier-daemon`). Un seul candidat :
lui. Plusieurs : le plus proche dans l'arbre des composants déjà ancrés de la même couche, sinon
le plus haut, puis le premier par ID. Une boîte sans nom ne s'ancre pas.

**Ce qu'une arête désigne** : un composant, sa boîte réelle et tout ce qu'elle contient ; une
couche, les boîtes de ses composants ancrés (une arête entre couches vaut pour tout leur
contenu). Une dépendance compte si elle part de l'une et arrive dans l'autre ; celle dont les deux
bouts sont des deux côtés est intérieure, pas une traversée. Les dépendances sont celles de la
carte, agrégées entre les boîtes les plus profondes.

**Le bloc d'`AGENTS.md`** dit de lire l'intention avant de changer la structure et de vérifier la
dérive après. `doctor` dit quand le tableau blanc manque.

## Alternatives écartées

- **Un ancrage écrit dans `intent.json`** : la spec l'interdit, le développeur est seul auteur.
- **Le nom exact seulement** : écarté par Q68.
- **L'image en ressource MCP à part** : un appel de plus ; le contenu image d'un résultat d'outil
  est lu par les agents qui voient.

## Conséquences

- Un agent sans vision a la structure, les places et la dérive ; l'image ne fait qu'ajouter.
- Une dérive se calcule sur le graphe de la tête : avant la fin de l'indexation, elle peut dire
  « manquant » à tort.
