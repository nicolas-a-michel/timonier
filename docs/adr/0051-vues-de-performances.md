# 0051 — Les vues de performances : le calque de la carte, et le panneau

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Performances » (vues : timeline par test, carte annotée, évolution par commit),
  « Expérience et design » ; Q73 ; ADR 0020 (système de design), 0021 (carte : l'épaisseur est
  au trafic), 0042 (où se montre une boîte), 0043, 0047, 0048 ; tâche PLAN.md 8.7

## Contexte

La spec veut une timeline par test (spans emboîtés, ressources du processus dessous), une carte
annotée (temps cumulé, nombre d'appels, arêtes épaissies selon le trafic) et l'évolution par
commit ; tu as demandé (2026-10-18) un arbre des appels qu'on déplie, comme les outils des
navigateurs, avec le temps et les ressources à chaque niveau. Ta réponse Q73 : le calque sur la
carte, le reste dans un panneau à part.

## Décision

**Données** : trois méthodes du démon. `perf/runs` (les 50 derniers runs : durée, CPU,
mémoire, tests, échecs, spans) ; `perf/run { run }` (ses tests, ses spans, au plus 20 000, les
racines d'abord, leurs temps comptés depuis le début du run, ce qui tient dans un nombre
JavaScript ; ses échantillons ; le nom de chaque nœud) ; `perf/map { run? }` (le dernier run fini
avec traces par défaut : par nœud, ses appels, son **temps propre**, celui de ses spans moins
celui des spans qu'ils ont appelés, et le temps de ses spans qu'aucun span d'un nœud n'a
appelés ; par lien d'un nœud à un autre, ses appels et le temps des spans appelés ; la lignée de
chaque nœud). La carte somme ces chiffres sur la boîte qui montre chaque nœud, trouvée comme pour
l'intention (ADR 0042). Le **temps cumulé** d'une boîte (spec, Q76) est le temps pour lequel on
y est entré : depuis aucun nœud, ou depuis un nœud qu'une autre boîte montre ; un appel interne
à la boîte n'y ajoute rien. Limite : une boîte quittée puis reprise par un appel venu de dehors
compte ce second appel en plus.

**Le calque** : un bouton « Performances » de la barre d'outils et sa commande, gardés dans
l'état de la page. Allumé : sur chaque boîte, son temps cumulé (Q76) et, dans son infobulle,
pour les lecteurs d'écran et le détail, son temps propre et ses appels ; chaque lien épaissi selon les appels
mesurés, sur une échelle logarithmique de une à quatre fois l'épaisseur ordinaire (l'ADR 0021
réservait l'épaisseur à ce trafic) ; un bandeau qui nomme le run et les trois nœuds où le temps
est allé, et ouvre le panneau ; la légende. L'extension redemande `perf/runs` toutes les 3 s
tant que le calque est allumé et reconstruit le calque quand un nouveau run avec traces finit.

**Le panneau** : une quatrième page construite à part (`dist/performance/`), sous la CSP à
nonce. En tête, le choix du run (le plus récent par défaut, hors tests de fumée de `doctor`,
marqués en mot dans la liste ; relecture n° 3) et ses chiffres. Trois onglets, au
clavier par les flèches : la **timeline** (les tests, leur état en forme et en mots, leur durée en
barre ; les spans du test choisi en rangées par profondeur, sur le temps du test ; dessous, la
mémoire et le CPU du processus pendant le test) ; l'**arbre des appels** (un `treegrid` : les
tests en haut, chaque niveau fusionné par nom et nœud comme le « Call tree » des navigateurs,
total, propre, appels, et la mémoire max et le CPU de tout le processus pendant que le niveau
tournait, d'après ses échantillons : le processus, pas le niveau, et le panneau le dit) ;
l'**évolution** (spec : la médiane de plusieurs runs par commit ; durée et temps CPU, du plus
ancien commit au plus récent ; seuls les runs de même commande, même machine et mêmes traces que
le run affiché comptent ; un arbre modifié, Q77, fait un point à part, marqué « * » ; relecture
n° 1, I1). Un test en échec et un span en erreur le disent par une icône et un mot, pas par la
seule couleur (relecture n° 1, I2). Ouvert par la carte, le pont et la palette ; il redemande les runs toutes les 3 s tant
qu'il se voit.

**Les courbes** : uPlot 1.6.32 (MIT, vérifiée en Phase 0, prévue par la spec). Le canevas lit
les couleurs des jetons par un élément sonde (une variable CSS se lit telle qu'écrite) et les
relit quand le thème change. Chaque courbe a son tableau, caché à la vue, pour les lecteurs
d'écran ; la timeline des spans est une image dont l'arbre des appels dit le contenu.

## Alternatives écartées

- **Tout dans la carte**, **tout dans un panneau** : écartés par Q73.
- **Sommer le temps total des spans d'une boîte** : un appel imbriqué dans la même boîte
  compterait deux fois.
- **Pousser chaque fin de run du démon aux pages** : un abonnement de plus dans le protocole ;
  une requête légère toutes les 3 s, et seulement quand une vue se voit, suffit.
- **Un graphique en SVG dessiné à la main** : uPlot est déjà le choix de la spec, et il tient les
  séries longues.
- **La mémoire de chaque span** : rien ne la mesure ; le processus seulement, échantillonné.

## Conséquences

- Les vues ne montrent que ce que le run a reçu : un code sans instrumentation n'a que les
  racines de ses tests, et le panneau dit comment instrumenter.
- Au-delà de 20 000 spans, le panneau dit combien il en laisse.
- Le test des cinq secondes du panneau et du calque est à faire par toi.
