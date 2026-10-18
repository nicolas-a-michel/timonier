# 0022 — Vérifier les usages et les dépendants par le serveur de langage

- Statut : Accepté (2026-10-18, validation de la phase par l'humain)
- Date : 2026-10-17
- Phase : 4
- Liens : spec « Indexation et modèle de graphe » (trois vitesses) ; Q5, Q19 ; complète
  l'ADR 0014 (« Ce qui est vérifié », « Dépendants ») et l'ADR 0015 (le retour de l'arbre de
  travail après un commit) ; ADR 0009, 0012, 0018, 0019 ; tâche PLAN.md 4.5

## Contexte

L'ADR 0014 ne vérifie par le LSP que les `Calls`. Les `References`, `Implements` et `Inherits`
restaient ceux de tree-sitter, car une requête « définition » par site d'usage coûterait trop :
des centaines par fichier. Les dépendants d'une signature changée sont trouvés par leur nom,
ce qui en marque trop.

L'humain a gardé le critère d'origine de la tâche 4.5 (Q19, réponse b, 2026-10-17) :
« définitions et références obtenues via l'API VSCode et reçues par le démon ». Il demande
aussi « on optimise le code à fond ». Il faut donc vérifier les autres usages, et utiliser les
références, sans le coût que l'ADR 0014 redoutait.

Vérifié le 2026-10-17 dans VSCode 1.142.0 : les commandes intégrées
`vscode.executeDefinitionProvider` et `vscode.executeReferenceProvider` prennent un `Uri` et
une `Position`. La première rend des `Location` ou des `LocationLink`, la seconde des
`Location`.

## Décision

**Définitions : les autres usages d'un fichier.** L'affinage d'un fichier vérifie aussi ses
`References`, `Implements` et `Inherits`.
- Une seule question par chemin cité distinct, pas par site. L'extraction regroupe déjà les
  sites d'un même chemin, comme la résolution tree-sitter. Elle relève maintenant la position
  du nom au premier site (`ReferenceFact.at`), dans le texte du nœud : `Tr` dans `Tr<Q>`, `C`
  dans `a::b::C`. La version de l'extracteur passe à 3, ce qui invalide le cache des faits.
- Les chemins ne sont pas regroupés davantage. Deux chemins qui partagent leur premier segment
  peuvent être définis dans deux fichiers : une réexportation par `*` entre eux, un `impl`
  dans un autre fichier. La réponse pour l'un remplacerait alors une arête juste de l'autre
  (relecture n° 3, I-1).
- Ne sont pas demandés :
  - un chemin que tree-sitter résout dans le fichier même : il ne fait pas d'arête entre
    fichiers ;
  - un chemin que tree-sitter mène dans une dépendance hors du dépôt ;
  - un chemin que tree-sitter ne résout pas et que le pack déclare hors de tout dépôt : les
    types de la bibliothèque standard de Rust et les globales de TypeScript. La liste est
    dans la nouvelle section `[lsp] outside` du pack, optionnelle (ADR 0012, format 1
    inchangé) ;
  - un nom non résolu d'un seul caractère : `_` ou un paramètre de type comme `T`, déclarés
    dans l'élément même.
- Toutes les questions d'un fichier partent ensemble, en une requête `lsp/definitions`.
  Chaque question dit si tree-sitter place la définition dans un autre fichier par un import
  ou un chemin (`elsewhere`). Un nom que tree-sitter trouve par son seul nom n'en est pas : un
  « ici » du serveur l'y corrige, un paramètre générique par exemple.
  L'extension interroge VSCode en parallèle, huit à la fois au plus. L'attente cède à tout
  autre travail, comme pour les appels (ADR 0019, « Relais »).
- Une définition dans le dépôt donne l'arête : de fichier à fichier pour `References`, comme
  tree-sitter ; du symbole source au symbole qui contient la définition pour `Implements` et
  `Inherits`. Les observations partent en provenance `Lsp`. Elles vérifient les quatre sortes
  (`verifying([Calls, References, Implements, Inherits])`), et celles de tree-sitter pour ce
  fichier tombent (ADR 0009). Un fichier sans usage à demander les vérifie d'office : il n'a
  pas d'arête de ces sortes vers un autre fichier.
- **Une réponse douteuse ne remplace rien.** Pendant que le projet se charge, le serveur
  syntaxique de TypeScript répond seul, depuis le fichier demandé : un type importé y est
  « défini » à son import (vu dans VSCode 1.142). Donc :
  - l'extension redemande après 1, 2 puis 4 s les seules questions `elsewhere` qui ne
    reçoivent que des réponses dans le fichier, ou aucune. Mesuré : `area.ts` vérifié en 1 s ;
  - une définition dans le fichier même qui pointe sur l'import du nom est suivie, deux fois
    au plus, jusqu'à la déclaration ;
  - si une question `elsewhere` reste ainsi, le démon ne vérifie pas les usages du fichier :
    tree-sitter les garde. Seuls les appels sont vérifiés, et la raison est notée ;
  - aucune définition pour aucune question veut dire « pas de serveur, ou pas prêt » : c'est
    un échec, pas « défini nulle part ».
- Si les définitions échouent, l'affinage ne vérifie que les `Calls`, comme avant.
- La compaction (ADR 0018) ne réduit que les appels. Les autres arêtes vérifiées sont gardées
  telles quelles.
- Au redémarrage, une vérification ne tient plus si une extrémité a disparu (le symbole source
  ou cible d'un `Implements` ou d'un `Inherits`), ou si le fichier de destination d'une
  `References` ou d'un appel, compacté ou en détail, a disparu ou changé pendant l'absence :
  une définition a pu le quitter, ou une signature changer. Le démon en marche aurait cherché
  les dépendants de ce changement ; arrêté, il ne l'a pas vu. Le fichier retourne en file. Cela
  remplace, pour les appels en détail, la règle de l'ADR 0014, qui ne regardait que leurs
  extrémités (relecture n° 5, m-α).
- Quand le démon renonce à une question (il cède à un autre travail, ou le délai expire), il
  envoie `$/cancelRequest`, comme en LSP : l'extension cesse ses nouvelles tentatives.

**Références : les dépendants exacts.** Quand un fichier change la signature d'un symbole, ou
en ajoute un :
- le démon demande `lsp/references` à la position de son nom, au lieu de marquer tous les
  fichiers qui citent ce nom ;
- seuls les fichiers déjà vérifiés parmi ceux qui l'utilisent vraiment retournent en file,
  avec la priorité « dépendant » ;
- un fichier qui utilise un symbole cite son nom. La recherche n'est donc faite que si un
  fichier vérifié cite ce nom : sans lui, aucun dépendant ne peut revenir. Elle écarte, parmi
  ceux qui citent le nom, ceux qui ne l'utilisent pas ;
- ces recherches passent avant les dépendants, et après ce qui est affiché ou ouvert ;
- une recherche porte l'id du symbole. Le même symbole cherché de nouveau remplace la
  recherche qui attend : le contenu le plus récent, et les fichiers qui citent le nom dans
  l'une ou l'autre. Deux symboles restent deux recherches, même à la même position : une
  sauvegarde peut en mettre un autre où était le premier (relecture n° 5, I-β) ;
- une recherche faite sur un contenu qui a changé depuis (une autre sauvegarde) retrouve son
  symbole par son id, à sa position dans le contenu actuel. S'il a disparu, on retombe sur le
  marquage par nom (relecture n° 4, I-A) ;
- prudence : sans réponse (serveur absent, échec), ou si les utilisateurs nommés ne sortent
  pas du fichier, on retombe sur le marquage par nom. Le serveur peut ne pas savoir encore :
  aucun dépendant n'est manqué. VSCode compte la déclaration parmi les références : une liste
  vide est un échec. Sans client qui relaie, la recherche attend le suivant, comme un fichier
  (ADR 0019) ;
- un symbole retiré n'a plus de position : ses dépendants restent marqués par leur nom ;
- après un commit, l'arbre de travail revient sur les versions commitées le temps du gel
  (ADR 0015), et le gel fait tomber la vérification LSP des fichiers sales. Avant le gel,
  l'index est remis au niveau du disque : ce que git dit propre est gelé depuis l'index, et un
  fichier revenu à sa version commitée sans que le démon l'ait relu y serait gelé tel que
  l'index le tient (relecture n° 14, I14-1). Le parcours est refait aussi, comme si un fichier
  d'exclusion avait changé : un fichier que `.gitignore` saute désormais sort de l'index, un
  fichier qu'il ne saute plus y entre (relecture n° 15, I15-1). Coût mesuré par la relecture
  n° 15 (build release, tmpfs, cache chaud) : 18 ms de parcours et 66 ms de lecture et de
  hachage pour 10 000 fichiers ; 80 ms et 270 ms pour 40 000. Le retour se fait en deux temps
  (relecture n° 13, I13-1, I13-2) :
  - d'abord tel que l'index le tenait avant le gel, depuis la mémoire, sans lire le disque :
    les faits gardés des fichiers sales, et leur vérification gardée, dans un même lot. La
    file, le détail des appels et la dette n'ont pas bougé, puisque le gel ne touche que la
    base et la mémoire de l'indexeur : l'état d'avant le gel revient entier, et rien n'est
    demandé (relecture n° 12, I12-1) ;
  - puis le disque, relu comme tout changement : ce qui a changé depuis la dernière lecture de
    l'index est suivi, par rapport à ce que le serveur a vu, et non au commit (relecture
    n° 11, I11-1) ; un nouveau paquet suit le chemin ordinaire, qui fait tomber toutes les
    vérifications ;
  - si le premier temps échoue, le démon réindexe tout aussitôt depuis le disque, et les règles
    du redémarrage jugent ce qui reste vérifié (relecture n° 14, m14-1) ;
  - mesuré le 2026-10-17 sur le retour en deux temps, avec la sonde de coût de la relecture
    n° 12 et une copie de l'arbre de travail de Timonier (95 fichiers sales ; 10, 28 ou 138
    fichiers vérifiés ; le commit d'une note) : 0 question par commit, résultat retrouvé par la
    relecture n° 14. Le suivi complet en coûtait 221, 338 et 678 recherches, et 131, 539 et
    2 132 questions d'appels.

