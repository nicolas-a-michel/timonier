# 0054 — Ce que la carte dit d'un symbole : sa sorte et sa signature

- Statut : Accepté (2026-10-21, Phase 8 validée)
- Date : 2026-10-21
- Phase : 8
- Liens : spec « Expérience et design » (un même langage visuel ; la couleur jamais seule ;
  détail au survol), ADR 0017, 0020, 0021 ; tâche T-114 (demande de l'humain, 2026-10-21)

## Contexte

Ouvert jusqu'aux symboles, la carte disait « Symbole » pour une fonction comme pour un champ,
avec la barre de titre neutre des boîtes sans rôle. L'humain a demandé une couleur par sorte et
une infobulle qui dise s'il s'agit d'une fonction ou d'une variable, et le type d'une variable.
Le graphe a déjà la sorte (`function`, `method`, `type`, `trait`, `field`, `constant`, `module`)
et la signature normalisée du pack, qui porte le type d'un champ (`pub size: u32`) ou d'une
constante (`pub const K: u32 =`).

## Décision

- **Données** : `ViewNode.signature` (vue de la carte) porte la signature du nœud, `null` hors
  symboles ; rien d'autre ne change dans le protocole.
- **Couleur** : un symbole prend, à la place d'un rôle (il n'en a pas), la couleur que l'IDE
  donne à sa sorte dans la vue « Structure » : `--vscode-symbolIcon-{function,method,class,
  interface,field,constant,module}Foreground`, en jetons `--tim-symbol-*` mêlés au texte comme
  ceux des rôles, la barre de titre teintée à 22 %, sans teinte en contraste élevé. L'orange des
  types est mêlé davantage (65 %) pour garder 3:1 sur les thèmes clairs.
- **Forme et mot** : une icône par sorte (`SymbolIcon`), et la sorte en toutes lettres à la place
  du rôle dans le corps de la boîte ; la signature en police de code, sur une ligne.
- **Infobulle** : « Champ : pub size: u32 », la sorte puis la signature, en `title` de la boîte
  et du cadre ; les lecteurs d'écran et le panneau de détail disent la même chose.

## Alternatives écartées

- **Une palette propre à Timonier** : un sens de plus à apprendre ; celle de l'IDE est déjà
  connue du développeur et suit son thème.
- **Extraire le type seul** (« u32 ») : une règle par langage, dans la vue ; la signature du
  pack le dit déjà, et dit aussi les paramètres d'une fonction.
- **Une infobulle dessinée** : le `title` natif suffit pour une ligne, et ne demande ni focus ni
  gestion du survol.

## Conséquences

- Les teintes des symboles peuvent ressembler à celles de rôles (le violet des fonctions, le
  magenta du modèle de données) ; l'icône et le mot les distinguent, et un symbole n'a jamais de
  rôle.
- Un pack qui ne donne pas de signature laisse l'infobulle à la seule sorte.
