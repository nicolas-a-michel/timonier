# 0024 — Diff de revue

- Statut : Accepté (2026-10-20)
- Date : 2026-10-18
- Phase : 5
- Liens : spec « Expérience et design » (diff depuis ma dernière revue), « Stockage », « Identité
  stable » ; Q26, Q27, Q28, Q30, Q31, Q33 à Q42 ; relectures n° 1 à n° 8 de la Phase 5 ; ADR 0009
  (arêtes pendantes),
  0010 (historique), 0015 (gel d'un commit), 0016 (identité des composants), 0019 (protocole),
  0020 (système de design), 0021 (carte) ; tâches PLAN.md 5.1 à 5.3

## Contexte

La spec veut un diff « depuis ma dernière revue » : composants, dépendances et cycles ajoutés,
composants dont le fan-in ou le fan-out a bondi, badge « modifié » sur les boîtes touchées par
un refactoring interne ; un bouton « marquer comme revu » crée un snapshot `review_mark`. Elle
ne dit ni ce qu'est une dépendance entre composants, ni le seuil d'un bond, ni comment le
diff arrive à la carte. Tes réponses fixent trois points : les suppressions ne sont que dans le
résumé (Q26), pas de diff avant la première revue (Q27), et ce que la seule vérification par
le serveur de langage change ne compte pas (Q28). L'empreinte du contenu d'un fichier n'est
gardée que pour la tête (table `scope`) : Q28 demande de la retrouver à la revue.

## Décision

**Marquer comme revu** (`review/mark`, requête sans paramètre, qui rend le snapshot figé : son
numéro et son horodatage).
- Le démon fait passer le gel par le fil du pipeline, entre deux étapes. Un gel `review_mark`
  ne tombe donc jamais au milieu d'une consolidation, dont le gel prend un moment les versions
  commitées (ADR 0015). Comme avant un commit, une base en retard est rattrapée d'abord, et ce
  que le disque tient sans que la watch l'ait encore dit est relu : une sauvegarde faite juste
  avant le clic est dans la revue.
- Le même gel garde l'empreinte du contenu de chaque fichier à cet instant (table
  `snapshot_content`, migration 0005), pour la dernière revue seulement. Ce ne sont pas des
  faits du graphe : comme le cache d'extraction, la table n'a pas d'intervalle de validité.
- L'extension l'offre en commande (« Marquer comme revu ») et par un bouton du résumé de la
  carte, avec une confirmation discrète dans la barre d'état.

**Ce que le diff compare** : le snapshot de la dernière `review_mark` et la tête. Sans revue,
pas de diff (Q27).

**Fichier changé** : présent d'un seul côté, ou d'empreinte différente à la revue et à la tête.

