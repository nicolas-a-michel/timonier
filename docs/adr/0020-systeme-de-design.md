# 0020 — Système de design de la webview

- Statut : Accepté (2026-10-18, validation de la phase par l'humain)
- Date : 2026-10-16
- Phase : 4
- Liens : spec « Expérience et design », « Précision affichée » ; ADR 0005, 0006 ; skill
  `vscode-webview-ui` ; tâche PLAN.md 4.6

## Contexte

La spec demande une interface calme, native et accessible :
- le thème de l'IDE, en clair, en sombre et en contraste élevé ;
- des couleurs vives réservées au sens, jamais seules ;
- un contraste WCAG AA ;
- une arête connue seulement de tree-sitter qui se distingue d'une arête confirmée ;
- la réduction des mouvements respectée.

Le plan veut ce système avant la première vue (4.6), avec un contraste vérifié automatiquement
par axe-core. Il fallait décider :
- d'où viennent les couleurs, et comment vérifier les contrastes dans des thèmes qu'on ne
  contrôle pas ;
- quels composants, et où ;
- comment coder la précision des arêtes ;
- comment les tests mesurent ce qu'axe-core ne mesure pas.

Vérifié le 2026-10-16 : axe-core 4.14.0 (MPL-2.0) et @playwright/test 1.64.1 (Apache-2.0),
les dernières versions, pour le développement seulement.

## Décision

**Jetons** (`webview/src/tokens.css`, préfixe `--tim-`, seul fichier qui nomme des couleurs)
- Chaque jeton renvoie à une variable du thème de VSCode : surfaces, texte, sélection,
  info-bulles, focus.
- Les contours des boîtes et les arêtes sont des mélanges (`color-mix`) du texte vers le fond.
  Ils restent calmes, mais au-dessus de 3:1 dans chaque thème. En contraste élevé, le contour
  vient de `contrastBorder`.
- **Sélection** : le fond et le texte de sélection des listes de VSCode. Les thèmes à
  contraste élevé n'ont pas de fond de sélection : la sélection y est montrée par le contour
  `contrastActiveBorder`, comme dans VSCode.
- **Sens** : violation, attention, conformité. Les couleurs viennent des listes de VSCode,
  faites pour du texte et des icônes sur fond de liste (`list.errorForeground`,
  `list.warningForeground`), puis de l'éditeur ; la conformité est `charts.green`. Le vert
  « test réussi » de VSCode (`testing.iconPassed`) est écarté : il tombe à 2:1 sur fond clair.
- **Rôles** (ADR 0017). Tu as demandé « quelques couleurs pour une meilleure lisibilité »
  (2026-10-16). Le choix des teintes est le mien :
  - point d'entrée en bleu (`charts.blue`), interface en violet (`charts.purple`), modèle de
    données en magenta (`terminal.ansiMagenta`), persistance en cyan (`terminal.ansiCyan`) ;
  - ce sont les quatre teintes calmes des thèmes hors du rouge, de l'orange, du jaune et du
    vert. Ces quatre-là portent la violation, l'attention et la conformité. La persistance
    était d'abord en jaune (`charts.yellow`), mais c'est la couleur d'attention de plusieurs
    thèmes : `#cca700` des deux côtés dans Dark Modern (relecture n° 2, I-c) ;
  - les tâches de fond, les utilitaires, les bibliothèques externes et ce qui n'a pas encore de
    rôle restent neutres : il n'y a pas de cinquième teinte libre ;
  - chaque teinte est tirée de 15 à 25 % vers la couleur du texte, pour tenir son contraste en
    clair comme en sombre ;
  - un rôle colore son icône, la barre de titre de ses boîtes (22 %), le voile d'un cadre
    ouvert (5 %) et les fils de l'élément survolé. Le texte secondaire d'une barre de titre se
    rapproche du texte (`--tim-color-muted-on-header`) pour garder 4,5:1 ;
  - en contraste élevé, aucune teinte : la couleur du rôle devient la ligne sous la barre de
    titre ;
  - jamais la couleur seule : chaque rôle a son icône, d'une forme à lui, partout. Son nom est
    écrit dans les boîtes, les cartes de rôle et les cadres ouverts ; au zoom éloigné, il ne
    reste que l'icône et le nom de l'élément ;
  - daltonisme, simulé (matrices de Machado, 2009) sur les couleurs rendues des six thèmes,
    en écart ΔE (CIE76) entre les deux rôles les plus proches :
    - les icônes restent distinctes : ΔE de 4,1 à 16,6 sous protanopie, deutéranopie ou
      tritanopie, contre environ 30 en vision normale ;
    - les teintes des barres de titre, à 22 %, se confondent en deutéranopie dans les thèmes
      sombres : interface et persistance à ΔE 0,5 à 0,7 ;
    - la couleur aide donc la vision normale, et l'icône avec le mot portent le rôle pour tous.
- Typographie (police et taille de l'IDE, police du code), espacements sur une base de 4 px,
  rayons, épaisseur du focus.
- Durées courtes avec une courbe douce ; elles passent à 0 sous `prefers-reduced-motion`.
- Une densité compacte (`data-density="compact"`) qui resserre les espacements.

**Composants** (`webview/src/design/`), qui n'utilisent que les jetons :
- **boîte de composant**, construite comme le nœud d'un éditeur de nœuds (ADR 0021,
  « Rendu ») :
  - une barre de titre teintée par le rôle, avec l'icône et le nom ;
  - un corps neutre, avec le rôle écrit et la taille ;
  - un nom accessible qui reprend les trois ;
  - un état survolé (contour plus marqué, fond inchangé), un état sélectionné (`aria-current`,
    contour doublé) et un état atténué (barre de titre sans teinte, nom en graisse normale) ;
  - les tirets ne veulent dire qu'une chose dans tout le système, une arête approchée : ni les
    boîtes atténuées ni les cadres ouverts n'en ont ;
- **arête** :
  - confirmée par LSP ou SCIP : trait plein ;
  - vue par tree-sitter seulement : tirets, plus pâle. Les tirets suffisent à distinguer les deux
    arêtes : la couleur n'est pas seule ;
  - une seule épaisseur : la spec la réserve au trafic mesuré pendant les tests (Phase 8) ;
  - le trait plein veut dire « sûre » : déclarée dans un manifeste (`Cargo.toml`,
    `package.json`), ou confirmée par un serveur de langage. Sur la carte d'ouverture de
    Timonier, toutes les arêtes pleines viennent des manifestes (relecture n° 2, I-b) ;
  - une légende en mots, avec le sens des arêtes : elles sortent d'une boîte à droite (un
    point) et entrent dans l'autre à gauche (un triangle) ;
  - les arêtes de l'élément survolé prennent sa couleur, et un liseré du fond les détache des
    lignes qu'elles croisent : pas la couleur seule ;
