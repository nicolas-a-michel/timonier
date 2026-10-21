# 0040 — Le dessin libre pour l'agent : une image et des emplacements

- Statut : Accepté (2026-10-21, validation de la Phase 7 par l'humain)
- Date : 2026-10-21
- Phase : 7
- Liens : spec « Tableau blanc » (mode mixte : dessin libre « interprété par un modèle vision et
  par le texte ») ; Q6, Q67 ; ADR 0039 (`intent.json`) ; tâches PLAN.md 7.4, 7.5

## Contexte

La spec veut le dessin libre interprété par un modèle vision et par le texte. Q6 : pas de clé
d'API dans le démon ; des heuristiques par défaut, enrichies par l'agent connecté, qui lit une
image du croquis par MCP. Le tableau blanc est à sens unique : rien ne revient dessus. Ta réponse
Q67 : l'agent reçoit une image du croquis, et en texte l'emplacement de chaque trait.

## Décision

**L'image** (`timonier_daemon::sketch::render`) : le démon dessine `intent.json` en PNG, noir
sur blanc quel que soit le thème de l'IDE (une image à lire, pas à montrer) : les couches en
tireté gris, les composants en cadre plein, les flèches avec leur tête (interdite : rouge et
tiretée ; requise : épaisse), les notes et budgets en cartes jaune pâle, les traits libres par
dessus, et l'ID de chaque forme écrit dans son coin (police de points 5×7 intégrée : capitales,
chiffres, tiret, souligné ; un ID nommé, Q69, s'écrit en capitales, un caractère hors police est
un carré creux, jamais sauté), pour que ce que l'agent voit et ce que dit `intent.json` se
répondent. Le cadre suit le dessin, avec une
marge ; le plus grand côté fait 1 600 pixels au plus. Le même tableau donne la même image ; un
tableau sans rien n'en donne pas. Bibliothèque : `tiny-skia` 0.12.0 (BSD-3-Clause, pur Rust,
vérifiée le 2026-10-21 ; elle apporte `png` 0.18.1 et ses dépendances, MIT, Apache-2.0, Zlib ou
BSD), dans le démon seulement : `timonier-core` reste sans dépendance de dessin.

**Les emplacements** (`Intent::stroke_places`, dans `timonier-core`) : pour chaque trait, son
rectangle, la plus petite couche et le plus petit composant qui contiennent son milieu, sinon le
composant le plus proche à 60 pixels au plus. Pour un agent qui ne voit pas, ou pour recouper.

**Où ça va** : l'outil `get_intended_architecture` (7.5) rend l'intention structurée, les
emplacements et l'image. Ni l'image ni les emplacements ne sont écrits : ils se calculent depuis
`intent.json` à chaque lecture. L'exemple `cargo run -p timonier-daemon --example sketch` dessine
un `intent.json` dans un fichier, pour voir ce que voit l'agent.

## Alternatives écartées

- **La webview fait l'image** (canevas) : l'image n'existerait que tableau ouvert, et pourrait
  différer du fichier après un `git checkout`.
- **Reconnaître les formes des traits** (rectangle, flèche) : écarté par Q67 ; des erreurs
  possibles, et le modèle vision de l'agent le fait mieux.
- **SVG plutôt que PNG** : les agents prennent des images PNG, JPEG, GIF ou WebP ; un SVG serait
  lu comme du texte.
- **`resvg`** : rend du SVG complet ; on n'a que des traits, des rectangles et des flèches.

## Conséquences

- Un agent sans vision a quand même la structure et les emplacements.
- Deux IDs nommés qui ne diffèrent que par la casse (`C-db`, `C-DB`) s'écrivent pareil sur
  l'image ; les emplacements en texte portent l'ID exact.
- Les noms des formes ne sont pas écrits sur l'image (pas de police de texte) : l'agent les a
  dans l'intention, reliés par les IDs écrits.