**Déplacements et renommages** (spec « Identité stable » : pas de « tout supprimé, tout
recréé »). Ce que la revue avait sous un autre identifiant est retrouvé, et le diff compare
sous les identifiants de la tête.
- Un fichier disparu d'un chemin et retrouvé à un seul autre avec le même contenu est
  déplacé : il est « modifié », pas « nouveau ». Ses symboles le suivent, appariés par leur
  place dans le fichier (les noms depuis le fichier, et l'étendue) : leur identifiant porte le
  chemin du module, que le déplacement a changé. Ouvert jusqu'aux symboles, un fichier déplacé
  garde ainsi ses liens. De même pour un fichier resté à son chemin, inchangé, dont les
  symboles ont changé d'identifiant : il a changé de crate, dont le nom est dans leurs noms
  qualifiés (ADR 0013).
- Une boîte disparue et une boîte nouvelle de même type sont une seule boîte, renommée ou
  déplacée, quand chacune tient la majorité des fichiers de l'autre ; chaque boîte une fois, le
  plus grand recouvrement d'abord. C'est le cas d'un dossier renommé une fois les composants
  recalculés (au commit, ADR 0016), ou d'un paquet renommé par son manifeste. Pour chaque boîte
  disparue, seules les boîtes nouvelles qui tiennent ses fichiers sont comptées : le coût suit
  le nombre de fichiers, pas le carré du nombre de boîtes.

**Boîtes** : les composants et les paquets, paquets externes compris. Seul un changement du
code compte (Q30) : un regroupement automatique n'est rien. Un composant en est toujours un, et
n'est recalculé qu'au commit (ADR 0016) : une boîte n'est donc nouvelle ou supprimée que si du
code arrive ou part avec elle (Q33). Un paquet, lui, vient de son manifeste, qui est du code
(Q35).
- Ajoutée : nouvelle (sans équivalent à la revue), et qui apporte un fichier nouveau (ni
  présent à la revue, ni déplacé) ; ou qui ne contient aucun fichier (un paquet externe, une
  catégorie d'entre eux), qui vient alors des manifestes ; ou un paquet nouveau, que son
  manifeste apporte, même autour de fichiers inchangés.
- Supprimée : disparue sans équivalent à la tête, et dont un fichier a été supprimé (ni
  présent à la tête, ni déplacé) ; ou qui ne contenait aucun fichier ; ou un paquet disparu,
  son manifeste parti (ses fichiers inchangés passent sous le dépôt).
- Modifiée : toute autre boîte de la tête, nouvelle ou non, qui contient un fichier changé ou
  déplacé, à la revue ou à la tête. Ainsi un dossier renommé avant le commit, dont les fichiers
  vont sous leur paquet en attendant les composants, ne laisse aucune boîte supprimée : le
  paquet et les fichiers sont modifiés. Un fichier ou un symbole déplacé change deux fichiers :
  chacune de leurs boîtes est modifiée.
- Un paquet est aussi modifié quand son manifeste le renomme, ou quand il perd tous ses
  fichiers ou en gagne (Q34) : une crate dont le manifeste est supprimé devient une dépendance
  externe, quand une autre en dépend, ses fichiers inchangés.
- Un paquet nouveau, supprimé ou changé par son manifeste compte, comme un fichier, pour les
  boîtes qui le contiennent, à la revue et à la tête : elles sont modifiées (ou ajoutées,
  supprimées, selon les mêmes règles). Ainsi la catégorie de dépendances externes que rejoint
  une crate sans manifeste. Ses fichiers comptent comme déplacés (Q37) : la boîte qu'ils
  quittent ou rejoignent est modifiée, la crate racine par exemple, et leurs dépendances les
  suivent.
- Un nom, un parent ou un rôle qui change sans fichier changé n'est pas un changement du code.
- Les fichiers affichés portent aussi « nouveau » ou « modifié ». Les symboles, non.

**Dépendances** : entre deux boîtes sœurs, tous types d'arêtes confondus. Une arête compte pour
la paire des enfants de son plus proche ancêtre commun qui contiennent ses deux bouts, si ces
deux enfants sont des boîtes. Les arêtes pendantes ne comptent pas (ADR 0009).
- Les arêtes elles-mêmes sont comparées (Q36, qui précise Q28), sous les identifiants de la
  tête. Une dépendance est ajoutée, ou retirée, quand une des arêtes qui la portent :
  - apparaît, ou disparaît, et touche un fichier modifié, ajouté ou supprimé (un déplacement
    n'est pas une modification) ; une arête entre paquets, d'un manifeste, toujours ;
  - ou existe des deux côtés, mais porte à la tête une autre dépendance qu'à la revue, parce
    qu'un de ses bouts a changé de boîte : un fichier déplacé, un fichier d'un paquet que son
    manifeste a fait, défait ou fait passer d'interne à externe, ou ce paquet lui-même (Q34,
    Q35, Q37). Un paquet seulement renommé reste la même boîte, ses fichiers aussi. Le bout
    compte si, au niveau de la dépendance, ni la boîte qu'il quitte ni celle qu'il rejoint
    n'est un regroupement (Q38) ; et, son paquet restant le même, si ces boîtes ne sont pas
    au-dessus de ce paquet : là, seul le regroupement du paquet l'a déplacé. Un fichier
    renommé dans son dossier ne bouge pas : s'il change de composant, c'est le fait du
    regroupement (Q40). Un fichier qui change de dossier, ou un dossier qui change de crate,
    emporte ainsi ses dépendances. Un fichier rangé dans un dossier nouveau que seul ce
    déplacement a fait n'emporte rien à l'intérieur de sa crate ; s'il change de crate, ses
    dépendances de crate le suivent (Q41).
  - Quand les deux bouts bougent, chacun compte pour son propre déplacement (Q41) : un
    appelant rangé dans un regroupement et un appelé passé d'une boîte gardée à une autre font
    lister la dépendance que l'appelé a quittée et celle qu'il rejoint.
- Le reste n'est pas un changement du code. Un regroupement (une boîte nouvelle sans être
  ajoutée, ou disparue sans être supprimée, `ReviewDiff::regrouped`), ou des paquets répartis
  autrement sous les composants, ne changent aucune dépendance, même si un fichier a changé
  ailleurs. Une arête que le serveur de langage seul a trouvée ou contredite, non plus.
- Un dossier renommé avant le commit ne laisse donc pas ses dépendances « retirées », même avec
  un commentaire ajouté à un fichier voisin. Un appel écrit depuis dans un composant regroupé
  au commit compte.

**Cycles ajoutés** : composantes fortement connexes (Tarjan) d'au moins deux boîtes, dans le
graphe des dépendances entre les enfants d'une même boîte (ou du dépôt), à la tête,
qu'aucune composante de la revue ne contient, sous les identifiants de la tête. Elles sont
cherchées dans les boîtes de la tête telles qu'elles sont, regroupements compris, sans ce que
le serveur de langage seul a apporté et avec ce qu'il a seul retiré (Q28). Elles ne comptent
que si une dépendance ajoutée les ferme : un cycle qu'un regroupement seul a fait n'est pas un
changement du code (Q30, Q33).

**Fan-in et fan-out** : le nombre de boîtes sœurs distinctes qui dépendent d'une boîte, et dont
elle dépend, à la revue, puis avec les dépendances ajoutées et sans les retirées. Il a bondi
s'il a augmenté d'au moins 3 et d'au moins 50 % (`après ≥ avant + 3` et
`2 × après ≥ 3 × avant`). Seulement pour une boîte présente des deux côtés.