- **badge** : une forme par ton (point, croix, triangle, coche) et des mots qui disent le sens ;
- **état vide** pédagogique : titre, texte, détail du programme, action ; annoncé quand il suit
  le démon ;
- **info-bulle** : au survol et au focus, fermée par Échap (WCAG 1.4.13) ;
- **barre d'outils** : un seul arrêt de tabulation, puis les flèches, Début et Fin (modèle
  WAI-ARIA).

Les icônes sont des SVG en ligne de 16 px, dans la couleur du texte, sauf celles des rôles, dans
la couleur de leur rôle. Elles sont décoratives pour les lecteurs d'écran : le mot à côté dit
la même chose. Aucune police d'icônes n'est ajoutée.

**Page de démonstration** (`webview/demo.html`, en développement seulement, hors du paquet)
- Elle montre chaque composant dans chaque état.
- `?theme=` y applique les variables d'un thème, `?locale=fr` le français.

**Thèmes réels**
- `editors/vscode/scripts/capture-themes.mjs` lance le VSCode des tests d'intégration une fois
  par thème intégré. Il ouvre la carte, puis lit, par le protocole DevTools, les variables
  `--vscode-*` et la classe du `body` que reçoit la webview.
- Les six thèmes : Light 2026 et Dark 2026 (par défaut dans VSCode 1.142), Light Modern et Dark
  Modern, contraste élevé sombre et clair.
- Le script prend le VSCode le plus récent que les tests d'intégration ont téléchargé. Les
  captures actuelles viennent de VSCode 1.142.0.
- Ils sont versionnés dans `webview/test/themes/`, et capturés à nouveau quand VSCode change ses
  thèmes.

**Vérification** (`npm run test:a11y`, Playwright et Chromium, dans la CI sous Linux), sur la
page de démonstration, dans chaque thème :
- axe-core, règles WCAG 2.0 à 2.2, niveaux A et AA, en anglais et en français. Aucune
  violation n'est permise. Un contraste qu'axe ne sait pas trancher compte comme un échec ;
- les contours des boîtes, les arêtes, les broches de la carte (sur le fond et sur la barre de
  titre), les icônes des badges et des rôles, la légende et l'anneau de focus doivent atteindre
  3:1 (WCAG 1.4.11), ce qu'axe ne mesure pas. Le test compose les couches transparentes avant
  de calculer le rapport ;
- sur la carte, le même test mesure aussi l'état survolé de chaque élément, une fois les
  transitions finies ;
- sous `prefers-reduced-motion`, les transitions durent 0 s.

## Alternatives écartées

- **@vscode-elements/elements** pour les composants : ses web components ne couvrent ni la
  boîte de composant, ni les arêtes, ni les badges de sens. La carte (React Flow) en aurait de
  toute façon besoin en React. Il reste possible pour des formulaires plus tard.
- **Des valeurs de thème recopiées à la main** : elles dériveraient de VSCode sans que rien ne
  le voie. Les valeurs capturées sont celles que la webview reçoit vraiment.
- **axe-core dans jsdom** : sans rendu, sa règle de contraste rend « incomplet ».
- **axe-core embarqué dans la webview du paquet**, pour tester dans VSCode même : ce serait du
  code de test livré, sous MPL-2.0. La page de démonstration teste les mêmes composants avec les
  mêmes variables.
- **La couleur seule pour la précision des arêtes** : interdit par la spec, et illisible pour
  les daltoniens.

## Conséquences

- Chaque nouveau composant passe par les jetons, et se montre sur la page de démonstration. Le
  test de contraste le vérifie dans les six thèmes.
- Un nouveau thème de VSCode : relancer la capture, puis les tests. Un contraste qui tombe
  sous le seuil fait échouer la CI.
- Les thèmes de tiers ne sont pas vérifiés : ils dépendent de leurs auteurs.
- La carte (4.7) dessine ses nœuds avec la boîte de composant et ses arêtes avec `EdgeLine`.
