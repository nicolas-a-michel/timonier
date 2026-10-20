# Timonier pour VSCode

Timonier permet de piloter des agents IA sans lire tout le code qu'ils produisent : architecture
vivante, tableau blanc d'intention, performances mesurées pendant les tests, plan et todo en direct.

**État : développement (version 0.1.0), non publiée.** Linux et macOS.

## Ce qui marche

- L'extension lance le démon du projet (le programme `timonier`), et le relance s'il s'arrête.
  Son journal est dans le canal de sortie « Timonier ».
- L'icône Timonier (une barre à roue) de la barre d'activité ouvre la vue d'ensemble. Son bouton
  « Ouvrir la carte », comme la commande « Timonier : Ouvrir la carte », ouvre la carte
  d'architecture du projet, en Rust ou en TypeScript :
  - quelques boîtes nommées par rôle à l'ouverture (point d'entrée, interface, modèle de
    données, persistance…) ; un clic ou Entrée ouvre une boîte sur son contenu, jusqu'aux
    symboles ;
  - les liens sont les dépendances et les appels : ils sortent d'une boîte à droite et entrent
    dans l'autre à gauche. En tirets, ils ne sont vus que par tree-sitter, donc approchés. En
    trait plein, ils sont sûrs : déclarés dans un manifeste (`Cargo.toml`, `package.json`), ou
    confirmés par un serveur de langage de VSCode. Un fichier Rust ou TypeScript modifié mais
    pas encore enregistré suspend ces confirmations, qui reprennent à l'enregistrement ;
  - la carte suit le code à chaque sauvegarde, sans déplacer ce qui n'a pas changé ;
  - la disposition est calculée, mais chaque boîte se déplace à la main, à la souris ou avec
    Alt et une flèche ; « Réinitialiser la disposition » revient aux positions calculées ;
  - « Aller au code » (Maj+Entrée, double-clic) ouvre le fichier à côté de la carte ;
  - « Marquer comme revu » fige la carte telle que vous l'avez revue. Ensuite, chaque boîte
    nouvelle ou modifiée depuis le dit en toutes lettres, comme chaque lien nouveau et chaque
    boîte d'un cycle ajouté, ou dont le fan-in ou le fan-out a bondi. Un résumé compte et liste
    les changements, suppressions comprises ; « Mettre en avant les changements » estompe le
    reste. Seuls les changements du code comptent : ni ce que la seule vérification d'un
    serveur de langage fait apparaître, ni les fichiers que Timonier regroupe autrement. Une
    boîte n'est nouvelle ou supprimée que si du code arrive ou part avec elle, une crate avec
    son manifeste ; un dossier ou une crate renommés sont modifiés ;
  - tout se fait au clavier, et chaque action a sa commande dans la palette.
- Elle suit le thème de VSCode, clair, sombre ou à contraste élevé, en anglais ou en français.

## Réglages

- `timonier.daemon.path` : le chemin du programme `timonier`. Vide, celui du `PATH`.
- `timonier.map.density` : la densité de la carte, `comfortable` (confortable, par défaut) ou
  `compact` (boîtes plus petites, espacements resserrés).

Code source et feuille de route : <https://github.com/nicolas-a-michel/timonier>.
Licence : MIT.