**Sur la carte** (protocole, version 3)
- `ViewNode.review` : ce qui change pour la boîte (`added`, `modified`), ses bonds de fan-in et
  de fan-out (avant, après), et si elle est dans un cycle ajouté ; `null` sans changement, et
  toujours avant la première revue.
- `ViewEdge.review` : une arête affichée est « nouvelle » si, à la revue, aucune arête ne
  reliait les deux mêmes nœuds affichés (sous les identifiants de la tête), et qu'une des
  arêtes entre eux, tous types confondus, est apparue dans un fichier changé, ou a suivi un
  fichier déplacé entre deux boîtes qui ne sont pas des regroupements (Q28, Q36) ; elle dit
  aussi si elle relie deux boîtes d'un cycle ajouté.
- `ViewSnapshot.review`, et `ViewDelta.review` quand il change : le résumé, indépendant du zoom.
  Il donne la revue de départ (ou rien), les boîtes ajoutées, modifiées et supprimées, les
  dépendances ajoutées et retirées, les cycles ajoutés et les bonds. Chaque boîte y est nommée
  avec celle qui la contient : à la tête, sauf pour ce qui est parti (boîtes supprimées,
  dépendances retirées), nommé comme à la revue (Q34). Les suppressions ne sont que là (Q26).
- Une ligne de la carte qui regroupe plusieurs arêtes affichées (entre deux rôles fermés, par
  exemple) n'est « nouvelle » que si toutes le sont : une dépendance nouvelle entre deux rôles
  déjà reliés se voit au compte du rôle et dans le résumé.
- Badges : le ton « attention » du système de design (ADR 0020), une forme et un mot par
  sens : « Nouveau » (une étoile pleine), « Modifié » (un point plein), « Cycle », « Fan-in »
  ou « Fan-out » avec les deux nombres. Pas de « + » ni de crayon : au test des cinq secondes
  n° 1, ils se lisaient comme des actions, « ajouter » et « éditer ». Un rôle fermé dit
  « 2 modifiés ». La légende de la carte explique ces marques dès qu'il y a une revue. Une arête nouvelle prend la couleur d'attention et une marque à mi-chemin, et son
  nom accessible le dit.
- Le filtre « Mettre en avant les changements » estompe ce qui n'a pas changé, sans rien
  retirer : la mise en page ne bouge pas (ADR 0021 ; Q31). Avant la première revue, il n'a pas
  d'effet. Les lignes estompées ne sont plus tenues au contraste de 3:1 : elles reculent.
- Le résumé est un bandeau en haut, au-dessus de la carte et sur toute sa largeur : c'est ce
  que l'écran dit d'abord (test des cinq secondes n° 1, où le résumé en bas à droite n'avait
  pas été vu). Une phrase « Depuis votre revue du … : » et les comptes ; les actions à droite,
  « Marquer comme revu » en bouton principal ; les listes dessous à la demande. Sur une carte étroite, la légende ne garde que son titre,
  et le bandeau replié ses comptes et « Détails ».