**Le contenu enregistré seulement** (relecture n° 5, I-α). Les serveurs de VSCode répondent
sur les documents ouverts. Un document modifié sans être enregistré, ou que VSCode n'a pas
encore relu après une écriture sur le disque, n'est pas le contenu que le démon a indexé. Sa
réponse serait pourtant enregistrée « vérifiée au hash X » du disque (spec, « Indexation et
modèle de graphe »).
- Un fichier source est un fichier du dépôt que lit un pack : le démon en donne les extensions
  à `initialize` (`sourceExtensions`). Une note, un `PLAN.md` en cours d'écriture ne changent
  aucune réponse (relecture n° 6, m6-2).
- L'extension ne répond pas, avec l'erreur `ContentModified` (-32801) de LSP, quand :
  - un fichier source a des modifications non enregistrées, au début de la question (elle
    refuse alors sans interroger ses serveurs) ou à la fin ;
  - un fichier source a changé pendant la question ;
  - un document que la question a lu n'est pas le fichier du disque : le fichier demandé, et,
    pour les appels et les définitions, ceux où pointent les réponses. Les références ne
    donnent que des fichiers, sans position : seul le fichier demandé compte. La comparaison
    se fait une fois par version du document et par date et taille du fichier. Le fichier est
    décodé comme VSCode a décodé le document (son encodage), et `\r\n` contre `\n` n'y
    compte pas, car ils ne changent ni ligne ni colonne.
