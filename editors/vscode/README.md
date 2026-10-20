# Timonier pour VSCode

Timonier permet de piloter des agents IA sans lire tout le code qu'ils produisent : architecture
vivante, tableau blanc d'intention, performances mesurées pendant les tests, plan et todo en direct.

**État : développement (version 0.1.0), non publiée.** Linux et macOS.

## Ce qui marche

- L'extension lance le démon du projet (le programme `timonier`), et le relance s'il s'arrête.
  Son journal est dans le canal de sortie « Timonier ».
- L'icône Timonier (une barre à roue) de la barre d'activité ouvre la vue d'ensemble, le
  pont. Il répond en quelques secondes à trois questions :
  - où en est le plan : le jalon en cours, son avancement, la session d'agent ouverte ;
  - que s'est-il passé depuis votre dernière revue : boîtes nouvelles, modifiées, supprimées,
    dépendances, sessions d'agent et fichiers à relire ;
  - où regarder : d'abord ce qui demande une décision (tâche bloquée, fichier illisible, cycle
    ajouté), puis les boîtes à relire, les non suivies en tête. Un clic ouvre la carte sur la
    boîte ; la commande « Où regarder » fait de même depuis la palette.
- Le bouton « Ouvrir la carte » du pont, comme la commande « Timonier : Ouvrir la carte »,
  ouvre la carte d'architecture du projet, en Rust ou en TypeScript :
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
    nouvelle ou modifiée depuis le dit en toutes lettres (une étoile et « Nouveau », un point
    et « Modifié » ; un rôle fermé dit « 2 modifiés »), comme chaque lien nouveau et chaque
    boîte d'un cycle ajouté, ou dont le fan-in ou le fan-out a bondi ; la légende explique ces
    marques. Un bandeau au-dessus de la carte compte les changements depuis la revue et les
    liste, suppressions comprises ; « Mettre en avant les changements » estompe le reste. Seuls les changements du code comptent : ni ce que la seule vérification d'un
    serveur de langage fait apparaître, ni les fichiers que Timonier regroupe autrement. Une
    boîte n'est nouvelle ou supprimée que si du code arrive ou part avec elle, une crate avec
    son manifeste ; un dossier ou une crate renommés sont modifiés ;
  - « Dette de revue », dans la barre d'outils, montre sur chaque boîte combien de ses fichiers
    ont changé depuis votre dernière revue et restent à relire (« 9 fichiers à relire »), en
    trois paliers ; ce qui a changé sans session d'agent ouverte est hachuré et dit
    « non suivi » ;
  - tout se fait au clavier, et chaque action a sa commande dans la palette.
- La vue « Plan, todo et sessions », dans la barre latérale secondaire, à droite (« Afficher la
  barre latérale secondaire » si elle est fermée), suit l'agent en direct, à chaque appel de ses
  outils MCP. Large, elle montre deux colonnes collées, étroite, l'une sous l'autre :
  - « Plan » : les jalons, leur avancement (« 2 sur 5 faites » et une barre) et leurs tâches,
    chacune avec une forme selon son état (à faire, en cours, bloquée, faite, abandonnée) ;
  - « Todo » : la todo de la session ouverte, qui se remplit quand l'agent découpe le travail
    et se coche au fil de l'exécution ;
  - « Sessions » : le journal, la session ouverte d'abord, avec son objectif, ses tâches, les
    composants touchés et les dépendances ajoutées ou retirées (en direct pendant la session),
    sa todo et son résumé de fin.

  Un fichier de `.timonier/` modifié à la main s'y voit aussi ; s'il ne se lit plus, le
  panneau le dit.
- Elle suit le thème de VSCode, clair, sombre ou à contraste élevé, en anglais ou en français.

## Réglages

- `timonier.daemon.path` : le chemin du programme `timonier`. Vide, celui du `PATH`.
- `timonier.map.density` : la densité de la carte, `comfortable` (confortable, par défaut) ou
  `compact` (boîtes plus petites, espacements resserrés).

Code source et feuille de route : <https://github.com/nicolas-a-michel/timonier>.
Licence : MIT.