**Coût** : le snapshot de la revue, figé, est lu une fois par revue et gardé en mémoire par le
fil des vues. À chaque changement s'ajoutent la lecture des empreintes de la tête et le diff :
deux ensembles d'arêtes et une agrégation de plus, linéaires en arêtes, et l'appariement des
renommages, linéaire en fichiers (chaque fichier compté pour les boîtes qui le tiennent).
Ce qui peut changer de boîte (fichiers déplacés, paquets changés par leur manifeste et leurs
fichiers) est rassemblé une fois par diff ; seules les arêtes qui en partent ou y arrivent
sont placées aux deux bouts. Mesuré le 2026-10-19 après la relecture n° 6 (PLAN.md 5.2,
`view_bench synthetic:CxDxF`, build release ; d'une passe à l'autre, les chiffres varient d'un
tiers) :
- 2 450 fichiers (6 958 nœuds) : 14 à 15 ms tous fichiers changés, 25 à 28 ms tout renommé
  (84 ms avec l'appariement quadratique d'avant la relecture n° 2) ; la vue entièrement
  ouverte, 8 à 10 ms au lieu de 3,4 ms ;
- 6 125 fichiers (17 390 nœuds, environ 1 140 boîtes) : 38 à 50 ms et 78 à 122 ms ; la vue
  entièrement ouverte, 30 à 44 ms au lieu de 11 à 14 ms.
- Le graphe de la revue gardé en mémoire, mesuré dans un processus neuf (croissance de la
  mémoire résidente pendant le chargement, son transitoire compris, donc au plus) : 6,3 Mio
  pour Timonier (3 694 nœuds), 7,6 Mio pour 6 958 nœuds, 18,7 Mio pour 17 390.

## Alternatives écartées

- **La détection de déplacements de l'ADR 0010** (similarité des symboles) : elle lit
  l'historique de la base à chaque changement. Le contenu identique couvre un `git mv`, et la
  majorité des fichiers, un dossier ou un paquet renommé. Un fichier déplacé et modifié à la
  fois reste « supprimé » à un chemin et « nouveau » à l'autre.
- **Une dépendance par type d'arête** : un appel ajouté là où un import existait déjà ferait une
  « dépendance ajoutée » de plus entre les mêmes boîtes.
- **Versionner l'empreinte des fichiers dans leur nœud** : chaque enregistrement réécrirait la
  portée de structure, qui tient tous les fichiers.
- **Des fantômes sur la carte** pour ce qui a disparu : écartés par ta réponse à Q26.
- **Un diff calculé par l'extension ou la webview** : la vérité du graphe est dans le démon
  (ADR 0019).

## Conséquences

- La carte et le résumé suivent la tête en continu : un changement après la revue apparaît
  aussitôt, et « marquer comme revu » remet le diff à zéro.
- Seule la dernière revue garde ses empreintes : comparer à une revue plus ancienne demandera
  de les garder aussi.
- Une dépendance que la vérification seule a changée n'apparaît pas dans le diff, mais la carte
  la montre telle qu'elle est (Q28). Un fichier dont seuls les commentaires ont changé compte
  comme changé : son empreinte a changé.
- Le seuil des bonds est à revoir sur un vrai projet, si les bonds signalés sont trop ou trop
  peu nombreux.
- Un dossier vidé dans un autre n'est pas listé comme supprimé : son code n'est pas parti, et
  la boîte qui le reçoit est modifiée (Q33). Ses dépendances ne sont pas « retirées » non plus,
  et celles de la boîte qui le reçoit ne changent que par ce qu'une arête nouvelle apporte.
- Un cycle qu'un regroupement seul a fait n'est pas signalé ; un cycle qu'une dépendance
  ajoutée ferme l'est, au niveau que la carte montre, même entre composants regroupés au même
  commit.
- Une modification qui ne change aucune arête (un commentaire, un corps de fonction) marque son
  fichier et ses boîtes « modifiés », sans dépendance changée.
- Un fichier renommé dans un dossier découpé en composants, et modifié en même temps, n'est pas
  retrouvé (un déplacement avec modification ne l'est pas) : il est « nouveau », rejoint le
  composant de la majorité de ses voisins, et ses arêtes comptent. Jusqu'au commit, le
  résumé peut alors lister des dépendances et un cycle entre deux composants du dossier
  (relecture n° 8 de la Phase 5, accepté par Q42).
- Un fichier regroupé au commit dans un composant nouveau et modifié, qui appelle désormais un
  autre fichier de la même boîte, fait lister une dépendance retirée (de son ancienne boîte)
  et une ajoutée (de la nouvelle) pour un code qui dépend toujours de cette boîte. Le cas est
  rare, noté par ta réponse à Q39.