- Un fichier source non enregistré suspend toutes les réponses, pas seulement celles qui le
  lisent. Une absence ne se voit pas : un appel ou un usage retiré dans ce tampon manquerait à
  la réponse sur un autre fichier.
- Le démon garde alors la tâche de côté, fichier ou recherche, sans la perdre ni rien
  enregistrer. Elle ne compte pas dans la file : rien ne tourne à vide. Elle y revient quand le
  client envoie `editor/saved`, quand un autre client relaie (le pipeline le voit lui-même, même
  sans être réveillé), ou quand un fichier change sur le disque. Un rafraîchissement qui ne
  change rien ne libère rien : il n'y a rien de nouveau à demander. L'extension envoie
  `editor/saved` quand plus aucun fichier source ne diffère du disque après un changement :
  enregistrement, retour arrière, relecture, fermeture.
- Avant d'enregistrer, le démon vérifie que le disque a toujours le contenu indexé, pour le
  fichier et pour les fichiers indexés où pointent les réponses. Sinon, un changement qu'il
  n'a pas encore vu a eu lieu, et VSCode a pu répondre sur le nouveau contenu : la tâche
  attend ce changement. Le démon relit aussitôt les fichiers trouvés différents, sans attendre
  son observateur de fichiers, qui a pu manquer le changement (relecture n° 7). La tâche est
  libérée si la relecture a changé l'index, ou si le fichier correspond de nouveau à l'index
  (un changement défait). Un fichier illisible, lui, reste différent : la tâche attend le
  changement suivant, au lieu d'être relue et redemandée sans fin (relecture n° 8, I8-1).
  Une réponse qui pointe dans un fichier que le démon n'indexe pas (une dépendance sous la
  racine, dans `node_modules`, un fichier ignoré, ou supprimé depuis) ne fait pas d'arête, et
  n'est pas vérifiée : elle ne garde rien (relecture n° 6, I6-1).

**Ce que l'affinage doit survit au redémarrage** (relecture n° 9, I9-2). La file vit en
mémoire. Un démon arrêté (inactif, relancé, tué) oubliait les recherches pas encore faites et
les dépendants pas encore revérifiés : leur vérification d'avant passait pour sûre, même quand
on rouvrait le fichier, vérifié « entier ».
- La base garde donc, à chaque changement, ce que l'affinage doit : les fichiers vérifiés
  qu'une signature changée a rendus périmés, et les recherches de dépendants en attente ou
  gardées de côté, avec leur symbole, sa position, son contenu et les fichiers qui citent son
  nom (migration 0004).
- Au démarrage, ces fichiers, s'ils sont encore vérifiés, retournent en file comme dépendants,
  et les recherches attendent de nouveau. Le reste de la file, l'affiché et l'ouvert, revient
  avec les clients.
- Un lot d'indexation et la dette qu'il laisse s'écrivent dans la même transaction, au
  démarrage avec les remises à zéro : ni un arrêt brutal ni un échec d'écriture ne laissent le
  lot sans sa dette (relecture n° 10, m10-1 ; n° 11, m11-1). Après un pas d'affinage, la dette
  s'écrit après les observations : un arrêt entre les deux laisse plus de dette que due,
  jamais moins.
- Seul ce qui change est écrit (m10-2). Mesuré sur une dette de 110, 1 100, 11 000 et 55 000
  lignes, un pas coûte 0,04, 0,11, 0,52 et 2,4 ms, contre 0,08, 1,5, 16,5 et 106 ms pour une
  réécriture entière (2026-10-17, build release, `cargo run --release -p timonier-store
  --example debt_bench`).
- Après un pas d'affinage, une écriture de la dette qui échoue est notée au journal, sans
  faire échouer le pas : la dette reste en mémoire, et part au pas suivant (m10-3). Avec un
  lot, l'échec fait échouer le lot entier : une réindexation complète le refait aussitôt, avec
  la dette qu'il laissait (relecture n° 12, m12-1). Si elle échoue aussi, le rafraîchissement
  suivant la refait d'abord (m12-2), et de même tout changement du dossier git, avant de
  figer un snapshot s'il y a un nouveau commit (relecture n° 13, m13-2 ; n° 16, m16-1). Ce
  rattrapage est rapporté avec le rafraîchissement ou la consolidation qui le fait, comme
  tout ce que la consolidation change dans la tête en route, snapshot figé ou non : les vues
  relisent la tête (relecture n° 15, m15-3 ; n° 16, m16-5). Une indexation ou une
  consolidation qui échoue les fait relire aussi, la tête ayant pu changer avant l'échec
  (relecture n° 17, m17-5).
- La base est un cache reconstructible : la perdre, c'est perdre aussi les vérifications.
- Une question perdue parce que le client qui relaie part, ou qu'un autre prend sa place, rend
  la tâche à la file : le client suivant la reçoit (relecture n° 6, m6-3).

**Un fichier vérifié entier n'est pas redemandé** (relecture n° 5, m-β). Un client qui
redemande un fichier (ouvert, affiché) dont le contenu actuel est vérifié pour les quatre
sortes ne coûte aucune question. Restent redemandés : un fichier dont les usages sont restés
ceux de tree-sitter (réponse douteuse, définitions en échec), un fichier compacté, qui
retrouve son détail (ADR 0018), un dépendant d'une signature changée, un fichier changé.

**Le reste ne change pas** : file paresseuse, priorités, vérification au hash, relais par le
dernier client.

## Alternatives écartées

- **Une définition par site** : c'est le coût que l'ADR 0014 refusait, pour une réponse
  identique à celle du premier site dans presque tous les cas.
- **Regrouper les chemins de même premier segment** (`Kind`, `Kind::A`, `Kind::B`) : un tiers
  de questions en moins en Rust (521 contre 800, essayé le 2026-10-17), mais faux dès qu'une
  réexportation par `*` ou un `impl` dans un autre fichier les sépare. Les variantes ne sont
  pas extraites comme symboles : rien ne permet de le vérifier.
- **Croire une réponse locale pour un nom que tree-sitter place ailleurs** : c'est la réponse
  du serveur syntaxique, et l'arête juste de tree-sitter tomberait.
- **`executeReferenceProvider` sur chaque symbole du fichier affiché**, pour ses arêtes
  entrantes : il interroge tout le projet pour chaque symbole, et inverse la logique « on
  affine ce qu'on regarde ».
- **Vérifier aussi les chemins locaux** : ils ne font aucune arête entre fichiers.
- **Une liste de noms hors dépôt écrite dans le démon** : c'est une connaissance du langage.
  Elle va dans le pack.
- **Reconnaître une recherche à sa position** : deux sauvegardes peuvent mettre deux symboles
  au même endroit, et la recherche fusionnée perdait les dépendants du premier.
- **Répondre sur le tampon non enregistré** : le démon n'indexe que le disque. Un tampon n'a
  pas de hash dans le modèle, et l'agent n'écrit que sur le disque.
- **Ne suspendre que les questions qui lisent le document non enregistré** : voir plus haut,
  une absence ne se voit pas.
- **Retrouver au redémarrage ce qui était dû, sans le garder** : la base ne date ni les
  vérifications ni les changements de fichiers, et une recherche pour un symbole apparu ne se
  déduit de rien.
- **Redemander une tâche refusée à intervalle régulier** : le démon interrogerait sans fin un
  client dont un tampon reste modifié des heures. `editor/saved` dit quand reprendre.
- **Tout document du dépôt suspend les réponses** : une note en cours d'écriture bloquait toute
  vérification (relecture n° 6, m6-2).
- **Comparer en UTF-8 seulement** : un fichier d'un autre encodage ne correspondait jamais au
  disque, et l'extension posait toutes ses questions avant de refuser, à chaque libération.

## Conséquences

- Un chemin cité sous deux sens dans un même fichier (un nom masqué par une déclaration
  locale) est vérifié une fois, au premier site. C'est déjà la granularité de tree-sitter.
- Un fichier dont un usage reste douteux garde ses usages de tree-sitter jusqu'à son prochain
  affinage : quand il change, ou qu'on l'ouvre ou l'affiche de nouveau. Le doute ne porte que
  sur un nom lié par un import ou un chemin, que le serveur place à son import : un serveur
  prêt le résout.
- Tant qu'un fichier source est modifié sans être enregistré, rien n'est vérifié : la carte
  garde ce qu'elle montre, et ce qui attend reprend à l'enregistrement ou au retour arrière.
  Ce comportement se voit : l'humain l'a confirmé (Q22, réponse a, 2026-10-18). Pendant qu'un
  agent écrit, chaque écriture libère ce qui attend, et l'extension le
  refuse aussitôt, sans interroger ses serveurs : un aller-retour par tâche.
- Limites :
  - un serveur voit un fichier qu'aucun éditeur n'a ouvert avec le retard de son propre
    observateur de fichiers. Dans cet intervalle, une réponse peut s'appuyer sur l'ancien
    contenu d'un fichier que la question ne lit pas. Le démon ne peut pas le voir ;
  - un fichier que VSCode décode autrement qu'en UTF-8 est comparé dans son encodage, mais le
    démon le lit en UTF-8 : sur une ligne qui a des caractères hors ASCII, ses colonnes peuvent
    différer de celles de VSCode ;
  - un fichier devenu illisible garde ce qui pointe vers lui jusqu'au changement suivant qui
    modifie l'index (une note ne compte pas), jusqu'à `editor/saved`, ou jusqu'à ce qu'on rouvre
    ou affiche le fichier gardé. Redevenu lisible, il ne le dit pas : un changement de droits ne
    fait pas d'événement pour l'observateur.
- Mesuré sur Timonier le 2026-10-17 (exemple `lsp_cost`, PLAN.md 4.5 ; le dépôt change, les
  chiffres avec lui) :
  - Rust : 5 532 sites, 1 764 chemins distincts, 787 questions (86 % de moins qu'une par
    site) ;
  - TypeScript : 883 sites, 417 chemins distincts, 180 questions (80 % de moins) ;
  - un lot par fichier, donc 135 allers-retours au plus pour tout le dépôt, et seulement pour
    ce qui est affiché, ouvert ou dépendant ;
  - une signature changée : une recherche si un autre fichier cite le nom (1 098 symboles sur
    2 012 en Rust, 510 sur 837 en TypeScript), au lieu de revérifier les fichiers qui le
    citent (8,7 et 6,2 en moyenne). Aucune pour les autres.
- Le statut du démon compte les questions auxquelles le client a répondu, par méthode, et
  celles qu'il n'a pas pu répondre, faute de serveur (`lspAnswers`), les recherches de
  dépendants, nommés par le serveur ou par nom (`lookups`), et les tâches gardées de côté
  (`refinementsHeld`).
- Le protocole gagne deux méthodes du démon vers le client qui relaie, `lsp/definitions` et
  `lsp/references`, la notification `editor/saved`, le code d'erreur `ContentModified`, et les
  extensions des fichiers source dans la réponse à `initialize` (ADR 0019).
- Un futur serveur lancé par le démon (Q5, plus tard) implémente les mêmes trois questions.
