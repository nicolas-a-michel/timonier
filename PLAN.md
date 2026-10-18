# PLAN — Timonier (MVP)

Plan de réalisation du MVP défini dans [docs/spec.md](docs/spec.md) (section « Découpage »),
selon les phases du prompt de démarrage. La spec fait autorité. Les décisions techniques non
couvertes par la spec sont dans [docs/adr/](docs/adr/README.md).

Format (voir [ADR 0002](docs/adr/0002-format-de-plan-md.md)) :

- une section `## Phase N — Titre` par phase, avec une ligne `Statut : à faire | en cours | terminée` ;
- une case de premier niveau par tâche, avec son critère d'acceptation ;
- en fin de phase, une ligne `Décisions :` et une ligne `Écarts :`.

Le hook de démarrage de session lit ce format.

---

## Questions ouvertes

Contradictions ou manques de la spec. J'attends tes réponses : je n'invente pas. Chaque
question indique la phase qu'elle bloque et ma recommandation.

**Q1 — Licence du projet** — ✅ répondu le 2026-10-14 : **MIT**, titulaire Nicolas Michel.
- Options : MIT OR Apache-2.0 (convention Rust), Apache-2.0 seule, AGPL-3.0, licence
  non libre (BSL…).
- Contraintes connues : elkjs 0.12 est sous EPL-2.0 OR GPL-3.0, un copyleft faible au niveau
  du fichier, compatible avec toutes ces options. tldraw étant écarté (Q3), aucune dépendance
  prévue n'impose de choix.
- Titulaire des droits : Nicolas Michel. Projet public.

**Q2 — Dépôt GitHub pour la CI** — ✅ répondu le 2026-10-14
- Réponse : propriétaire Nicolas Michel, dépôt public créé par toi :
  <https://github.com/nicolas-a-michel/timonier>.
- Conséquence : le workflow de CI est écrit en Phase 1 et ses commandes sont vérifiées en
  local (Linux). Le critère « vert en CI sur Linux et macOS » sera constaté au premier push.

**Q3 — Licence de tldraw** — ✅ répondu le 2026-10-14 : **pas de tldraw**. On utilise autre
chose. Le remplaçant sera choisi au début de la Phase 7, par une ADR soumise à validation.
Rappel de ce qui avait été vérifié :
- Vérifié le 2026-10-14 sur tldraw 5.5.2 : le SDK n'est pas open source. Hors développement,
  il exige une clé de licence.
- Une webview VSCode publiée compte comme production. Sans clé, le canevas s'affiche cinq
  secondes puis devient vide.
- Les clés possibles :
  - « hobby » : discrétionnaire, non commerciale, filigrane « made with tldraw » obligatoire ;
  - commerciale : payante ;
  - essai : 100 jours.
- Tout fork de l'extension aurait besoin de sa propre clé.
- Options :
  - (a) garder tldraw et obtenir une clé ;
  - (b) Excalidraw (licence MIT, à vérifier), que la spec juge moins adapté aux formes
    typées ;
  - (c) un tableau blanc construit sur React Flow (MIT) avec une bibliothèque de dessin à
    main levée sous licence permissive : plus de travail en Phase 7, mais aucune contrainte.
- Candidats pour la Phase 7 :
  - **Excalidraw** : licence MIT d'après GitHub le 2026-10-14 ; la spec l'avait jugé moins
    adapté aux formes typées ;
  - **un tableau blanc construit sur React Flow**, déjà dans la pile : cohérent avec « un seul
    langage visuel », et les formes typées y sont des nœuds de première classe.
- Piste privilégiée à ce stade : React Flow.

**Q4 — Place des composants dans l'arbre strict** — ✅ répondu le 2026-10-14 : option (a).
- La spec dit que la hiérarchie `parent` est un arbre strict contenant Repo, Component,
  Package, File et Symbol. Elle dit aussi que les composants viennent d'une agrégation
  structurelle (crates, packages, dossiers) suivie d'un clustering Louvain/Leiden, avec une
  carte récursive (« des boîtes dans des boîtes »).
- Elle ne dit pas où se placent les packages et les dossiers par rapport aux composants, ni si
  un cluster peut chevaucher deux packages.
- Options :
  - (a) les composants sont dans l'arbre (Repo → Component* → Package → Component* → File →
    Symbol), et le clustering ne coupe jamais un package : il regroupe des packages entiers
    en haut et découpe à l'intérieur d'un package ;
  - (b) arbre physique strict (Repo → Package → File → Symbol), plus une hiérarchie de
    composants séparée, en appartenance, qui peut chevaucher les packages.
- Recommandation : (a), plus fidèle au texte (arbre strict, composants comme nœuds).

**Q5 — Accès au LSP** — ✅ répondu le 2026-10-14 : option (c), en commençant par (a).
- La spec parle du « LSP (déjà lancé par l'IDE) », mais le prompt place un « client LSP »
  dans `timonier-index`, côté démon.
- Options :
  - (a) l'extension relaie au démon les requêtes de l'IDE (`vscode.executeDefinitionProvider`,
    `executeReferenceProvider`, hiérarchie d'appels) : un seul serveur de langage en mémoire,
    mais pas de LSP hors IDE (CLI, CI) ;
  - (b) le démon lance ses propres serveurs (rust-analyzer, serveur TypeScript) : autonome,
    mais la mémoire double ;
  - (c) les deux, derrière une même interface.
- Recommandation : (c) avec (a) d'abord pour le MVP, et (b) quand la CI en aura besoin (v3).

**Q6 — Quel LLM, appelé par qui ?** — ✅ répondu le 2026-10-14 : (c) par défaut, enrichi
par (a) ; pas de clé d'API dans le démon.
- La spec dit : « Le LLM nomme et résume les clusters », les rôles sont « confirmés par le
  LLM », et le dessin libre est « interprété par un modèle vision ». Elle ne dit pas d'où
  vient ce modèle.
- Options :
  - (a) l'agent connecté via MCP : le démon expose des clusters non nommés et l'agent soumet
    noms et rôles par un outil d'écriture structuré, puis lit l'image du croquis via
    `get_intended_architecture` ;
  - (b) le démon appelle lui-même une API de LLM avec une clé de l'utilisateur ;
  - (c) heuristiques seules dans le MVP.
- Recommandation : (c) par défaut, enrichi par (a). Pas de clé d'API dans le démon.

**Q7 — Coût de l'IA : MVP ou v2 ?** — ✅ répondu le 2026-10-14 : collecte par session et
par tâche en Phase 9 ; le calque par composant reste en v2.
- Le prompt le met en Phase 9. Le « Découpage » de la spec le classe en v2, et seule sa
  méthode de mesure est décidée.
- Recommandation : en Phase 9, la seule collecte par session et par tâche via la télémétrie
  OpenTelemetry de Claude Code. Le calque par composant reste en v2.

**Q8 — Assistant « Nouveau projet piloté »** (Phase 9)
- Il est décrit dans « Initialisation » mais absent de la liste du MVP et du prompt.
- Recommandation : en Phase 9, sinon v2. La détection à l'ouverture d'un dépôt non configuré
  est prévue en Phase 6.

**Q9 — Projet pilote du critère de sortie du MVP** (Phase 9)
- Critère : « un projet réel piloté de bout en bout sans lire le code ligne à ligne ».
- À trancher : lequel ?

**Q10 — Outils MCP de lecture du suivi** (Phase 6)
- La liste de la spec n'a aucun outil pour relire le plan ou la todo (`get_plan`,
  `get_todo`), alors que l'agent doit les lire sans ouvrir les fichiers à la main.
- Recommandation : les ajouter. La spec dit que les noms sont provisoires.

**Q11 — `PLAN.md` et `.timonier/plan.json`** (Phase 6)
- À partir de la Phase 6, le plan est tenu via MCP.
- Options : migrer ce plan dans `plan.json` et générer `PLAN.md`, ou garder `PLAN.md` comme
  feuille de route humaine et `plan.json` pour les tâches.
- À trancher au début de la Phase 6.

**Q12 — Identifiant d'éditeur sur la Marketplace VSCode** (avant toute publication) — ⏸
reportée le 2026-10-16 (« la Q12 attendra »). `nicolas-a-michel` reste provisoire.
- Le paquet utilise `publisher: nicolas-a-michel`, qui n'existe pas encore sur la
  Marketplace. Cet identifiant doit être créé par toi sur marketplace.visualstudio.com.
- Il ne bloque que la publication, pas le développement.
- À trancher : garder `nicolas-a-michel`, ou en choisir un autre (par exemple `timonier`).

**Q13 — Langue de l'interface** — ✅ répondu le 2026-10-16 : **bilingue** (anglais de base,
traduction française) pour l'instant ; d'autres langues seront ajoutées plus tard. ADR 0006
acceptée.
- La spec ne la fixe pas.
- J'ai mis en place l'anglais comme langue de base, avec une traduction française qui suit la
  langue de VSCode.
- À trancher : garder ce choix, ou passer au français seul.

**Q14 — Fichiers hors de tout package** — ✅ répondu le 2026-10-16 : extension gardée. Un
fichier hors de tout package se place sous le Repo ou sous un Component de haut niveau.
- Ta réponse à Q4 place les fichiers sous un Package, ou sous un Component à l'intérieur d'un
  package.
- J'ai aussi permis un fichier directement sous le Repo ou sous un Component de haut niveau,
  pour les fichiers qui n'appartiennent à aucun package (scripts à la racine, par exemple).
- À trancher : garder cette extension, ou refuser ces fichiers (ils ne seraient alors pas
  indexés).

**Q15 — Validation des ADR de la Phase 2 et du critère 2.1** — ✅ répondu le 2026-10-16 :
ADR 0008 à 0011 acceptées. Pour la suite, option (a) : les ADR sont écrites (« Proposé ») avant
le code, et tu les acceptes ensemble à la validation de la phase.
- Le critère de 2.1 disait « ADR acceptées avant le code qui en dépend ». Je l'ai réécrit en
  « écrites avant le code, acceptées à la validation de la phase ».
- Les ADR 0008 à 0011 sont donc implémentées sans ton acceptation préalable. La relecture l'a
  signalé à juste titre.
- L'ADR 0009 a beaucoup évolué au fil des trois relectures. Les sections « Propriété » et
  « Contenu de référence des observations » méritent d'être lues : propriété à la plus petite
  clé pour un nœud, à la source la plus fiable pour une arête, contributions en attente.
- À trancher : accepter les ADR 0008 à 0011 (avec Q14) et le nouveau libellé, ou demander des
  changements avant la suite.

**Q16 — Zoom sémantique à la molette** (Phase 4, relecture I4) — ✅ répondu le 2026-10-16 :
**(a)**, on garde le clic pour l'instant (« on garde le click pour l'instant »).
- La spec dit « plus on zoome, plus le niveau de détail change (composants → fichiers →
  symboles) » et « cliquer ou zoomer sur une boîte ouvre son contenu ».
- Aujourd'hui, seuls le clic et Entrée ouvrent une boîte. La molette ne fait que grossir ou
  réduire ; sous un zoom de 0,5, les boîtes ne montrent que leur nom. Je l'ai décidé dans
  l'ADR 0021, alors que c'est un comportement produit : la relecture a raison de le signaler.
- Options :
  - (a) garder le clic seul. C'est prévisible : la carte ne change pas sans qu'on le demande ;
  - (b) ouvrir aussi à la molette (recommandé, car c'est la lettre de la spec). Zoomer au-delà
    d'un seuil, par exemple 1,5, sur une boîte l'ouvre. Dézoomer sous 0,5 referme ce qui a été
    ouvert ainsi. Le risque : la mise en page bouge sous le pointeur, sans clic.
- À trancher : (a) ou (b). Rien n'est changé avant ta réponse.
- Suite : l'ADR 0021 le note ; la question pourra revenir après la Phase 5.

**Q17 — `tim daemon stop` avec VSCode ouvert** (Phase 4, relecture m2) — ✅ répondu le
2026-10-16 : « ok pour le moment », confirmé le 2026-10-17 (« oui ») : on garde le comportement
actuel pour l'instant, l'extension relance le démon (option b).
- L'extension voit la connexion se fermer et relance le démon 1 s plus tard. Un arrêt demandé
  ne se distingue pas d'un plantage. Ni la spec ni l'ADR 0019 ne décrivent ce cas.
- Options :
  - (a) un arrêt demandé est respecté (recommandé). Le démon prévient ses clients avant de se
    fermer ; l'extension ne relance pas, et la carte affiche « Démon arrêté » avec un bouton
    « Relancer » ;
  - (b) l'extension relance toujours : le démon est le sien tant qu'une fenêtre est ouverte.
- À trancher : (a) ou (b).

**Q18 — `.timonier/` créé avant `timonier init`** (Phase 4, relecture m18) — ✅ répondu le
2026-10-16 : **(a)**, comportement gardé pour le MVP (« ok »).
- Ouvrir la carte dans n'importe quel dépôt lance le démon. Il crée alors
  `.timonier/graph.sqlite` et un `.gitignore` qui l'exclut de git.
- Ensuite, l'extension s'active d'elle-même à chaque ouverture de ce dossier
  (`workspaceContains:.timonier`).
- La spec réserve `.timonier/` aux fichiers de suivi de `timonier init`, et propose `init` à
  l'ouverture d'un dépôt « non configuré ». La Phase 6 devra distinguer ce cas.
- Options :
  - (a) garder ce comportement (recommandé pour le MVP). La carte marche partout, sans
    préalable, et rien n'entre dans git ; la Phase 6 juge « configuré » d'après
    `plan.json`, pas d'après le dossier ;
  - (b) avant `init`, mettre la base hors du dépôt (sous `~/.cache/timonier/`), et ne créer
    `.timonier/` qu'avec `init` ;
  - (c) demander avant la première indexation d'un dépôt.
- À trancher : (a), (b) ou (c).

**Q20 — Déplacer les boîtes à la souris** (Phase 4, relecture n° 2, m-k) — ✅ répondu le
2026-10-17 : « on les calcule par défaut, mais il faut pouvoir les déplacer ». Option **(b)** :
positions calculées, et chaque boîte déplaçable à la main. Où garder une position déplacée :
H13.
- Dans les Blueprints, on déplace les nœuds à la main. Sur la carte de Timonier, les positions
  sont calculées, et rien ne se déplace à la souris. Je l'ai décidé dans l'ADR 0021, alors que
  c'est un comportement produit.
- Options :
  - (a) garder les positions calculées (recommandé pour le MVP). La mise en page reste stable
    et identique pour tous ; glisser sur une boîte fait défiler la carte ;
  - (b) permettre de déplacer une boîte. Sa position serait gardée par la webview, et la mise
    en page incrémentale la respecterait. Il faudrait décider où la garder (par machine, ou
    dans le dépôt) et que faire quand le code change autour.
- À trancher : (a) ou (b).

**Q21 — Les boîtes déplacées : où, et quand le code change** (Phase 4, relecture n° 3, m-2) —
✅ répondu le 2026-10-17 : « oui ». Les deux choix ci-dessous sont gardés : les positions dans
l'état de l'espace de travail de VSCode (H13, confirmée), et une boîte déplacée qui reste où tu
l'as laissée quand le code change.
- Ta réponse à Q20 ne tranche pas deux points. Je les ai réglés ainsi, à confirmer :
  - **où** : dans l'état de l'espace de travail de VSCode, sur ta machine, pas dans le dépôt
    (H13). Pas partagé avec l'équipe, et rien dans git ;
  - **quand le code change** : une boîte déplacée reste où tu l'as laissée ; une boîte
    nouvelle ou qui grandit va autour d'elle ; ce qu'elle recouvre descend sous elle. Si la
    boîte disparaît, sa position est gardée pour son retour. « Revenir aux rôles » garde les
    positions ; seule « Réinitialiser la disposition » les oublie.
- Autre option pour le premier point : un fichier `.timonier/layout.json` dans le dépôt,
  versionné et partagé.
- À trancher : garder ces choix, ou changer l'un d'eux.

**Q25 — Worktree en chemins relatifs : quand fixer le dossier courant du démon** (Phase 4,
relectures n° 17 et 18, m17-1, m18-1) — ✅ répondu le 2026-10-18 : **plus tard**.
- Dans un worktree créé en chemins relatifs (git 2.48 et plus), `.git/info/exclude` ne
  s'applique que si le démon est lancé depuis le haut du worktree : la crate `ignore` lit ce
  chemin depuis le dossier courant du processus (ADR 0023).
- Le correctif, que le démon prenne lui-même ce dossier comme dossier courant, attend la phase
  où Timonier créera les worktrees des agents : en chemins absolus, ou avec ce correctif.

**Q24 — Clore la tâche 4.12 sur « prête avec réserves »** (Phase 4, relecture n° 18) — ✅
répondu le 2026-10-18 : **clore ainsi**.
- La relecture n° 18 ne trouve rien qui empêche « prête » au regard des critères de la phase,
  de la spec et des ADR, et aucune régression ; ses quatre mineurs sont traités (un test, des
  textes). Le critère de 4.12 demandait « prête ».

**Q23 — Snapshots de commit faux dans deux cas : quand les corriger** (Phase 4, relecture
n° 14, I14-1) — ✅ répondu le 2026-10-18 : **en Phase 5**.
- Le snapshot d'un commit est faux quand un manifeste n'est modifié que dans l'arbre de travail
  (crate renommée, dépendance ajoutée sans commit), ou quand `src/lib.rs` n'existe que d'un
  côté : le gel garde les paquets de l'arbre de travail. Défaut de la tâche 3.7 (ADR 0015).
- La carte n'est pas touchée : elle montre la tête. Ces snapshots ne servent qu'au diff de revue
  de la Phase 5, qui le corrigera avant de les lire (tâche 5.4).
- Le troisième cas relevé (un fichier revenu à sa version commitée sans être relu) est corrigé
  en Phase 4.

**Q22 — Vérification suspendue pendant qu'un fichier source n'est pas enregistré** (Phase 4,
relecture n° 6, m6-2) — ✅ répondu le 2026-10-18 : **(a)**, garder ce réglage. Tant qu'un
fichier source est modifié sans être enregistré, aucune vérification n'est faite ; elles
reprennent à l'enregistrement ou au retour arrière.
- Les serveurs de langage de VSCode répondent sur ce que montre l'éditeur, enregistré ou non.
  Le démon, lui, indexe le disque. Une réponse sur un tampon modifié serait enregistrée comme
  vérifiée pour le contenu du disque : c'est le défaut I-α de la relecture n° 5.
- Je l'ai réglé ainsi, à confirmer : tant qu'un fichier source (Rust, TypeScript) est modifié
  sans être enregistré, aucune vérification n'est faite. Ce qui attend reprend à
  l'enregistrement ou à l'annulation des modifications. La carte garde ce qu'elle montre ; les
  liens d'un fichier affiché pendant ce temps restent en tirets jusque-là. Une note ou un
  `PLAN.md` en cours d'écriture ne suspendent rien.
- Autres options :
  - (b) ne suspendre que ce qui touche le fichier modifié : moins sûr, car un appel retiré dans
    le tampon ne se voit pas dans la réponse sur un autre fichier ;
  - (c) en plus, proposer d'activer l'enregistrement automatique de VSCode, qui rend la pause
    très courte.
- À trancher : garder ce réglage, (b) ou (c).

**Q19 — Critère de 4.5 reformulé** (Phase 4) — ✅ répondu le 2026-10-17 : **(b)**, « et on
optimise le code à fond ». Le critère d'origine reste : définitions et références obtenues par
l'API de VSCode et reçues par le démon. Compris comme : vérifier aussi les usages autres que les
appels, et réduire au plus le nombre de requêtes au serveur de langage. L'ADR 0022 le décide ;
elle complète l'ADR 0014.
- **De quoi il s'agit.** La tâche 4.5 fait passer les serveurs de langage de VSCode
  (rust-analyzer, le serveur TypeScript) au démon. Ils confirment ce que tree-sitter devine.
  Le critère écrit demandait « définitions et références obtenues via l'API VSCode et reçues
  par le démon ».
- **Ce qui est fait et testé : les appels.** Pour chaque fonction d'un fichier, le démon
  demande à VSCode quelles fonctions elle appelle. C'est la « hiérarchie d'appels », celle de
  la commande « Afficher la hiérarchie d'appels ». La réponse donne, pour chaque fonction
  appelée, l'endroit exact où elle est définie, et combien de fois elle est appelée. C'est ce
  qui fait passer un lien de la carte des tirets au trait plein.
- **Ce qui n'est pas fait : les autres usages.** Un type, une constante ou une interface d'un
  autre fichier, utilisé sans être appelé, n'est pas vérifié. Il faudrait demander à VSCode
  « où est défini ce nom ? » (Aller à la définition) pour chaque usage, ou « qui utilise ce
  symbole ? » (Rechercher toutes les références).
  - Exemple : `MapView.tsx` utilise le type `ViewData` de `viewModel.ts`, sans rien appeler
    de ce fichier. Ce lien reste « vu par tree-sitter seulement », donc en tirets, même si
    VSCode sait exactement où est `ViewData`.
  - Un lien de la carte qui contient aussi un appel confirmé est en trait plein.
- **Pourquoi.** C'est ce que tu as accepté dans l'ADR 0014, en Phase 3. Vérifier les usages
  demanderait une requête « définition » par usage, des centaines par fichier. Et la mémoire du
  serveur de VSCode est partagée avec ton travail. SCIP (v2) donnera définitions et références
  d'un coup, sans ce coût.
- Options :
  - (a) accepter le nouveau libellé, « appels sortants obtenus par la hiérarchie d'appels de
    VSCode et reçus par le démon » (recommandé, cohérent avec l'ADR 0014). Les autres usages
    restent approchés jusqu'à SCIP ;
  - (b) garder le libellé d'origine. Il faut alors ajouter la vérification des usages, revenir
    sur l'ADR 0014, et accepter plus de requêtes au serveur de VSCode.
- À trancher : (a) ou (b).

## Hypothèses de travail

Ce sont mes interprétations. Elles valent tant que tu ne les corriges pas.

- **H1** — `webview/` est à la racine (réutilisable par d'autres front-ends, comme le
  prévoit la spec pour Zed ou une application autonome) ; l'extension est dans
  `editors/vscode/`.
- **H2** — Les « < 100 ms » de la Phase 3 se mesurent de la fin de l'anti-rebond (~300 ms,
  spec) jusqu'au graphe validé dans SQLite. Valeur p95, sur le dépôt Timonier, en build
  release, sur la machine de développement.
- **H3** — « Quelques secondes » pour indexer Timonier : cible ≤ 3 s à froid, en build
  release.
- **H4** — SCIP est en v2 (Découpage). La consolidation au commit du MVP se limite au
  re-clustering et au snapshot figé.
- **H5** — Le pont (vue d'accueil) arrive en Phase 6, quand plan, todo et diff de revue
  existent.
- **H6** — Le volet instrumentation de `timonier init` et le test de fumée tracé de
  `timonier doctor` se terminent en Phase 8, car ils dépendent du récepteur OTLP.
- **H7** — Le lien entre une tâche du plan et un composant du tableau blanc existe dès la
  Phase 6. Il ne peut être renseigné qu'après la Phase 7.
- **H8** — Plateformes du MVP : Linux et macOS, les cibles de la CI. Windows (named pipes)
  viendra plus tard. Sur macOS, le niveau 1 est réduit (pas de cgroups ni de procfs ; durée,
  CPU et mémoire max via rusage). Les compteurs d'instructions matériels ne sont visés que
  sur Linux.
- **H9** — Langues :
  - en anglais : code, commentaires, commits, ainsi que `CLAUDE.md`, les skills, les
    subagents et les messages des hooks, qui sont des fichiers pour les agents ;
  - en français : `PLAN.md`, les ADR, les rapports de relecture et la documentation
    utilisateur ;
  - sauf `README.md`, en anglais à ta demande (2026-10-16).
- **H10** — L'en-tête de boîte aux lettres est présent dès la Phase 6, avec zéro message non
  lu (la messagerie est en v2).
- **H11** — TypeScript est figé en 6.0.x : typescript-eslint 8.71 ne prend pas en charge
  TypeScript 7.
- **H12** — npm workspaces plutôt que pnpm, car `vsce` gère mal les liens symboliques de
  pnpm. Une ADR le confirmera en Phase 1.
- **H13** — Une boîte déplacée à la main (Q20) garde sa position dans l'espace de travail de
  VSCode, sur ta machine, et non dans le dépôt : la disposition est à toi, elle n'entre pas
  dans git. « Réinitialiser la disposition » revient aux positions calculées. Confirmée par
  ta réponse à Q21 (2026-10-17).

---

## Phase 0 — Équiper l'environnement Claude Code du projet

Statut : terminée (0.11 reste à confirmer par toi dans une nouvelle session)

- [x] 0.1 Lire `docs/spec.md` en entier — critère : contradictions et manques relevés dans
  « Questions ouvertes ».
- [x] 0.2 Vérifier dans la documentation officielle le format actuel des skills, hooks,
  subagents et `settings.json` — critère : sources et date citées dans l'ADR 0001.
- [x] 0.3 Vérifier existence, version et licence des paquets nommés par la spec — critère :
  tableau « Dépendances vérifiées » plus bas.
- [x] 0.4 Initialiser le dépôt git (branche `main`) et écrire `.gitignore` — critère :
  `.claude/state/` et `.claude/settings.local.json` sont ignorés.
- [x] 0.5 Écrire `CLAUDE.md` — critère : vision, carte du dépôt, commandes, règles, pointeurs,
  hooks documentés avec leur test manuel.
- [x] 0.6 Écrire les sept skills (`rust-daemon`, `graph-model`, `language-pack`,
  `vscode-webview-ui`, `mcp-tools`, `otel-instrumentation`, `adr`) — critère : frontmatter
  conforme à la doc, avec une description qui dit quand les utiliser.
- [x] 0.7 Écrire les hooks (`.claude/settings.json`, `.claude/hooks/`) — critère :
  `node --test .claude/hooks/hooks.test.mjs` vert, et chaque script exécuté à la main avec
  une entrée réelle.
- [x] 0.8 Écrire le subagent `phase-reviewer` — critère : outils en lecture seule (`Read`,
  `Grep`, `Glob`, `Bash`), rapport en français.
- [x] 0.9 Écrire les ADR 0001 et 0002, l'index `docs/adr/README.md` et le modèle d'ADR —
  critère : fichiers présents, index à jour.
- [x] 0.10 Rédiger et soumettre `PLAN.md` — critère : chaque tâche a un critère vérifiable.
- [ ] 0.11 Vérification par l'humain dans une nouvelle session (hooks, skills, subagent,
  CLAUDE.md) — critère de sortie du prompt :
  - le message de démarrage « Timonier · Phase … » s'affiche ;
  - `/hooks` liste SessionStart, UserPromptSubmit, PreToolUse, PostToolUse et Stop ;
  - `/skills` liste les sept skills ;
  - `/agents` liste `phase-reviewer` ;
  - `CLAUDE.md` suffit à reprendre le travail.
- [x] 0.12 Validation de ce plan et réponses à Q1–Q7 — critère : réponses consignées ici. Feu
  vert donné le 2026-10-14.

Décisions : [ADR 0001](docs/adr/0001-hooks-claude-code-en-node.md),
[ADR 0002](docs/adr/0002-format-de-plan-md.md).
Écarts :
- `git init` fait ici, car le dossier n'était pas un dépôt. Phase 0 commitée (`33e2d76`).
- Pour 0.11, ce qui a déjà été constaté dans la session de travail :
  - Claude Code a chargé les hooks à chaud, et la garde `intent.json` s'est déclenchée ;
  - les sept skills sont listés ;
  - `phase-reviewer` est proposé comme type d'agent.

  Le message de démarrage ne peut être vu que dans une nouvelle session.
- Corrigé avant le commit de la Phase 0 : la garde `intent.json` ne juge plus que la
  sous-commande qui écrit (faux positif constaté en réel).
- Corrigé pendant la Phase 1 :
  - deux références à un mauvais nom de fichier pour l'ADR 0002 dans les hooks ;
  - les erreurs de typecheck indiquent leur workspace ;
  - le hook de formatage couvre tous les types que la CI vérifie.

## Phase 1 — Squelette du dépôt

Statut : terminée (validée par toi le 2026-10-14)

Prérequis levés : licence MIT (Q1) ; dépôt public
<https://github.com/nicolas-a-michel/timonier> (Q2).

- [x] 1.1 Ajouter la licence MIT (« Copyright (c) 2026 Nicolas Michel ») — critère : fichier
  `LICENSE` présent ; `license = "MIT"` dans `Cargo.toml` et `package.json` ; `cargo metadata`
  et `npm pkg get license` renvoient `MIT`.
- [x] 1.2 Fixer la toolchain : `rust-toolchain.toml` (stable, rustfmt, clippy), édition 2024 et
  `rust-version` dans `[workspace.package]`, `rustfmt.toml` — critère : `cargo fmt --all
  --check` vert ; ADR toolchain et MSRV (stable actuelle 1.99, 1.96 installée en local).
- [x] 1.3 Créer le workspace Cargo avec les sept crates (`timonier-core`, `-store`, `-index`,
  `-daemon`, `-otlp`, `-mcp`, `-cli`) — critère : `cargo build --workspace` vert ; ADR sur
  le découpage, le sens des dépendances et le crate qui porte les types du protocole.
- [x] 1.4 Binaire `timonier` et alias `tim` (clap 4.6), avec `--version` et `--help` — critère :
  `timonier --version` et `tim --version` impriment la même chaîne (test assert_cmd) ; ADR
  sur le mécanisme d'alias.
- [x] 1.5 Lints du workspace (`[workspace.lints]`) — critère : `cargo clippy --workspace
  --all-targets -- -D warnings` vert.
- [x] 1.6 Un test de fumée par crate — critère : `cargo test --workspace` vert.
- [x] 1.7 Monorepo Node — critère : `npm ci && npm run lint && npm run typecheck && npm test`
  verts ; ADR gestionnaire de paquets et bundler de l'extension (H12). Contenu :
  - `package.json` racine, workspaces `editors/vscode` et `webview` ;
  - scripts `build`, `test`, `lint`, `typecheck`, `format` ;
  - TypeScript 6.0.x strict (H11), ESLint en configuration plate avec typescript-eslint,
    prettier.
- [x] 1.8 `webview/` : application React 19 + Vite 8 qui affiche un état vide (tokens `--tim-*`
  minimaux sur les variables VSCode) — critère : `npm run build -w webview` produit le
  bundle ; test vitest vert.
- [x] 1.9 `editors/vscode/` : extension avec la commande « Timonier : Ouvrir la carte », qui
  ouvre un panneau webview chargeant le bundle de `webview/` sous une CSP stricte (nonce,
  `localResourceRoots`) — critère : F5 (« Run Extension ») ouvre le panneau vide en thème
  clair et sombre ; un test d'intégration `@vscode/test-cli` exécute la commande et vérifie
  la création du panneau.
- [x] 1.10 Packaging — critère : `npx @vscode/vsce package` produit un `.vsix` que
  `code --install-extension` installe.
- [x] 1.11 Activer les hooks sur le squelette — critère : test manuel documenté dans
  `CLAUDE.md` (une erreur de type introduite bloque la fin de tour une fois). Changements :
  - le hook Stop appelle le script `typecheck` racine ;
  - le hook de formatage trouve prettier local.
- [x] 1.12 CI GitHub Actions sur ubuntu-latest et macos-latest — critère : chaque étape du
  workflow passe en local sous Linux. Le vert sur les deux OS sera constaté au premier push
  sur le dépôt public créé par toi (Q2). Étapes :
  - Rust : vérification du format, clippy `-D warnings`, `cargo test` ;
  - Node : `npm ci`, lint, typecheck, tests, build, package ;
  - tests de l'extension sous xvfb (Linux) ;
  - tests des hooks ;
  - caches cargo et npm.
- [x] 1.13 `README.md` minimal en français (ce qu'est Timonier, état, comment construire) et
  commandes de `CLAUDE.md` à jour — critère : relu.
- [x] 1.15 Icône Timonier dans la barre d'activité, ajoutée à ta demande du 2026-10-14 —
  critère : un conteneur Timonier avec une icône 24×24 d'une seule couleur, une vue « Vue
  d'ensemble » dont l'accueil propose « Ouvrir la carte », et un bouton de carte dans sa barre
  de titre ; tests unitaires du manifeste et test d'intégration verts ; ADR 0007.
  - Constat : 19 tests unitaires et 5 tests d'intégration verts.
  - L'icône a été rendue pixel par pixel à 24 px, et la version à 6 rayons retenue.
  - Le rendu réel est à regarder avec F5, en même temps que 1.9.
- [x] 1.14 Relecture par `phase-reviewer` — critère : verdict « prête », ou réserves traitées.
  Réserves traitées (voir plus bas) ; il n'y a pas eu de seconde relecture.

Critère de sortie : `cargo test` et le build de l'extension passent en CI (constaté au
premier push, voir Q2) ; l'extension s'installe en mode développement et affiche un panneau
vide.

Décisions : [ADR 0003](docs/adr/0003-decoupage-des-crates.md) (crates, dépendances, alias),
[ADR 0004](docs/adr/0004-toolchain-rust.md) (toolchain, MSRV 1.96),
[ADR 0005](docs/adr/0005-monorepo-node-et-build-de-l-extension.md) (npm, TS 6, esbuild, Vite,
tests, CSP), [ADR 0006](docs/adr/0006-langue-de-l-interface.md) (langue de l'interface,
acceptée le 2026-10-16 : Q13), [ADR 0007](docs/adr/0007-entree-dans-la-barre-d-activite.md) (icône et vue
d'ensemble dans la barre d'activité).

Constats (2026-10-14, Linux, machine de développement, après la relecture) :
- Rust : `cargo fmt --check` et `clippy -D warnings` verts ; `cargo test` : 9 tests verts ;
  `cargo +1.96 check` vert (MSRV). `timonier --version` et `tim --version` affichent tous deux
  `timonier 0.1.0`.
- Après suppression de tous les builds (état d'un checkout neuf), `npm test` est vert :
  - webview : 14 tests vitest, dont un vérificateur de tokens qui est lui-même testé ;
  - extension : 14 tests `node:test` (CSP nonce seul, échappement de chaque champ, manifeste,
    traductions complètes, sous-dossiers compris) ;
  - hooks : 24 tests.
- `npm run test:integration` (qui construit d'abord) donne 4 tests verts dans VSCode
  1.141.2 :
  - l'extension s'active et la commande existe ;
  - le bundle s'exécute sous la CSP stricte : la webview répond « ready » ;
  - le panneau ouvert est révélé et redevient l'onglet actif, sans second panneau.
- Lint, typecheck et `format:check` sont verts.
- `npm run package` produit `timonier.vsix`. Il contient le bundle, le manifeste, les
  `THIRD-PARTY-NOTICES.md` (React, react-dom, scheduler : MIT), les traductions, le README et
  la licence. L'installation réussit dans un profil temporaire.
- Le bloc « Testing by hand » de `CLAUDE.md`, exécuté tel quel, produit les sept sorties
  annoncées. En particulier, une erreur de type bloque la fin de tour une fois, puis n'est
  plus que signalée.

CI GitHub (commit `6d3e0f8`, run 38604588410) : 5 jobs verts au premier push.
- Rust sur ubuntu-latest et macos-latest ; MSRV 1.96.
- Extension et webview sur les deux OS : 4 tests d'intégration verts dans VSCode, sous xvfb
  sur Linux et directement sur macOS.
- `timonier.vsix` publié comme artefact du run.

Relecture `phase-reviewer` (verdict initial : « pas prête ») — tout est traité :
- B1 : la CI lançait les tests d'intégration sans webview construite. Corrigé :
  `test:integration` construit d'abord, et le scénario a été vérifié depuis un état propre.
- I1 : 1.9 décochée (contrôle visuel F5) ; 1.11 documentée et vérifiée.
- I2 : skills `vscode-webview-ui` et `rust-daemon` alignés sur les ADR 0003 à 0006.
- I3 : dépendances et actions ajoutées au tableau ; action tierce épinglée par SHA.
- I4 : la langue de l'interface (ADR 0006) a été implémentée avant ta réponse. C'est une
  exception assumée, car réversible ; validée ensuite (Q13, 2026-10-16).
- I5 : notices tierces livrées dans le `.vsix`.
- Mineurs :
  - CSP réduite au nonce seul ;
  - `.vscodeignore` explicite ;
  - tests renforcés (échappement, révélation, tokens, traductions) ;
  - job CI `msrv` ;
  - configuration « Extension Tests » corrigée ;
  - ADR 0005 corrigée (scripts npm, raison de `node:test`).

Écarts :
- 1.9 : contrôle visuel fait par toi avec F5 (« très bien pour l'icône », 2026-10-14). Le
  rendu dans chaque thème n'a pas été commenté en détail.
- Checklist d'accessibilité de l'état vide :
  - aucun élément interactif, donc ni focus ni clavier à traiter ;
  - le texte passe par les variables de thème VSCode ;
  - aucune animation.

  Le contraste réel dépend du thème : il sera mesuré par axe-core en Phase 4.
- `npm audit` signale 4 vulnérabilités, toutes dans mocha (via `@vscode/test-cli`) : 1 haute
  (serialize-javascript), 1 modérée, 2 basses. C'est un outil de test, absent du `.vsix`.
- L'éditeur de la Marketplace (`publisher`) est provisoire : `nicolas-a-michel` (Q12).
- Les tests de fumée des crates sans code (store, index, otlp, daemon, mcp) ne prouvent que la
  compilation et le lien avec core. Ils seront remplacés par de vrais tests dans les phases
  suivantes.

## Phase 2 — Modèle de graphe et stockage

Statut : terminée (validée le 2026-10-16)

- [x] 2.1 Écrire les ADR — critère : ADR écrites (statut « Proposé ») avant le code qui en
  dépend, puis acceptées quand tu valides la phase (procédure du skill `adr`). Sujets :
  - place des composants : consigne la réponse à Q4, option (a) ;
  - classement de fiabilité des sources pour la fusion ;
  - sémantique des snapshots `working_tree` ;
  - outil de migration (`rusqlite_migration` 2.6 ou `refinery` 0.10) ;
  - pragmas SQLite.
- [x] 2.2 `timonier-core` : les types `Node`, `NodeKind`, `SymbolKind`, `Edge`, `EdgeKind`,
  `Provenance`, la confiance, le poids et le span, sérialisables (serde) — critère :
  round-trip JSON testé pour chaque type.
- [x] 2.3 Identité stable : symbole SCIP ou `lang:qualified_name` — critère : tests unitaires
  et proptest (`parse(format(id)) == id`).
- [x] 2.4 Invariant d'arbre strict : un seul parent, pas de cycle — critère : un proptest
  génère arbres et reparentages, et toute violation est refusée par une erreur typée.
- [x] 2.5 Règles de fusion multi-sources — critère : tests en table :
  - une arête vue par TreeSitter et Lsp garde la provenance Lsp ;
  - une arête TreeSitter contredite par le Lsp est supprimée ;
  - `Intended` n'est jamais fusionnée.
- [x] 2.6 Détection des déplacements par signature et hash du corps normalisé, avec un seuil
  fixé par ADR — critère : un fichier renommé, une fonction déplacée entre deux fichiers et
  une fonction renommée à corps égal donnent « déplacé », jamais « supprimé + ajouté ».
- [x] 2.7 `timonier-store` — critère : migrations testées sur base vide ; test de lectures
  concurrentes pendant les écritures. Contenu :
  - schéma SQLite : nœuds, arêtes avec `valid_from` / `valid_to`, `snapshot(seq, commit_sha,
    timestamp, kind)`, état par fichier (hash, « vérifié par LSP au hash X ») ;
  - migrations ;
  - WAL, écrivain unique sur un thread dédié, lecteurs en lecture seule.
- [x] 2.8 Mise à jour par différence d'un fichier — critère : réappliquer un fichier identique
  modifie 0 ligne ; modifier un symbole ferme un intervalle et en ouvre un.
- [x] 2.9 Snapshots `commit`, `review_mark` et `working_tree` — critère : création, liste et
  cohérence des intervalles testées.
- [x] 2.10 Requête de diff entre deux snapshots : ajouts, suppressions, modifications,
  déplacements — critère : tests sur fixtures ; un déplacement de fichier est rapporté comme
  déplacement.
- [x] 2.11 Arêtes entre composants agrégées par requête depuis les arêtes de symboles, jamais
  stockées à la main — critère : test du poids agrégé attendu.
- [x] 2.12 Mesure de référence : insertion et diff sur un graphe synthétique — critère :
  chiffres consignés dans ce plan.
- [x] 2.13 Relecture par `phase-reviewer` — critère : verdict « prête ». Obtenu à la troisième
  relecture ; ses mineurs ont ensuite été corrigés.

Critère de sortie : tests couvrant l'insertion, la mise à jour par différence, le diff de
snapshots et la détection d'un déplacement de fichier.

Décisions :
- [ADR 0008](docs/adr/0008-hierarchie-et-identite-des-noeuds.md) : hiérarchie (Q4 a), forme
  des identifiants, positions ;
- [ADR 0009](docs/adr/0009-portees-et-fusion-des-sources.md) : portées, observations,
  fusion, arêtes pendantes, agrégation ;
- [ADR 0010](docs/adr/0010-versionnement-snapshots-et-diff.md) : tête ouverte, snapshots
  figés, diff par intervalles, déplacements ;
- [ADR 0011](docs/adr/0011-sqlite-migrations-et-ecrivain-unique.md) : rusqlite `bundled`,
  rusqlite_migration, pragmas, fil d'écriture unique.

Constats (2026-10-14, Linux, après deux relectures) :
- `cargo test --workspace` : 105 tests verts.
  - `timonier-core`, 55 tests, dont 5 proptests :
    - aller-retour des identifiants ;
    - toute construction d'arbre valide est acceptée ;
    - un reparentage sous un descendant est un cycle ;
    - sur des mutations aléatoires, le vérificateur est d'accord avec un juge naïf écrit à part.
  - Allers-retours JSON de chaque variante de diff, de `EdgeKey` et de `AggregatedEdge`.
  - `timonier-store`, 9 tests unitaires, dont un test des plans de requête sur le SQLite
    embarqué : les lectures à la tête passent par les index partiels.
  - `timonier-store`, 34 tests d'intégration. Ils couvrent :
    - le critère de sortie : insertion, mise à jour par différence (0 ligne écrite à
      l'identique, fermeture et ouverture de version après gel), diff, déplacement de fichier ;
    - les déplacements de fichier, avec nouveaux identifiants ou identifiants conservés
      (`src/foo/mod.rs` → `src/foo.rs`, dans les deux ordres du lot) ;
    - un symbole déplacé entre fichiers ;
    - un nœud retiré puis rétabli après gel ;
    - un lot où les enfants précèdent leurs parents ;
    - des diffs entre snapshots non consécutifs ;
    - la fusion, une réponse LSP tardive ignorée, et après suppression du fichier ;
    - le refus :
      - des observations précises sans hash, ou qui posent le contenu ;
      - des appels entre symboles hors LSP et SCIP ;
      - de l'intention et des arêtes de composants ;
    - le contenu des portées imposé ;
    - un nœud et une arête fournis par deux fichiers : la tête ne dépend pas de l'ordre, la
      réapplication n'écrit rien, aucun faux déplacement, puis passage de relais à l'autre
      fichier ;
    - la convergence d'un symbole déplacé ;
    - la propriété passée de main en main entre trois portées, sans attente qui reste en
      table ;
    - une attente qui change de version sans toucher la tête ;
    - le retrait d'une portée propriétaire ;
    - la version d'arête la plus fiable, qui l'emporte sur la plus petite clé ;
    - un fichier qui ne peut pas quitter la structure tant qu'une de ses contributions attend ;
    - le refus des cycles, paquets imbriqués, parents interdits, orphelins et second repo ;
    - un proptest d'équivalence entre vérifications incrémentales et vérification complète,
      avec reparentages, retraits et rétablissements ;
    - les lecteurs concurrents ;
    - le test sur modèle : à chaque snapshot figé, nœuds (tous attributs) et arêtes
      identiques ; diffs vérifiés entre toute paire de points ; réapplication sans écriture.
- `cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --check` sont verts.
- Mesures (2.12), en build release, sur Ryzen 9 5950X. Le bench donne à chaque fichier 3 appels
  fichier → fichier (tree-sitter) et 2 appels vérifiés par fonction (LSP).

  | `bench [packages] [fichiers] [symboles]` | `20 50 20` (défaut) | `50 100 40` |
  | ------------------------------------------ | ------------------: | ----------: |
  | Taille | 1 000 fichiers, 20 000 symboles, ~40 000 arêtes | 5 000 fichiers, 200 000 symboles, 400 000 arêtes |
  | Chargement initial (une transaction) | 536 ms | 4 739 ms |
  | Réappliquer un fichier identique (médiane) | 0,14 ms | 0,19 ms |
  | Changer un fichier, lignes nées dans la tête | 0,84 ms | 1,28 ms |
  | Changer un fichier, après un gel à chaque échantillon | 1,09 ms | 1,61 ms |
  | Changer 10 % des fichiers, tous distincts | 85 ms (100 fichiers) | 801 ms (500) |
  | Diff depuis la marque de revue | 22 ms (2 000 nœuds, 8 000 arêtes) | 247 ms (20 000, 80 000) |
  | Lire tous les nœuds à la tête | 16 ms | 163 ms |
  | Agréger au niveau des packages | 48 ms | 627 ms |

CI GitHub (2026-10-16) :
- commit `48e397f` (run 38860803467) : rouge. Clippy 1.99, plus récent que le 1.96 installé en
  local, refusait un `chunks_exact(2)` (lint `chunks_exact_to_as_chunks`). Un test
  d'intégration a aussi échoué sous Linux : la fenêtre de VSCode 1.142.0, sortie la veille, a
  été déclarée « unresponsive » sous xvfb avant le premier test.
- commit `72f2ae5` (run 38861207962) : les 5 jobs sont verts. Le correctif utilise
  `as_chunks::<2>()`, stable depuis 1.88 et donc compatible MSRV. Le même test d'intégration est
  passé, avec la même version de VSCode : l'échec Linux était intermittent.
- La toolchain stable locale est passée à 1.99.1, pour lancer en local le même clippy que la
  CI.

Relecture n° 1 (`phase-reviewer`) : « pas prête », avec 1 bloquant et 6 importants (B1, I1 à
I6) et 10 mineurs. Tous ont été corrigés ou consignés :
- B1 : déplacement « même identifiant », que le fichier suit ;
- I1 : `computed_on` ;
- I2 : reprise entre portées, revue ensuite (N1) ;
- I3 : granularité des `Calls` imposée ;
- I4 : vérifications incrémentales testées ;
- I5 : critère 2.1 soumis à ta validation (Q15) ;
- I6 : bench corrigé.

Relecture n° 2 : plus aucun bloquant ; deux défauts importants introduits ou laissés par les
corrections, et 8 mineurs. Tout est corrigé ou consigné :
- N1 : avec « la dernière portée l'emporte », la tête dépendait de l'historique quand deux
  fichiers fournissent le même identifiant (bascules, écritures inutiles, faux déplacements).
  Corrigé : propriété à la portée de plus petite clé, et versions des autres en attente qui
  prennent le relais (ADR 0009).
- N2 : une réponse LSP tardive portant un hash de contenu ramenait l'ancien contenu. Corrigé :
  une source précise ne peut plus poser le hash de contenu.
- Mineurs corrigés :
  - n1 : contenu des portées imposé, et arêtes observées qui partent de ce que la portée
    fournit ;
  - n2 : observations sur une portée sans contenu périmées, sources précises toujours ancrées ;
  - n3 : appels entre symboles venant de `Manifest` refusés ;
  - n4 : appariement par signature seulement s'il est unique des deux côtés ;
  - n5 : tests élargis ;
  - n6 : allers-retours JSON complets ;
  - n7 : test des plans de requête.
- Mineur consigné (n8) : les deux reports sont maintenant écrits dans les tâches 3.7 et 5.2.

Relecture n° 3 : « prête », sans bloquant ni important ; 10 mineurs. Corrigés :
- m1 : les contributions en attente comptent dans le contrôle des orphelins ;
- m2 : une portée de fichier sans contenu ne prend aucune observation ;
- m3 : une source précise n'apporte pas de nœuds ;
- m4 : pour une arête, la provenance la plus fiable l'emporte avant la plus petite clé
  (conforme à la spec « Fusion ») ;
- m5 : commentaire périmé ;
- m6 : tests à trois portées, attente qui change, retrait du propriétaire, absence de fuite ;
- m7 : une version fermée garde le propriétaire de son époque, et les rétrogradations sont
  comptées ;
- m8 : un retrait n'apporte rien d'autre ;
- m9 : allers-retours avec `computed_on` et `Freeze` ;
- m10 : skill corrigé.

Écarts :
- Le stockage ne menace pas l'objectif de 100 ms de la Phase 3 : moins de 2 ms par fichier.
- L'agrégation relit tout le graphe : 0,63 s à 200 000 symboles. La Phase 4 devra la mettre en
  cache par vue.
- Le critère de 2.1 a été reformulé ; tu as accepté le nouveau libellé (Q15).
- Limites assumées (ADR 0010) :
  - un fichier sans symbole déplacé reste « supprimé + ajouté » ;
  - un type supprimé et un autre ajouté, chacun avec une seule méthode de même nom et même
    signature, s'apparient encore.

## Phase 3 — Indexation

Statut : terminée (validée par toi le 2026-10-16, sans verdict « prête » : voir 3.12)

Réponses appliquées : Q5 (interface LSP commune, relais de l'IDE d'abord) et Q6 (heuristiques,
enrichies par l'agent via MCP).

- [x] 3.1 ADR sur le format des packs de langage : manifeste versionné, requêtes `.scm`,
  heuristiques, emplacement — critère : ADR écrite (« Proposé »), acceptée quand tu valides la
  phase (Q15) ; skill `language-pack` mis à jour.
  - [ADR 0012](docs/adr/0012-format-des-packs-de-langage.md) : packs dans
    `crates/timonier-index/packs/<langue>/`, incorporés au binaire ; contrat des captures
    vérifié au chargement.
  - Restent du code : la table des grammaires (une ligne par grammaire) et les écosystèmes
    `cargo` et `npm`.
- [x] 3.2 Packs Rust et TypeScript : grammaires (tree-sitter 0.27.1, tree-sitter-rust 0.24.2,
  tree-sitter-typescript 0.23.2), requêtes de définitions, d'imports et
  d'appels/références — critère : snapshots insta des nœuds et arêtes extraits de fixtures
  Rust et TS.
  - Trois instantanés relus ligne à ligne (Rust, TS, TSX), dans
    `crates/timonier-index/tests/snapshots/`.
  - La résolution des chemins en arêtes est testée à part
    (`crates/timonier-index/src/project.rs`), dont les réexportations Rust et TS.
- [x] 3.3 Noms qualifiés stables et hash de corps normalisé, par langage — critère : reformater
  un fichier ne change aucune identité (test).
  - Testé en Rust et en TypeScript : un fichier reformaté garde chaque nom qualifié, chaque
    signature et chaque hash de corps.
  - Réordonner les définitions ne change aucune identité (test).
  - Un déplacement de fonction entre fichiers et un renommage de fichier donnent « déplacé »
    dans le diff (test de bout en bout, `crates/timonier-daemon/tests/indexing.rs`).
- [x] 3.4 Indexation initiale parallélisée, avec un cache persistant (hash par fichier) —
  critère : une deuxième indexation sans changement ne reparse aucun fichier (compteur).
  - Cache dans SQLite (migration 0002, [ADR 0013](docs/adr/0013-extraction-resolution-et-cache.md)).
  - Seconde indexation : 0 fichier reparsé ; au redémarrage sur la même base, 0 ligne écrite
    (tests).
- [x] 3.5 Vitesse 1, à la sauvegarde — critère : latence mesurée selon H2. Contenu :
  - watcher (notify 8.2 + notify-debouncer-full 0.7), anti-rebond ~300 ms ;
  - respect de `.gitignore` (crate `ignore`) ;
  - hash du fichier, reparse tree-sitter incrémental, mise à jour par différence.

  Constat (2026-10-16, build release, Ryzen 9 5950X), sur une copie du dépôt Timonier
  (71 fichiers) et 200 sauvegardes : chiffres de 3.11 (p50 2,5 ms, p95 10,4 ms, max 24 ms) ;
  - la mesure va du début de la passe (fin de l'anti-rebond) au commit SQLite ;
  - outil : `cargo run --release -p timonier-daemon --example index_bench`.

  Le watcher réel est testé de bout en bout : sauvegarde, anti-rebond, graphe mis à jour.
- [x] 3.6 Vitesse 2, en arrière-plan, selon Q5 — critère : test avec un serveur LSP réel ou un
  double conforme. Contenu :
  - les fichiers dépendants sont marqués quand une signature publique change ;
  - file LSP basse priorité, en commençant par ce qui est affiché ;
  - arêtes fonction → fonction issues du LSP ;
  - « vérifié par LSP au hash X ».

  [ADR 0014](docs/adr/0014-affinage-lsp-en-arriere-plan.md) : interface `LanguageServer`
  (hiérarchie d'appels), file à trois priorités (affiché, ouvert, dépendant), seuls les `Calls`
  vérifiés.

  Constat : 3 tests avec un double conforme au protocole (`crates/timonier-daemon/tests/refinement.rs`) :
  - arêtes fonction → fonction en provenance `Lsp`, qui remplacent les `Calls` tree-sitter du
    fichier ;
  - « vérifié par LSP » perdu quand le contenu change, et le fichier revient en file ;
  - dépendants renvoyés en file quand une signature change ;
  - cession du travail à une sauvegarde ;
  - fichier sans serveur ignoré ;
  - affinage par le watcher quand il est libre.

  Le relais des serveurs de l'IDE est branché en Phase 4, avec le lien démon ↔ extension.
- [x] 3.7 Vitesse 3, au commit : détection du commit, re-clustering complet initialisé par la
  partition précédente, snapshot `commit` figé, sans SCIP (H4) — critère : test d'intégration
  sur un dépôt git temporaire.
  - Le snapshot doit décrire l'arbre commité, pas l'arbre de travail indexé avec ses
    modifications non commitées (ADR 0010, « Conséquences ») : indexer le commit, ou marquer le
    snapshot comme « sale ».

  [ADR 0015](docs/adr/0015-consolidation-au-commit.md) : on indexe l'arbre commité. Les
  fichiers qui diffèrent du commit prennent leur version commitée le temps du gel, puis
  l'arbre de travail revient. La détection passe par le watcher, sur le dossier git.

  Constat : 2 tests sur un dépôt git temporaire (`crates/timonier-daemon/tests/commit.rs`) :
  - le snapshot du commit contient la modification commitée, mais ni le travail en cours ni le
    fichier non suivi, que la tête garde ;
  - le retour à l'arbre de travail n'écrit aucune ligne ;
  - le watcher voit un vrai `git commit`.

  Le re-clustering à ce moment est fait avec 3.8.
- [x] 3.8 Composants — critère : stabilité mesurée (une petite modification change moins de
  X % des affectations, seuil fixé par ADR). Contenu :
  - agrégation par structure (membres du workspace Cargo, workspaces npm, dossiers) ;
  - clustering Louvain/Leiden stable. ADR : `graphrs` 0.12 (MIT) est le seul crate vérifié
    qui offre Louvain et Leiden, avec une adoption faible ; l'alternative est d'implémenter
    Leiden nous-mêmes.

  [ADR 0016](docs/adr/0016-composants-et-clustering.md) :
  - un niveau ne se découpe qu'au-delà de 12 éléments, par dossiers d'abord, puis par Leiden ;
  - Leiden écrit par nous, car graphrs ne part pas d'une partition initiale ;
  - les clés restent stables d'un clustering à l'autre ;
  - seuil de stabilité : 5 %.

  Constats :
  - Graphe synthétique de 300 nœuds : moins de 5 % des affectations changent (test).
  - Sur Timonier, après 200 sauvegardes dont 100 ajoutent un appel vers un autre fichier du
    paquet : 0 affectation changée sur 23 ; re-clustering en 4,4 ms.
  - Seul `timonier-index` dépasse 12 fichiers. On y trouve un composant `tests` et deux
    groupes : les composants (`cluster`, `components`, `structure`) et l'extraction avec la
    résolution.
  - Un redémarrage reprend les composants de la tête, sans clustering, et n'écrit rien
    (test). Un fichier laissé seul par Leiden le reste (test).
- [x] 3.9 Rôles architecturaux et dépendances externes — critère : sur Timonier, les rôles
  attendus sont vérifiés par un test (par exemple `timonier-daemon` = serveur,
  `timonier-store` = persistance). Contenu :
  - rôles par heuristiques des packs (TcpListener, routeur HTTP, ORM, types sérialisés…) ;
  - dépendances externes regroupées par usage (réseau, sérialisation, base de données,
    async) ;
  - nommage et confirmation selon Q6.

  [ADR 0017](docs/adr/0017-roles-architecturaux.md) : un attribut `role` sur les paquets et
  les composants (migration 0003), calculé à partir de trois signaux, les bibliothèques, les
  imports et le code. Les tests et exemples ne comptent pas.

  Constat (`crates/timonier-daemon/tests/timonier.rs`, sur le dépôt lui-même) :

  | Paquet | Rôle |
  | ------ | ---- |
  | cli | point d'entrée |
  | core | modèle de données |
  | daemon | tâche de fond |
  | index | utilitaire |
  | store | persistance |
  | webview | interface |
  | extension | interface |

  - Les dépendances externes sont rangées par usage (base de données, sérialisation, ui…).
  - Écart : `timonier-daemon` ne pouvait pas encore être « serveur ». Il n'avait pas de
    serveur JSON-RPC avant la Phase 4. Avec la tâche 4.2, il devient « point d'entrée », et
    le test est à jour.
  - Le nommage et la confirmation par l'agent (Q6) arrivent avec les outils MCP de la
    Phase 6.
- [x] 3.10 Compaction au-delà d'environ 50 000 arêtes de symboles — critère : test de charge
  synthétique.

  [ADR 0018](docs/adr/0018-compaction-des-aretes-de-symboles.md) : budget de 50 000 appels
  fonction → fonction issus du LSP. Au-delà, les fichiers vérifiés les plus anciens gardent
  leurs appels de fichier à fichier, toujours en provenance `Lsp` et vérifiés, jusqu'à 90 %
  du budget.

  Constats :
  - Test de comportement : compaction du plus ancien, vérification conservée, retour au
    détail quand le fichier est rouvert.
  - Test de charge, en build release : 260 fichiers de 20 fonctions, 52 000 appels vérifiés
    en 0,83 s. Il reste 46 800 appels de symbole à symbole dans la tête, et 26 fichiers sont
    compactés.
  - Commande : `cargo test --release -p timonier-daemon --test compaction -- --ignored`.
- [x] 3.11 Mesures sur le dépôt Timonier — critère : indexation à froid ≤ 3 s (H3) et mise à
  jour p95 < 100 ms (H2), chiffres et machine consignés.

  Constats (2026-10-16, après les corrections de la relecture n° 1 ; build release, Ryzen 9
  5950X, 32 fils, Linux), sur 71 fichiers Rust et TypeScript, soit 1 557 nœuds et 666 arêtes :

  | Mesure | Valeur | Cible |
  | ------ | -----: | ----: |
  | Indexation à froid du dépôt, consolidation git comprise | 456 ms | ≤ 3 s |
  | Indexation à froid d'une copie, sans git | 145 ms | |
  | Redémarrage avec le cache | 95 ms, 0 reparse, 0 ligne écrite | |
  | Sauvegarde, p50 / p95 / max (200 sauvegardes) | 2,5 / 10,4 / 24,4 ms | p95 < 100 ms |
  | Re-clustering | 4,4 ms, 0 affectation changée | |

  - La consolidation git est la plus coûteuse ici : les fichiers non commités prennent leur
    version de HEAD le temps du gel.
  - Les arêtes passent de 690 à 666 : les appels de méthode sur un receveur inconnu ne sont
    plus résolus par leur seul nom (relecture n° 1, I6).
  - Outil : `cargo run --release -p timonier-daemon --example index_bench`.
- [ ] 3.12 Relecture par `phase-reviewer` — critère : verdict « prête ». **Non atteint.**
  - Les trois relectures concluent « pas prête » ; tous leurs constats sont corrigés
    (ci-dessous).
  - Tu as validé la phase le 2026-10-16 pendant la relecture n° 3. La case restait cochée à
    tort : la relecture n° 3 l'a relevé (P1). L'écart est consigné plus bas.

  Relecture n° 1 : « pas prête », avec 1 bloquant, 7 importants et 10 mineurs. Tous sont
  corrigés, chaque scénario reproduit est gardé comme test
  (`crates/timonier-daemon/tests/robustness.rs` et les tests cités) :
  - B1 :
    - un paquet renommé ou retiré laissait un composant sous un paquet disparu, et le
      stockage refusait ensuite toute structure ;
    - corrigé : les composants sont recalculés quand les paquets changent, et seuls les
      composants valides sont émis ;
    - après une écriture refusée, tout est indexé à nouveau pour remettre la mémoire
      d'accord avec la base.
  - I1 : une réexportation qui change de cible résout à nouveau ceux qui la citent.
  - I2 : un manifeste illisible ou un chemin sans identifiant possible sont écartés et
    signalés, sans arrêter la passe ; un manifeste en cours d'édition garde les paquets.
  - I3 : la consolidation ne prend que les fichiers que le parcours indexerait (dossiers
    cachés, `.gitignore`).
  - I4 : un `lib.rs` enregistré n'entraîne plus de resynchronisation ; une resynchronisation
    n'a lieu que si les paquets changent vraiment.
  - I5 : l'état d'affinage LSP est relu de la base au démarrage (dépendants, budget). Les
    vérifications tombent quand les identifiants changent.
  - I6 : les appels sur un receveur inconnu (`x.iter()`) ne sont plus résolus par leur seul
    nom (capture `@reference.method`, version d'extracteur 2). Les composants de Timonier en
    sont plus justes.
  - I7 : un redémarrage reprend les composants de la tête au lieu de les recalculer
    (ADR 0016).
  - Mineurs corrigés :
    - m1 : un singleton de Leiden reste seul ; un nœud isolé quitte sa communauté ;
    - m2 : le bench ajoute de vraies arêtes ; chiffres mis à jour ;
    - m3 : nom de test corrigé ;
    - m4 : tests de `git.rs` (sous-dossier, worktree, dépôt sans commit), de
      `Reader::symbol_at`, `scopes` et `observations` ;
    - m5 : erreur de création du fil rendue, pas de `expect` ;
    - m6 : ADR 0012 complétée ; la clé du cache inclut la version de la grammaire, et un test
      la compare à l'espace de travail ;
    - m7 : préfixes `@tests`, `@bin`… pour les cibles Rust ; `src/` transparent sans fusion de
      dossiers ;
    - m8 : `proptest` retiré de `timonier-index` ;
    - m9 : le watcher ne surveille que les dossiers parcourus et indexe une fois les
      surveillances posées ;
    - m10 : les échecs de git sont distingués et signalés ; un fichier devenu trop gros quitte
      le graphe.
  - Trouvé en corrigeant : le commit pouvait arriver au watcher avant les sauvegardes qui
    l'ont précédé. La consolidation relit d'abord les fichiers changés depuis le dernier
    commit (ADR 0015).

  Relecture n° 2 : « pas prête ». Elle confirme B1, I1 à I6 et presque tous les mineurs
  corrigés, mais trouve 1 bloquant, 4 importants et 5 mineurs. Tous sont corrigés, et chacun
  est un test de `crates/timonier-daemon/tests/robustness.rs` :
  - N1 (bloquant, introduit par m9) : un dossier supprimé puis recréé (changement de branche)
    n'était plus surveillé. Le watcher oublie les dossiers disparus et resurveille ceux qui
    reviennent.
  - N2 : un manifeste illisible quelque part gelait tous les paquets ; seul son paquet reste
    figé.
  - N3 : `git diff` sans `--no-renames` laissait l'ancien chemin d'un renommage dans le
    snapshot de commit.
  - R1 : une tête sans composant déclenchait un clustering au redémarrage. Seule une base
    jamais indexée en déclenche un.
  - N4 : une sauvegarde dans un sous-dossier pendant l'indexation initiale était perdue. Tous
    les dossiers sont surveillés avant, et chaque dossier ajouté ensuite est relu.
  - Mineurs :
    - m-a : un paquet renommé pendant l'arrêt garde ses composants (nouveau clustering) ;
    - m-b : la version de grammaire des packs est comparée à `Cargo.lock` ;
    - m-c : la file d'affinage garde ses demandes en attente quand la base est relue ; un
      fichier vérifié sans appel n'est plus « compacté » ; l'ordre après redémarrage est
      documenté (ADR 0018) ;
    - m-d : en Rust, la résolution par nom seul reste dans la même cible ;
    - m-e : chiffres de 3.5 et mentions du redémarrage mis à jour.

  Relecture n° 3 : « pas prête ». Tous les constats de la n° 2 sont corrigés, sauf N4 dans les
  faits ; elle trouve 1 bloquant, 1 important et 1 mineur, corrigés après ta validation :
  - F1 (bloquant) : le watcher prenait les lectures de fichiers pour des modifications, y
    compris celles du démon. Il s'en nourrissait sans fin :
    - 46 passes en 8 s sur un dépôt au repos ;
    - un affinage LSP qui cédait toujours ;
    - sur 20 040 fichiers, une sauvegarde jamais vue et un arrêt de 158 s.

    Corrigé :
    - seuls les événements qui peuvent changer le graphe sont gardés (création, suppression,
      écriture, renommage) ;
    - `refresh` ne parcourt plus tous les fichiers pour chaque chemin reçu.

    Les sondes de la relecture donnent maintenant :
    - 2 passes en 8 s (l'indexation, puis la sauvegarde) et un affinage vérifié ;
    - sur 20 040 fichiers, la sauvegarde faite à 1 s est vue et l'arrêt prend 67 ms.

    N4 est donc corrigé lui aussi. Test : `the_watch_rests_when_nothing_changes`.
  - P1 : 3.12 était cochée sans verdict « prête ». Décochée, écart consigné.
  - m-f : un `refresh` sur un nouveau dossier ne trouvait pas son manifeste. Un dossier reçu
    est maintenant parcouru, et les paquets sont redécouverts quand l'ensemble des manifestes
    change. Test : `a_new_crate_is_a_package`.

  Contre-vérification de ces trois corrections par `phase-reviewer` :
  - F1 et P1 sont corrigés, N4 aussi ; aucune régression trouvée.
  - m-f ne l'était qu'en partie. Les parcours qui ne redécouvrent pas les paquets mettaient
    aussi à jour l'ensemble des manifestes : celui d'un nouveau fichier, celui du watcher et
    celui de la consolidation.
    - Effet : un crate créé de 100 à 250 ms après un autre fichier n'était jamais reconnu.
    - Corrigé : l'ensemble est celui d'où les paquets ont été découverts. Tout parcours de
      `refresh` qui en trouve un autre les redécouvre.
    - Test : `a_new_crate_seen_by_another_walk_first_is_a_package`, qui compare le résultat
      à une indexation neuve. Les sondes de la relecture reconnaissent maintenant le crate
      quel que soit le délai.
  - Hypothèse de la relecture : sous macOS, une écriture serait vue comme un simple changement
    de métadonnées. Elle est écartée : notify traduit `ITEM_MODIFIED` en `Modify(Data)`, et la
    CI macOS voit les sauvegardes.

Critère de sortie : le dépôt Timonier s'indexe en quelques secondes ; une modification de
fichier met le graphe à jour en moins de 100 ms (mesuré).

Décisions :
- [ADR 0012](docs/adr/0012-format-des-packs-de-langage.md) : format des packs de langage ;
- [ADR 0013](docs/adr/0013-extraction-resolution-et-cache.md) : extraction, noms qualifiés,
  empreinte, résolution approchée, réexportations, cache ;
- [ADR 0014](docs/adr/0014-affinage-lsp-en-arriere-plan.md) : affinage LSP, file à trois
  priorités ;
- [ADR 0015](docs/adr/0015-consolidation-au-commit.md) : snapshot de commit fidèle à l'arbre
  commité ;
- [ADR 0016](docs/adr/0016-composants-et-clustering.md) : composants, Leiden écrit par nous,
  stabilité ;
- [ADR 0017](docs/adr/0017-roles-architecturaux.md) : rôles et dépendances externes ; un
  attribut `role` sur les nœuds ;
- [ADR 0018](docs/adr/0018-compaction-des-aretes-de-symboles.md) : compaction des arêtes de
  symboles.

Écarts :
- 3.12 n'est pas atteinte : aucune relecture n'a conclu « prête ». C'est ta validation du
  2026-10-16 qui clôt la phase. Les constats de la relecture n° 3 ont été corrigés ensuite,
  sans quatrième relecture complète.
- Le relais des serveurs LSP de l'IDE et le rôle « serveur » du démon attendent la Phase 4.
  L'affinage est testé avec un double conforme, comme le critère de 3.6 le permet.
- Les composants sont ceux du dernier commit (ADR 0016). Avec beaucoup de travail non
  commité, la tête montre les composants calculés sur le commit. Un fichier nouveau rejoint
  ses voisins de dossier.
- Limites assumées :
  - les appels hors de toute fonction disparaissent quand le LSP vérifie le fichier
    (ADR 0014) ;
  - les noms qualifiés Rust sont approchés (`#[path]` ignoré, fichiers de `main.rs`) ;
  - une mise en forme qui ajoute des jetons change le hash du corps (ADR 0013).
- Packs livrés dans le binaire seulement ; les packs de l'utilisateur viendront plus tard
  (ADR 0012). Un nouvel écosystème (Python, Go) demande du code dans `timonier-index`.
- Pour le diagnostic : `cargo run --release -p timonier-daemon --example index_dump` montre
  l'arbre, les rôles et les arêtes les plus lourdes d'un dépôt.

## Phase 4 — Démon, extension et carte

Statut : terminée (validée par toi le 2026-10-18 ; 4.12 close avec réserves, Q24)

- [x] 4.1 ADR sur le protocole — critère : ADR écrite (« Proposé »), acceptée à la validation
  de la phase (Q15). [ADR 0019](docs/adr/0019-protocole-demon-extension.md). Sujets :
  - socket local, un démon par projet avec verrou, découverte ;
  - méthodes et versionnement ;
  - génération des types partagés Rust → TypeScript ;
  - jsonrpsee 0.26 ou implémentation dédiée.
- [x] 4.2 Serveur JSON-RPC 2.0 sur socket Unix : `initialize`, abonnement et désabonnement par
  vue, deltas versionnés, snapshot complet en cas de désynchronisation, arrêt — critère :
  tests d'intégration client Rust ↔ démon, dont une désynchronisation simulée qui renvoie un
  snapshot complet.
  - Messages JSON-RPC et cadrage dans `timonier-core` (`rpc`) ; serveur tokio, fil des vues et
    client bloquant dans `timonier-daemon`. Les détails sont dans l'ADR 0019.
  - Tests (`crates/timonier-daemon/tests/server.rs`, un vrai démon sur un socket temporaire) :
    - `initialize`, version refusée, statut, erreurs JSON-RPC ;
    - une vue suit le graphe par deltas, jusqu'à l'état d'un nouvel abonnement ;
    - désynchronisation simulée : le client perd un delta, voit le trou, reçoit un snapshot
      complet, puis applique de nouveau les deltas ;
    - abonnements propres à leur connexion, un démon par projet, socket périmé remplacé ;
    - arrêt sans client après le délai, arrêt demandé qui ferme les autres clients.
  - Le test des rôles sur Timonier est à jour : `timonier-daemon` devient « point d'entrée »
    (écart de 3.9 levé).
  - Mesures (build release, copie de Timonier). Le dépôt grandit, et les lectures avec lui :

    | Mesure | 2026-10-16, 81 fichiers | 2026-10-17, 135 fichiers |
    | ------ | ----------------------- | ------------------------ |
    | Lecture de la tête | 2 001 nœuds, 789 arêtes ; p50 2,9 ms | 3 047 nœuds, 1 162 arêtes ; p50 4,5 ms |
    | Vue du haut | 10 boîtes, 39 arêtes, 7 Kio de JSON ; 0,1 ms | 10 boîtes, 46 arêtes, 8 Kio ; 0,1 ms |
    | Vue avec paquets et composants ouverts | 127 boîtes, 788 arêtes, 164 Kio ; 0,4 ms | 197 boîtes, 1 161 arêtes, 242 Kio ; 0,7 ms |
    | `view/subscribe`, de bout en bout | 5 à 6 ms | 7 à 9 ms (relecture n° 5 : 9 à 10 ms) |
    | Sauvegarde → delta reçu, anti-rebond de 300 ms compris (30 sauvegardes) | p50 346 ms, p95 353 ms | p50 347 ms, p95 350 ms |

    Outil : `cargo run --release -p timonier-daemon --example view_bench`. Après l'anti-rebond
    (300 à 375 ms selon son pas), l'indexation prend environ 2 ms et les vues environ 5 ms.
- [x] 4.3 CLI `timonier daemon start|stop|status` — critère : tests assert_cmd.
  - Plus `run`, le démon au premier plan, que `start` et l'extension lancent.
  - Comportement (projet trouvé en remontant, journal, codes de sortie) : ADR 0019, « CLI ».
  - Tests (`crates/timonier-cli/tests/daemon.rs`) :
    - `status` sans démon sort avec le code 3, et `stop` sans démon n'est pas une erreur ;
    - `start`, un second `start`, `status`, le journal, `stop`, puis plus de socket ;
    - le projet est trouvé depuis un sous-dossier ;
    - `run` au premier plan : un second `run` le voit, et SIGTERM l'arrête proprement.
- [x] 4.4 Extension : lancement et supervision du démon (redémarrage avec backoff, canal de
  sortie « Timonier »), client JSON-RPC (vscode-jsonrpc 9.0), relais postMessage typé —
  critère : test d'intégration extension ↔ démon réel ; un démon tué est relancé et la vue
  se resynchronise.
  - Extension : superviseur du démon, relais par carte, commandes « Relancer le démon » et
    « Afficher le journal », réglage `timonier.daemon.path`. Détails : ADR 0019, « Extension ».
  - Webview : modèle de vue pur, qui applique snapshots et deltas et demande une resync sur
    un trou. En attendant la carte (4.7), un état pédagogique par situation du démon, en
    anglais et en français.
  - Tests d'intégration (`editors/vscode/src/test/integration/daemon.test.ts`, VSCode 1.142,
    vrai démon sur un projet temporaire) :
    - connexion au démon lancé ;
    - la carte reçoit la vue ;
    - un fichier ajouté arrive par delta ;
    - démon tué par SIGKILL : relancé après 1 s, la carte reçoit un nouveau snapshot complet.
  - Tests unitaires : délais de relance, choix du programme, ligne de `daemon run`, messages
    de la webview (node:test) ; modèle de vue et états pédagogiques (vitest).
  - `npm run test:integration` construit d'abord le démon (`cargo build -p timonier-cli`). La
    CI installe Rust dans le job de l'extension.
  - Licence de vscode-jsonrpc, empaqueté dans `extension.js` :
    `dist/THIRD-PARTY-NOTICES.md`, écrit au build d'après le metafile d'esbuild.
  - Rôles du pack TypeScript (version 1.1.0) : importer `net` ou `http` ne fait plus d'un
    paquet un point d'entrée, puisqu'un client les importe aussi. Les signaux sont dans le code
    (`createServer`, `listen`). L'extension, cliente du socket, reste « interface ». Test :
    `a_typescript_server_is_seen_in_the_code`.
- [x] 4.5 Relais du LSP de l'IDE vers le démon, si Q5 le retient — critère : définitions et
  références obtenues via l'API VSCode et reçues par le démon (test).
  - Le critère d'origine reste (Q19, réponse b, 2026-10-17) : la tâche, rouverte, est refaite
    avec l'ADR 0022, qui complète l'ADR 0014.
  - **Appels** : `lsp/outgoingCalls` vers le client qui relaie. L'affinage attend un relais sans
    perdre sa file (ADR 0019, « Relais »). Extension : `vscode.prepareCallHierarchy` puis
    `vscode.provideOutgoingCalls`, colonnes UTF-8 ↔ UTF-16, `editor/opened` pour les éditeurs
    visibles.
  - **Définitions** : `lsp/definitions`, un lot par fichier. Les `References`, `Implements` et
    `Inherits` du fichier sont vérifiés par `vscode.executeDefinitionProvider`, et celles de
    tree-sitter tombent. Sans réponse, seuls les appels sont vérifiés, comme avant.
  - **Références** : `lsp/references`. Quand une signature change, `vscode.executeReferenceProvider`
    nomme les fichiers qui utilisent le symbole : eux seuls retournent en file, au lieu de tous
    ceux qui citent son nom. Sans réponse, on retombe sur le nom. Une recherche est celle d'un
    symbole, par son id : après une autre sauvegarde, elle le retrouve à sa nouvelle place.
  - **Contenu enregistré seulement** (relecture n° 5) : l'extension refuse de répondre
    (`ContentModified`, comme en LSP) tant qu'un fichier source n'est pas celui du disque
    (Q22). Le démon garde la tâche de côté et la reprend sur `editor/saved`. Avant
    d'enregistrer, il vérifie que le disque a toujours le contenu indexé, pour les fichiers
    qu'il indexe : une réponse dans `node_modules` ne garde rien (relecture n° 6).
  - **Optimisations** (« on optimise le code à fond ») :
    - une question par chemin cité distinct, pas par site ;
    - ni ce qui est local, ni ce qui est dans une dépendance, ni la bibliothèque standard (liste
      `[lsp] outside` des packs), ni `_` ou `T` ;
    - un seul aller-retour par fichier, et huit questions en parallèle dans VSCode ;
    - des dépendants exacts, et une recherche seulement si un fichier vérifié cite le nom ;
    - un fichier vérifié entier et inchangé n'est pas redemandé quand on l'ouvre ou l'affiche
      de nouveau ;
    - rien ne change dans ce qui reste paresseux : l'affiché, l'ouvert et les dépendants
      seulement.
  - **Précision d'abord** (relecture n° 3) : une réponse douteuse ne remplace rien. Un nom que
    tree-sitter place dans un autre fichier, et que le serveur place dans celui-ci ou nulle
    part, laisse au fichier ses usages de tree-sitter. Le regroupement des chemins de même
    premier segment, essayé, est retiré : il remplaçait des arêtes justes (ADR 0022).
  - Mesures sur Timonier (2026-10-17, après la relecture n° 5, `cargo run --release -p
    timonier-daemon --example lsp_cost`). Le dépôt change, les chiffres avec lui :

    | Langage | Fichiers | Hiérarchies d'appels | Sites d'usage | Chemins cités | Questions |
    | ------- | -------- | -------------------- | ------------- | ------------- | --------- |
    | Rust | 69 | 1 012 | 5 532 | 1 764 | 787 |
    | TypeScript | 66 | 229 | 883 | 417 | 180 |

    Soit 86 % de questions de moins qu'une par site en Rust, 80 % en TypeScript, et 135
    allers-retours au plus pour les définitions de tout le dépôt.

    Une signature changée : une recherche si un autre fichier cite le nom (1 098 symboles sur
    2 012 en Rust, 510 sur 837 en TypeScript), au lieu de revérifier les fichiers qui le
    citent (8,7 et 6,2 en moyenne). Aucune pour les autres symboles.
  - Défaut trouvé par le test dans VSCode, et corrigé : pendant que le projet se charge, le
    serveur syntaxique de TypeScript répond seul, depuis le fichier demandé. Un type importé y
    est « défini » à son import. L'extension redemande alors après 1, 2 puis 4 s, et seulement
    pour un nom que tree-sitter place ailleurs.
  - Tests :
    - intégration VSCode (le projet de test est en TypeScript) : l'appel `run → helper` arrive
      en provenance LSP ; l'usage du type `Shape` par `area.ts` est vérifié ; quand `helper`
      change de signature, VSCode nomme ses utilisateurs, et `main.ts` est vérifié de nouveau ;
      `main.ts` modifié sans être enregistré, rien n'est répondu, la recherche attend, puis
      reprend au retour arrière, et l'appel enregistré reste celui du disque ; un fichier dont
      les appels vont dans `node_modules` est vérifié sans être gardé ; une note modifiée ne
      suspend rien ; un fichier en Latin-1 est vérifié ; dans une crate Rust que rien ne lit,
      tree-sitter garde ses arêtes.
      Durées lues dans le journal du démon (`RUST_LOG=timonier_daemon=debug`) : `area.ts` en
      1,0 s, la recherche des utilisateurs en 18,5 ms ;
    - intégration Rust sur socket : un client qui relaie répond aux appels, aux définitions et
      aux références, et le statut compte les recherches ; un client qui refuse
      (`ContentModified`) n'est plus interrogé, puis l'est de nouveau après `editor/saved` ;
      quand le relais change de main, ce qui était gardé ou en cours va au client suivant ;
      `view/displayed` met en file ;
    - unitaires du relais (canaux, sans socket) : un client qui n'écrit plus, ou qui cesse
      d'écrire après la question, cesse de relayer et rend la tâche aussitôt ; un lot de
      définitions a plus de temps ;
    - pipeline (`tests/refinement.rs`) : usages vérifiés et corrigés par leurs définitions (un
      `Unit` mal résolu par tree-sitter) ; une réponse douteuse garde tree-sitter ; repli sur
      les appels seuls ; dépendants nommés par le serveur, ou par nom ; au redémarrage, une
      vérification dont un fichier ou un symbole a disparu, ou dont la cible a changé, retourne
      en file ; une recherche périmée retrouve son symbole ; deux symboles à la même position
      font deux recherches ; un client aux documents non enregistrés (dès les appels ou aux
      définitions), ou un changement pas encore indexé, font attendre la tâche, fichier ou
      recherche ; une réponse dans un fichier non indexé ne garde rien ; un fichier vérifié
      entier n'est pas redemandé, un fichier vérifié en partie l'est, avant et après un
      redémarrage ; ce que l'affinage doit (recherches, dépendants) survit à un redémarrage ;
    - unitaires : le relais (les trois questions, les réponses comptées, `ContentModified`),
      les usages demandés, les positions des noms (`Tr<Q>`), la compaction qui garde les
      usages, la file (tâches gardées de côté, fichiers vérifiés entiers, recherches par
      symbole), les questions en parallèle, les colonnes, les chemins, la comparaison d'un
      document au disque, les fichiers source, et la règle du relais (refus d'emblée, après un
      changement en cours de question, ou pour un document lu qui n'est pas le disque).
- [x] 4.6 Système de design minimal, avant la première vue — critère : contraste AA vérifié
  automatiquement (axe-core) en clair, sombre et contraste élevé ; ADR sur le système de
  design. [ADR 0020](docs/adr/0020-systeme-de-design.md). Contenu :
  - tokens : couleurs sémantiques sur les variables VSCode, typographie, espacements, rayons,
    durées ;
  - composants : boîte de composant, arête TreeSitter vs arête confirmée, badge, état vide,
    info-bulle, barre d'outils ;
  - une page de démonstration en mode développement.

  Fait :
  - jetons dans `webview/src/tokens.css`, composants dans `webview/src/design/`, page
    `webview/demo.html` (développement seulement) ;
  - les variables de six thèmes de VSCode 1.142 sont capturées dans VSCode même
    (`editors/vscode/scripts/capture-themes.mjs`) : Light et Dark 2026, Light et Dark Modern,
    contraste élevé sombre et clair ;
  - `npm run test:a11y` (Playwright, Chromium, dans la CI sous Linux) : 20 tests verts.
    - axe-core, WCAG A et AA, sans violation ni contraste indécis, en anglais et en français ;
    - 3:1 mesuré pour les contours, les arêtes, les icônes et le focus ;
    - transitions à 0 s sous réduction des mouvements ;
  - le test a trouvé trois défauts, corrigés :
    - le vert « test réussi » de VSCode à 2:1 sur fond clair ;
    - une icône d'attention à 2,3:1 sur une boîte sélectionnée ;
    - un texte illisible sur une sélection en contraste élevé, où VSCode ne donne pas de fond de
      sélection ;
  - tests de composants (vitest) : noms accessibles, barre d'outils au clavier, info-bulle,
    arêtes, calcul des contrastes.
  - À ta demande (2026-10-16), « quelques couleurs pour une meilleure lisibilité » : une couleur
    calme pour quatre rôles, toujours avec l'icône et le mot (ADR 0020, « Rôles » ; le choix
    des teintes est le mien).
  - À ta demande aussi, « inspire-toi des Blueprints d'Unreal Engine ». Les boîtes ont une
    barre de titre teintée par le rôle et un corps neutre. Des broches marquent la sortie
    (à droite) et l'entrée (à gauche) des arêtes. Les cadres ouverts ressemblent aux boîtes de
    commentaire, et un quadrillage discret sert de fond (ADR 0021, « Rendu »).
  - Le test d'accessibilité mesure aussi les broches, les icônes de rôle et le survol de chaque
    élément : 36 tests verts, sur les thèmes recapturés dans VSCode 1.142.0.
- [x] 4.7 Carte (React Flow 12.12 + elkjs 0.12) — critère : sur Timonier, la vue d'ouverture
  tient en quelques boîtes nommées par rôle. Contenu :
  - vue haut niveau par rôles ;
  - zoom sémantique récursif : rôles → composants → sous-composants → fichiers → symboles ;
  - divulgation progressive au survol et au clic.

  [ADR 0021](docs/adr/0021-carte-d-architecture.md). Constat sur la vraie vue de Timonier
  (`webview/test/views/timonier`, écrite par l'exemple `view_snapshot`) :
  - 10 nœuds en haut, rangés en 6 boîtes nommées par rôle : point d'entrée (cli, daemon),
    interface (webview, extension), modèle de données (core), persistance (store),
    utilitaire (index, mcp, otlp), bibliothèques externes ;
  - le clic ou Entrée ouvre un rôle sur ses membres, puis une boîte sur son contenu, jusqu'aux
    symboles ; le survol et le focus montrent un détail ;
  - 10 tests Playwright : la vue d'ouverture, le français, le clic, Entrée, le retour aux rôles,
    et WCAG AA avec le 3:1 des lignes dans les six thèmes ;
  - tests unitaires du modèle de la carte et de la mise en page ; l'intégration VSCode passe
    avec la carte sous la CSP stricte.
- [x] 4.8 Mise en page stable, avec les positions précédentes comme contraintes d'elkjs —
  critère : après la modification d'un fichier, le déplacement moyen des nœuds inchangés
  reste sous un seuil mesuré et consigné.
  - Méthode (ADR 0021, « Stabilité ») : une mise en page incrémentale, conteneur par conteneur.
    - Ce qui n'a pas changé garde sa place exacte.
    - Un conteneur dont un enfant a grandi fait de la place par colonnes : à droite d'un élément
      plus large, en dessous d'un élément plus haut. Refermer défait ce qu'ouvrir a fait.
    - ELK ne sert qu'au premier affichage et aux conteneurs ouverts pour la première fois.
  - Seuil retenu : 22 px, un quart de la hauteur d'une boîte (88 px).
  - Mesures (2026-10-16, `webview/src/map/stability.test.ts`). Vraie vue de Timonier, tous
    rôles et boîtes ouverts, 183 éléments ; déplacement moyen des boîtes inchangées :

    | Changement | Stable | Depuis zéro | Temps |
    | ---------- | ------ | ----------- | ----- |
    | Un fichier ajouté | 4,9 px | 90,8 px | 2 ms |
    | Un appel ajouté entre deux paquets éloignés | 0,0 px | 5 511 px | 1 ms |
    | Un appel retiré | 0,0 px | 4,3 px | 1 ms |

    Premier affichage : 670 ms.
  - Correction honnête des premiers chiffres (17,6, 13,8 et 17,3 px, méthode à deux
    candidates). Ils étaient faussés par un défaut : l'indice de position recopiait aussi
    l'ancienne taille des boîtes. Le défaut est corrigé, et la méthode remplacée.
  - Test de la place faite par colonnes (`layout.test.ts`). Une fois un rôle ouvert, ce qui le
    suivait est à sa droite. Le refermer rend la mise en page de départ, au pixel près.
  - Essais écartés :
    - ELK en une passe sur toute la hiérarchie : 1 450 px, même relancé sur son propre
      résultat ;
    - placement Brandes-Köpf ou `SIMPLE` avec indices : 360 et 625 px ;
    - une mise en page incrémentale qui ne fait que descendre ce qui chevauche : un groupe
      ouvert s'élargissait par-dessus la colonne suivante, et ses arêtes repartaient en arrière
      à travers les boîtes.
  - **Boîtes déplacées à la main** (Q20, réponse b, 2026-10-17 ; ADR 0021) :
    - à la souris, ou avec Alt et une flèche, d'une case du quadrillage (16 px). Sans glisser
      (WCAG 2.5.7) : les quatre boutons du panneau de détail, ou la commande « Déplacer la
      sélection sur la carte… » ;
    - un cadre ouvert se déplace par sa barre de titre ; son corps laisse passer le pointeur,
      et glisser dessus fait défiler la carte ;
    - une boîte déplacée est épinglée. Les mises en page suivantes la laissent où elle est, et
      ce qu'elle chevauche descend sous elle ;
    - « Réinitialiser la disposition » (barre d'outils et palette) revient aux positions
      calculées ;
    - les épingles sont gardées dans l'état de l'espace de travail de VSCode (H13), et rendues à
      chaque nouvelle page ;
    - tests : la mise en page (une boîte épinglée reste, ce qu'elle couvre descend, une épingle
      d'un autre conteneur est ignorée), Playwright (glisser, Alt et flèche, boutons du panneau,
      corps d'un cadre qui fait défiler, réinitialiser), le relais (les épingles rendues à la
      page, gardées), la validation du message.
- [x] 4.9 Navigation — critère : test d'intégration de l'ouverture du code ; checklist
  d'accessibilité du skill. Contenu :
  - clic vers le code, ouvert au bon span ;
  - navigation au clavier ;
  - une commande de palette par action ;
  - respect de la réduction des mouvements.

  Fait :
  - **Code** : Maj+Entrée, double-clic, ou le bouton « Aller au code » du panneau de détail.
    Entrée suffit sur ce qui ne s'ouvre pas. L'éditeur s'ouvre à côté de la carte, le curseur
    au début du span, les colonnes UTF-8 converties en UTF-16.
  - **Clavier** :
    - Tab entre les nœuds ;
    - les flèches vers la boîte la plus proche dans leur direction ;
    - Début vers la première ;
    - Entrée ouvre ou ferme.
  - **Palette** : « Ajuster la carte à la fenêtre », « Zoom avant » et « Zoom arrière sur la
    carte », « Revenir aux rôles », « Ouvrir ou fermer la sélection », « Aller au code de la
    sélection », en plus d'« Ouvrir la carte ». Les touches + et − zooment aussi, et la barre
    d'outils a les deux boutons (relecture m23).
  - **Échap** range le panneau de détail (WCAG 1.4.13, relecture m15).
  - Test d'intégration (VSCode 1.142, vrai démon) : la palette déplie le paquet puis le fichier ;
    « Aller au code » ouvre `src/main.ts` à côté de la carte, le curseur sur la ligne de
    `run`.
  - Playwright : flèches, Début, Maj+Entrée, double-clic, bouton du panneau ; nom accessible
    qui dit ce que fait Entrée.
  - Deux défauts trouvés par ces tests, corrigés :
    - une commande envoyée pendant que la webview se rechargeait était perdue. Elle attend
      maintenant la page, puis la carte ;
    - le panneau de détail restait sur un élément que le pointeur avait quitté. Il montre
      désormais le dernier élément survolé ou focalisé.
  - Checklist d'accessibilité du skill :
    - [x] contraste 4,5:1 (texte) et 3:1 (lignes, icônes, focus), mesuré dans les six thèmes ;
    - [x] chaque action au clavier et dans la palette ;
    - [x] focus visible partout ; nœuds et arêtes nommés pour les lecteurs d'écran ;
    - [x] jamais la couleur seule : tirets pour les arêtes approchées, mots et formes pour les
      rôles et les badges ;
    - [x] réduction des mouvements : transitions à 0 s, cadrage sans animation ;
    - [x] axe-core dans les tests (`npm run test:a11y`).
- [x] 4.10 Rendu par niveau de détail, avec agrégation au-delà d'un seuil de nœuds — critère :
  images par seconde mesurées et consignées sur un graphe synthétique de plusieurs milliers
  de nœuds.
  - **Agrégation au-delà de 500 éléments** : la carte dessine des niveaux entiers, du haut vers
    le bas, tant qu'ils tiennent. Plus bas, une boîte est repliée, avec un badge « N repliés ».
    Un clic ou Entrée met son contenu en premier et le cadre ; les autres restent repliées.
  - **Au-delà de 600 arêtes**, celles des éléments les plus profonds remontent à leurs
    conteneurs, d'un niveau à la fois.
  - **Au zoom éloigné (sous 0,5)**, les boîtes ne montrent que leur nom.
  - Tout ce qui est dessiné reste dans le DOM, donc atteignable au clavier et par les lecteurs
    d'écran. Ne rendre que la partie visible gagnait 6 images par seconde, mais rendait le reste
    inaccessible : écarté.
  - Mesures (2026-10-16, `npm run bench:map -w webview`) :
    - environnement : Chromium 156 sans tête (Playwright 1.64.1), Ryzen 9 5950X ;
    - dépôt synthétique `webview/src/demo/synthetic.ts` : paquets × 12 composants × 12
      fichiers, environ deux appels par fichier, tout ouvert ;
    - « défilement » : glisser sur la carte pendant 3 s ; « zoom » : la molette pendant 3 s.

    | Dépôt | Nœuds de la vue | Dessinés (nœuds, arêtes) | Premier affichage | Défilement | Zoom |
    | ----- | --------------- | ------------------------ | ----------------- | ---------- | ---- |
    | 20 × 12 × 12, avant | 3 146 | 3 146, 5 292 | 12,0 s | 5,0 i/s | 59 i/s |
    | 20 × 12 × 12, après | 3 146 | 266, 194 | 1,7 s | 54 i/s | 60 i/s |
    | 40 × 12 × 12, avant | 6 286 | 6 286, 10 602 | 23,3 s | 1,2 i/s | 59 i/s |
    | 40 × 12 × 12, après | 6 286 | 46, 478 | 1,4 s | 60 i/s | 60 i/s |
    | 20 × 12 × 12, rendu Blueprints | 3 146 | 266, 194 | 1,7 s | 51 à 54 i/s | 60 i/s |
    | 40 × 12 × 12, rendu Blueprints | 6 286 | 46, 478 | 1,4 s | 60 i/s | 60 i/s |

    Correction (2026-10-17). La relecture n° 2 mesurait 40 à 42 i/s, et moi 45. J'en avais
    accusé le rendu plus riche, à tort :
    - le banc saisissait la carte en son centre. Depuis la mise en page qui fait de la place
      par colonnes, ce centre tombe sur une boîte, et le défilement passait par elle ;
    - depuis que les boîtes se déplacent (Q20), saisir une boîte la déplace. Le banc saisit
      donc le fond, comme une personne qui fait défiler : de 50,6 à 54,2 i/s selon les passages
      du 2026-10-17 (54,1 et 54,2 le matin, 50,6 à 51,9 l'après-midi, machine peu chargée),
      autour des 54 d'avant le nouveau rendu ;
    - la correction du survol reste : seules les arêtes de l'élément pointé changent.
  - Deux lenteurs quadratiques de la mise en page sont corrigées en passant. Seule, elle prend
    maintenant 1,2 s pour 266 éléments, en trois passes posées.
  - Conclusion sur ELK dans le fil de la webview (relecture m16). La page ne répond pas pendant
    le premier affichage et la première ouverture d'un grand conteneur. Un changement du code,
    lui, ne coûte que 1 à 2 ms : il ne passe plus par ELK (4.8). C'est acceptable pour la
    Phase 4, et le worker reste écarté. Il sera repris si un premier affichage dépasse 2 s sur
    un vrai dépôt (ADR 0021).
  - Test Playwright : une grande carte replie ses fichiers ; Entrée sur un composant replié
    montre ses 12 fichiers, et les autres restent repliés. Tests unitaires des budgets d'éléments
    et d'arêtes.
- [x] 4.11 Test des cinq secondes avec toi — critère : réussi et consigné.
  - Écran : la carte d'ouverture de Timonier, dans VSCode 1.142, en français, thème Dark 2026 :
    six boîtes nommées par rôle et leurs arêtes.
  - Date : 2026-10-16. Testeur : toi.
  - Réponses, après cinq secondes :
    - ce que montre l'écran : « l'architecture très haut niveau, avec les liens de partage des
      données entre les boîtes » ;
    - ce que tu ferais ensuite : « cliquer sur une boîte pour voir le détail d'implémentation
      d'une partie, en cas de besoin ».
  - Résultat : **réussi**. La question principale de l'écran (de quoi est fait ce projet, et où
    aller voir) a la bonne réponse, et l'action suivante est celle que l'écran propose.
  - Nuance : tu as lu les arêtes comme des partages de données. Ce sont des dépendances et des
    appels entre les parties ; la carte ne le dit nulle part.
  - Correction faite : une légende des arêtes, en haut à droite de la carte. Elle dit « Liens :
    dépendances et appels », distingue sûr et approché, et montre le sens (sortie à droite,
    entrée à gauche).
  - Refait (relecture n° 2, I-a) : le premier test portait sur le rendu d'avant les couleurs et
    les Blueprints.
  - Test n° 2 : la carte d'ouverture de Timonier dans VSCode 1.142, en français, thème Dark
    2026, avec le nouveau rendu. Date : 2026-10-17. Testeur : toi. Réserve : tu avais vu des
    captures de ce rendu la veille.
  - Réponses :
    - ce que montre l'écran : « les différents modules et leur lien de dépendance » ;
    - ce que tu ferais ensuite : « soit prompter pour la suite, soit cliquer sur une boîte pour
      voir le contenu, ou les déplacer pour mieux voir les liens ».
  - Résultat : **réussi**. Les liens sont lus comme des dépendances, ce qu'ils sont, et
    l'action suivante est celle que l'écran propose. Le souhait de déplacer les boîtes devient
    la réponse (b) à Q20.
- [x] 4.12 Relecture par `phase-reviewer` — critère : verdict « prête ». **Atteint avec
  réserves** : la relecture n° 18 rend « prête avec réserves », sans rien qui bloque ; tu as
  décidé de clore ainsi le 2026-10-18 (Q24).
  - Relecture n° 1 (2026-10-16) : « pas prête », aucun bloquant, 5 importants, 21 mineurs.
  - Importants :
    - I1 : un client relais qui ne lisait plus bloquait le pipeline. Corrigé : la question part
      sans attendre, et l'attente cède à tout autre travail. Test
      `a_relaying_client_that_stops_reading_holds_nothing` ;
    - I2 : un delta perdu en fin de rafale laissait le client désynchronisé sans qu'il le sache.
      Corrigé : quand sa file se vide, le client reçoit un delta vide qui lui montre le trou.
      Test `a_client_whose_queue_overflowed_learns_of_the_gap` ;
    - I3 : `view/displayed` n'était jamais envoyé. Corrigé : la carte dit quels fichiers elle
      dessine, et le relais les transmet, et les redit à chaque nouvelle connexion (test
      unitaire du relais) ;
    - I4 : le zoom sémantique ne se fait qu'au clic, alors que la spec dit « cliquer ou
      zoomer ». Posé en Q16 ; tu gardes le clic pour l'instant ;
    - I5 : le relais de vue laissait fuir des abonnements. Corrigé : ses opérations passent une
      à une. Premiers tests unitaires du relais (six), sans VSCode.
  - Mineurs corrigés :
    - m1 : démon lancé détaché. En le vérifiant, j'ai trouvé un vrai défaut : il mourait avec la
      fenêtre, car `tracing-subscriber` paniquait sur la sortie d'erreur fermée. Corrigé, avec
      le test « le parent parti, le démon continue » ;
    - m3 : le relais précédent reprend quand le dernier part ;
    - m4 : sans `XDG_RUNTIME_DIR`, le dossier d'exécution est sous `~/.cache`, plus sous `/tmp`,
      où un autre utilisateur pouvait le créer d'avance. L'ordre des contrôles reste : le
      verrou est créé avant la vérification du dossier, dont l'UID se compare à celui du
      verrou. Sans `unsafe`, la bibliothèque standard ne donne pas l'UID du processus, et le
      mode 0700 exigé rend l'ordre sans risque ;
    - m5 : messages de la webview vérifiés champ par champ, et « Aller au code » limité au
      dossier du projet ;
    - m6 : une connexion se ferme même si le fil des vues est mort ;
    - m7 : ce PLAN.md (Décisions, Écarts) ;
    - m8 : thèmes recapturés dans VSCode 1.142.0, le plus récent ;
    - m9 : seuil de 22 px, et méthode de stabilité refaite (4.8) ;
    - m10 : dépendances transitives consignées, et notice EPL avec le lien de la version
      exacte ;
    - m11 : H9 et Journal (README en anglais), page Marketplace et texte d'accueil à jour ;
    - m13 : une seule épaisseur d'arête ;
    - m14 : le survol ne touche plus au fond, les barres de titre des cadres sont opaques, et
      les arêtes restent sous toutes les boîtes. En mode « basic », React Flow les élevait au
      niveau des nœuds enfants, et elles barraient le texte. Le survol est mesuré ;
    - m15, m16, m17 (vues de Timonier régénérées), m19 (une resync ratée se réabonne), m23.
  - Mineurs consignés en écarts : m12 (densité), m20 (tests unitaires de l'extension).
  - Relecture n° 2 (2026-10-16), contre-vérification : « pas prête », aucun bloquant, 4
    importants, 17 mineurs. Les corrections de la n° 1 sont vérifiées.
  - Importants :
    - I-a : le test des cinq secondes porte sur l'ancien rendu. À refaire avec toi (4.11) ;
    - I-b : la légende disait « confirmée par le serveur de langage » pour tout trait plein.
      Or les arêtes déclarées dans un manifeste sont pleines aussi, et ce sont les seules de
      la carte d'ouverture de Timonier. Corrigé : « Sûre : déclarée dans un manifeste, ou
      confirmée par un serveur de langage », dans la légende, les noms accessibles et les deux
      README ;
    - I-c : la persistance avait la couleur d'attention (`charts.yellow`, identique à
      `list.warningForeground` dans plusieurs thèmes). Corrigé : la persistance passe en cyan,
      et la tâche de fond devient neutre. Ta demande se limitait à « quelques couleurs » : le
      choix des teintes est le mien, et l'ADR 0020 le dit maintenant ;
    - I-d : le défilement de 4.10 était tombé à 40-42 i/s. Le banc saisissait une boîte : il
      saisit maintenant le fond, et mesure 51 à 54 i/s, autour d'avant (voir 4.10). Le survol,
      qui redessinait toutes les arêtes, est corrigé aussi.
  - Mineurs corrigés :
    - m-a, m-b : l'ADR 0019 décrit le delta vide qui signale une perte, et l'attente du relais
      qui cède ;
    - m-c : le rôle est écrit dans le cadre d'une boîte ouverte, et l'ADR 0020 dit où il l'est ;
    - m-d : les icônes de rôle, les broches sur leur barre de titre, le triangle de la légende
      et le survol de chaque élément sont mesurés ;
    - m-e : les fils survolés ont un liseré du fond, pas seulement une couleur ;
    - m-f : tests de `view/displayed` côté démon (`files_shown_on_the_map_wait_for_refinement`)
      et côté webview (`MapView.test.ts`) ;
    - m-g : tests Playwright des touches + et −, des boutons de zoom et d'Échap ;
    - m-h : le relais ignore un `rendered` d'une révision qu'il n'a pas transmise ;
    - m-i : les deux fenêtres de course du signal de perte sont fermées (drapeau posé avant un
      second essai ; signal attendu, plus jamais perdu) ;
    - m-j : un nouvel élément va où mènent ses arêtes (test), et l'ADR 0021 dit ce qui n'est pas
      garanti ;
    - m-k : posé en question (Q20) ;
    - m-l : description commune des nœuds neutre ;
    - m-m : `timonier.daemon.path` en portée `machine`, et pas d'activation dans un dossier non
      approuvé (tests du manifeste) ;
    - m-o : daltonisme simulé, consigné (ADR 0020, et Écarts) ;
    - m-p : les tirets ne veulent plus dire qu'une arête approchée ;
    - m-q : commentaires des tests.
  - Restaient m-n (Q17) et I-a (le test des cinq secondes) : faits le 2026-10-17.
  - Relecture n° 3 (2026-10-17) : « pas prête », aucun bloquant, 4 importants, 9 mineurs. Le
    critère de 4.5 est atteint tel qu'il est écrit ; les défauts portent sur l'ADR 0022.
  - Importants, corrigés, chacun avec un test qui échoue sans la correction :
    - I-1 : le regroupement des chemins remplaçait des arêtes justes par des arêtes fausses
      « sûres », dès qu'une réexportation par `*` passait entre eux. Retiré : une question par
      chemin distinct ;
    - I-2 : la position demandée tombait dans les arguments d'un trait générique (`Tr<Q>`).
      Elle est maintenant cherchée dans le texte du nœud ;
    - I-3 : au redémarrage, les usages vérifiés n'étaient jamais jugés périmés. Une extrémité
      disparue (fichier supprimé, bibliothèque renommée) renvoie maintenant le fichier en file ;
    - I-4 : les recherches de dépendants coûtaient sans mesure, jusqu'à quatre requêtes et 7 s
      chacune. Elles ne se font plus que si un fichier vérifié cite le nom, sans nouvelle
      tentative, et leur coût est mesuré (`lsp_cost`). Les nouvelles tentatives des définitions
      ne portent plus que sur un nom que tree-sitter place ailleurs. En le faisant, j'ai trouvé
      qu'une règle de « serveur prêt » par langage n'était pas sûre : écartée.
  - Mineurs corrigés : m-1 (chiffres), m-3 (boutons du panneau), m-4 (cadres déplacés par leur
    barre de titre), m-5 (Alt et flèche annoncés), m-6 (tests : `main.ts` revérifié, socket pour
    les trois méthodes), m-7 (le saut vers la déclaration, dans l'ADR), m-8 (commentaire), m-9
    (tailles mesurées gardées : plus d'avertissement 015).
  - Question posée : m-2, devenu Q21.
  - Relecture n° 4 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 7 mineurs. Les corrections de la n° 3 sont vérifiées, par ses sondes.
  - Important, corrigé avec un test qui échoue sans la correction :
    - I-A : une recherche de dépendants, devenue périmée par une autre sauvegarde, était
      abandonnée, et les dépendants perdus. Elle retombe maintenant sur le marquage par nom ; la
      même recherche demandée de nouveau remplace celle qui attend, avec le contenu récent et les
      fichiers des deux.
  - Mineurs corrigés :
    - m-A : le doute ne porte plus que sur un nom lié par un import ou un chemin ; un nom deviné
      par tree-sitter se laisse corriger. Seules les questions douteuses sont redemandées ;
    - m-B : au redémarrage, une arête vers un fichier changé pendant l'absence ne tient plus ;
    - m-C : tests de l'appel compacté au redémarrage, du filtre des recherches et du repli ;
    - m-D : les questions abandonnées sont annulées (`$/cancelRequest`) ;
    - m-E : le défilement de 4.10 consigné avec sa variation (51 à 54 i/s) ;
    - m-F : les commandes de mesure dans CLAUDE.md ;
    - m-G : le statut du démon compte les recherches nommées par le serveur ou faites par nom ;
      le test VSCode exige qu'elle soit nommée par VSCode.
  - Relecture n° 5 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 2
    importants, 4 mineurs. Les corrections de la n° 4 sont vérifiées, par ses sondes.
  - Importants, corrigés, chacun avec un test qui échoue sans la correction :
    - I-α : un fichier affiné pendant que son éditeur avait des modifications non enregistrées
      recevait les appels du tampon, enregistrés comme vérifiés au hash du disque (reproduit
      dans VSCode). L'extension ne répond plus que sur le contenu enregistré, et le démon garde
      la tâche de côté jusqu'à `editor/saved`. En le corrigeant, j'ai fermé le cas voisin : un
      changement sur le disque que le démon n'a pas encore indexé, alors que VSCode l'a déjà lu.
      Tests : `unsaved_documents_hold_the_work_until_saved`,
      `answers_about_a_change_not_indexed_yet_wait_for_it`, le test sur socket et le test
      VSCode ;
    - I-β : deux recherches de dépendants à la même position, sur deux contenus, étaient
      fusionnées, même pour deux symboles différents : les dépendants du premier se perdaient.
      Une recherche porte maintenant l'id de son symbole. Périmée, elle le retrouve à sa
      nouvelle place au lieu de retomber sur le nom. Test
      `lookups_of_two_symbols_at_one_position_stay_apart`.
  - Mineurs corrigés :
    - m-α : au redémarrage, un appel en détail vers un fichier changé pendant l'absence ne
      tient plus, comme un appel compacté (une signature a pu changer). Le statut de l'ADR 0014,
      acceptée, renvoie à l'ADR 0022 pour cette règle ;
    - m-β : un fichier vérifié entier et inchangé n'est plus affiné de nouveau à chaque
      ouverture ou affichage. Le sont encore un fichier aux usages douteux, compacté, dépendant
      ou changé ;
    - m-γ : chiffres de 4.2 et de 4.5 mesurés de nouveau et datés, ADR 0022 et README à jour ;
    - m-δ : commentaire du compte `byName`, test Rust de `lookups` dans le statut,
      `lsp_cost.rs` dans la carte du dépôt de CLAUDE.md.
  - Relecture n° 6 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 4 mineurs. I-α, I-β, m-α, m-β, m-γ et m-δ sont vérifiés, par ses sondes et des
    mutations.
  - Important, corrigé avec un test qui échoue sans la correction :
    - I6-1 : une réponse qui pointait dans un fichier que le démon n'indexe pas (`node_modules`
      sous la racine) gardait la tâche pour toujours, refaite en entier à chaque libération.
      Confirmé dans VSCode ; 48 des 66 fichiers TypeScript de Timonier auraient été touchés. Le
      contrôle au disque ne porte plus que sur les fichiers indexés. Tests
      `answers_into_files_not_indexed_hold_nothing` et VSCode (une dépendance dans
      `node_modules` du projet de test). Dans la même famille, l'extension compare un document
      au disque dans son encodage : un fichier qui n'est pas en UTF-8 ne la fait plus refuser à
      chaque fois.
  - Mineurs :
    - m6-1 : tests ajoutés pour les mutations qui survivaient (recherche face à un changement
      pas encore indexé, fichier vérifié en partie redemandé avant et après un redémarrage,
      relais qui part, définitions refusées, et les trois contrôles de l'extension, devenus une
      fonction pure testée sans VSCode) ;
    - m6-2 : seuls les fichiers source, ceux que lisent les packs, suspendent les réponses. Le
      démon en donne les extensions à `initialize`. Posé en Q22 : la pause elle-même se voit ;
    - m6-3 : une question perdue quand le relais change de main rend sa tâche à la file, au
      lieu de la faire sortir sans vérification (défaut d'avant la relecture n° 5) ;
    - m6-4 : l'ADR 0022 dit que les références ne lisent que le fichier demandé, comme le code.
  - Relecture n° 7 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 4 mineurs. I6-1 est fermé dans VSCode et dans sa famille (cible trop grande,
    ignorée par git, fichier en Latin-1) ; m6-1, m6-2 et m6-4 sont vérifiés.
  - Important, corrigé avec un test qui échoue sans la correction :
    - I7-1 : un client relais dont la connexion n'écrivait plus, sans être parti, faisait
      tourner le démon à 100 % d'un cœur : ma correction m6-3 rendait la tâche à la file, qui
      la lui renvoyait aussitôt. Un tel client cesse maintenant de relayer. Test
      `a_client_that_writes_no_more_relays_no_longer`.
  - Mineurs :
    - m7-1 : tests ajoutés pour les mutations qui survivaient : le filtre des définitions
      (N2), une question dont le client part (N12), un fichier compacté rechargé (M5e), le
      suivi des fichiers source, devenu une classe pure (E7, E14), l'encodage (E8, un fichier
      Latin-1 dans le projet de test VSCode), l'absence de serveur (E11, E16, une crate Rust
      sans rust-analyzer). Pour ces derniers, le statut compte maintenant les questions sans
      réponse (`lspAnswers.failed`). Le test VSCode d'I6-1 exige un appel vérifié ;
    - m7-2 : le cache des comparaisons au disque se vide à la fermeture d'un document ;
    - m7-3 : une réponse à `initialize` sans extensions laisse compter tous les fichiers ;
    - m7-4 : l'ADR 0022 dit que, sans relais, une recherche attend le client suivant.
  - En plus, pour l'hypothèse de la relecture d'un index durablement en retard sur le disque :
    le démon relit lui-même un fichier trouvé différent, sans attendre son observateur. Test
    `the_watch_reads_a_file_found_stale_at_once`.
  - Trouvé en relançant les contrôles : le test d'accessibilité des lignes de la carte échouait
    une fois sur quatre. Il mesurait une couleur en pleine transition (lue `oklab(…)`), juste
    après un clic, et parfois avant qu'un survol ne lance la sienne. Il attend maintenant une
    image, puis la fin de toute animation : 8 passages sur 8.
  - Relecture n° 8 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 6 mineurs. I7-1 est fermé (la sonde du client à moitié fermé passe de 3 000 à
    10 ms de CPU par 3 s) ; les mutations de la n° 7 sont tuées.
  - Important, corrigé avec un test qui échoue sans la correction :
    - I8-1 : la relecture immédiate tournait sans fin sur un fichier illisible : relu, il ne
      changeait pas, mais le rafraîchissement libérait tout, et la tâche repartait (31 000
      questions en 2 s ; 500 par seconde au serveur TypeScript dans VSCode). Un rafraîchissement
      qui ne change rien ne libère plus rien ; après la relecture, la tâche n'est libérée que si
      le fichier a changé ou correspond de nouveau à l'index. Tests
      `a_file_out_of_reach_holds_without_asking_again`,
      `the_watch_does_not_loop_on_a_file_out_of_reach` et
      `a_change_undone_before_it_is_read_releases_what_it_held`. Une tâche gardée dit
      maintenant pourquoi (documents non enregistrés, ou disque différent de l'index).
  - Mineurs :
    - m8-1 : une connexion qui cesse d'écrire après le départ d'une question ne la fait plus
      attendre 10 s puis sauter : l'attente le voit, et rend la tâche aussitôt ;
    - m8-2 : le pipeline voit lui-même que le client qui relaie a changé, et redemande ce que le
      précédent avait refusé, même quand aucun réveil ne vient (double détachement) ;
    - m8-3 : tests ajoutés pour les mutations qui survivaient : l'événement de vue après la
      relecture (R4), la relecture vidée (U1), le délai des lots de définitions (P1),
      l'annulation des nouvelles tentatives, devenues une fonction pure testée (X5), et les
      colonnes UTF-8 d'un nom après des accents (X6, test VSCode). Le cache des comparaisons
      est maintenant par document, ce qui rend son nettoyage inutile (M72). X3 survit, mais il
      est équivalent : VSCode annule les modifications d'un document avant de le fermer, et
      c'est cette annulation qui libère ; le test VSCode le montre de bout en bout. Le test
      « client qui n'écrit plus » est rangé parmi les tests unitaires, ce qu'il est ;
    - m8-4 : le test du verrou attend qu'il se libère : un processus qu'un autre test lance le
      tient jusqu'à ce qu'il exécute son programme ;
    - m8-5 : clippy 1.96 propre (`is_none_or`) ;
    - m8-6 : les tests d'intégration effacent leur projet temporaire en fin de passage ; les 64
      dossiers laissés dans `/tmp` sont effacés.
  - Relecture n° 9 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 2
    importants, 4 mineurs. I8-1, m8-1, m8-2, m8-4 et m8-5 sont vérifiés ; X3 est confirmé
    équivalent dans le code de VSCode.
  - Importants, corrigés avec un test qui échoue sans la correction :
    - I9-1 : ma correction m8-6 effaçait le projet de test dès que js-debug listait la
      configuration, avant d'ouvrir VSCode : « Extension Tests » (F5) s'ouvrait sur un dossier
      effacé. Rien n'est effacé quand le fichier n'est que listé ; vérifié avec
      `--list-configuration` ;
    - I9-2 (défaut plus ancien) : un démon arrêté oubliait les recherches de dépendants et les
      dépendants pas encore revérifiés ; leur vérification d'avant passait pour sûre, même à la
      réouverture. La base garde maintenant ce que l'affinage doit (migration 0004), et le
      démon le reprend au démarrage. Test `work_owed_survives_a_restart` (recherche en
      attente, gardée, dépendant en attente) et test de migration depuis le schéma 3.
  - Mineurs :
    - m9-1 : un fichier demandé ou vérifié n'est plus gardé ; une recherche du même symbole
      emporte celle qui était gardée ;
    - m9-2 : tests ajoutés pour la raison d'une tâche gardée (A15), une cible supprimée avant
      d'être vue, et la conversion des colonnes dans le fichier interrogé (X6b, test VSCode).
      Une cible qui n'est plus indexée ne compte plus comme différente du disque, ce qui rend
      A2 équivalente. X5c reste sans test : produire dans VSCode une annulation pendant une
      nouvelle tentative n'est pas déterministe ; la règle elle-même est testée (`askAgain`) ;
    - m9-3 : le test 4.9 cherche l'éditeur parmi les éditeurs visibles, sans dépendre du focus
      de la fenêtre ;
    - m9-4 : textes de PLAN.md et de l'ADR 0022 corrigés (ce qui libère un fichier illisible),
      lignes trop longues de l'ADR 0022 coupées, l'ADR 0021 renvoie à Q21.
  - Relecture n° 10 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 5 mineurs. I9-2 et m9-1 tiennent, par les sondes de la n° 9 et de nouvelles
    (changement de paquets, commits, fichiers renommés pendant l'arrêt, base du schéma 3,
    réindexation de secours).
  - Important, corrigé :
    - I10-1 : « Extension Tests » (F5) restait rouge (9 tests sur 19) et laissait son projet.
      js-debug ne passe à VSCode que les arguments de la configuration listée : ni
      `--disable-workspace-trust`, que la ligne de commande ajoute seule, ni le nettoyage. Le
      drapeau est maintenant dans `launchArgs`, et les tests effacent eux-mêmes leur projet à la
      fin (`cleanup.test.ts`). Vérifié en relançant VSCode comme js-debug, avec ces seuls
      arguments : 19 réussis, rien ne reste.
  - Mineurs :
    - m10-1 : la dette s'écrit avant le lot d'un rafraîchissement, et au démarrage les remises
      à zéro partent avec le lot ; une vérification dont le fichier a changé pendant l'arrêt
      n'est plus reprise. Test `a_file_changed_while_away_is_asked_again`, qui manquait ;
    - m10-2 : seul ce qui change dans la dette est écrit. Un pas, sur une dette de 55 000
      lignes : 2,3 ms au lieu de 106 (ADR 0022) ;
    - m10-3 : un échec d'écriture de la dette est noté sans faire échouer le pas ;
    - m10-4 : le filtre redondant de `refine` est retiré, celui du contrôle au disque reste et
      est testé ; le test de fusion d'une recherche gardée vérifie les fichiers cités des deux
      (D16). X5b reste : une pause de plus avant d'abandonner, sans effet sur le résultat ;
    - m10-5 : le retour à l'arbre de travail après un commit ne cherchait plus de dépendants.
      Retiré à la relecture n° 11 (I11-1) : il en faisait manquer de vrais.
  - Relecture n° 11 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 6 mineurs. I10-1 est fermé (F5 rejoué avec les arguments exacts de js-debug,
    deux passages en parallèle compris) ; m10-1, m10-2 et m10-4 tiennent.
  - Important, corrigé :
    - I11-1 : ma correction m10-5 faisait manquer des dépendants. Le retour à l'arbre de
      travail après un commit relit le disque : une signature changée que le démon n'avait pas
      encore lue y était lue, et ses dépendants jetés (0 cas suivi sur 6 dans la sonde de la
      relecture, dès 50 ms entre le commit et l'écriture). m10-5 est retirée : ce n'était
      qu'une optimisation. Tests `a_signature_change_read_when_a_commit_comes_back_is_followed`
      et, pour le rattrapage du commit, `a_signature_change_committed_before_it_is_read_is_followed`.
  - Mineurs :
    - m11-1 : un lot et sa dette s'écrivent dans la même transaction (`apply_all_owing`) : un
      échec d'écriture de la dette ne laisse plus le lot enregistré sans elle. Test
      `a_debt_that_cannot_be_written_holds_back_its_batch` (un déclencheur SQLite refuse la
      dette) ;
    - m11-2 : tests de la recherche seulement déplacée (S6) et du rattrapage (N9) ; ceux de
      l'échec d'écriture après un pas (N4, N5) sont venus à la relecture n° 12 (m12-3) ;
    - m11-3 : la configuration des tests efface les projets de plus d'un jour laissés par un
      listage seul ou un passage interrompu ;
    - m11-4 : F5 « Extension Tests » compile aussi le démon (`.vscode/tasks.json`) ;
    - m11-5 : le banc de la dette est dans le dépôt (`examples/debt_bench.rs`, CLAUDE.md), le
      commentaire de la migration 0004 dit qu'elle s'écrit par différence ;
    - m11-6 : le test du verrou réessaie aussi de le reprendre.
  - Relecture n° 12 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 5 mineurs. I11-1 est fermé de bout en bout (24 cas suivis sur 24, avec le vrai
    watch) ; m11-1 tient sur ses chemins nominaux.
  - Important, corrigé avec un test qui échoue sans la correction :
    - I12-1 : ma correction d'I11-1 suivait toutes les signatures au retour de l'arbre de
      travail après un commit : des centaines de recherches par commit sur ce dépôt, et non
      « quelques questions » comme je l'avais écrit. Seul un fichier dont le contenu relu diffère
      de ce que l'index tenait avant le gel est suivi, et un fichier qui revient tel quel
      retrouve sa vérification LSP, que le gel faisait tomber : 0 question par commit au lieu de
      221 à 678 recherches et 131 à 2 132 questions d'appels. Test
      `a_commit_asks_nothing_about_what_the_working_tree_brings_back` ; les deux tests d'I11-1
      tiennent.
  - Mineurs :
    - m12-1 : une réindexation de secours garde la dette du lot qui a échoué (les vérifiés
      périmés sont repris avant de relire la base) ;
    - m12-2 : si elle échoue aussi, le rafraîchissement suivant refait d'abord une
      réindexation complète : la base ne reste plus en retard pour la session. Test
      `an_update_that_failed_twice_loses_nothing` pour les deux ;
    - m12-3 : test de l'échec d'écriture de la dette après un pas
      (`a_debt_not_written_after_a_step_goes_at_the_next`) ;
    - m12-4 : test de deux démarrages sans pas (`a_debt_survives_starts_without_a_step`) ;
    - m12-5 : le skill graph-model dit que la dette s'écrit par différence, avec son lot.
  - Relecture n° 13 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 2
    importants, 3 mineurs. Le coût d'un commit est bien de 0 question ; m12-1 à m12-5
    tiennent ; 12 cas jumeaux (un dépôt qui commite, l'autre non) restent identiques.
  - Importants, corrigés avec un test qui échoue sans la correction :
    - I13-1 : au retour de l'arbre de travail, un fichier qui avait changé sans être vu était
      comparé à la version commitée, pas à celle que le serveur avait vue : une signature
      ramenée à sa forme commitée, ou un symbole ajouté puis retiré, n'était plus suivi, et un
      dépendant gardé « vérifié » ne l'était plus vraiment. Test
      `a_signature_brought_back_to_the_commit_is_followed_at_a_commit` ;
    - I13-2 : un paquet trouvé au retour faisait tomber les vérifications dans la base, pas
      dans la file. Test `a_package_found_around_a_commit_keeps_the_queue_and_the_store_in_line`.
    - Les deux venaient de ma correction d'I12-1. Elle est remplacée par un retour en deux
      temps : l'état d'avant le gel, depuis la mémoire (`Indexer::restore`, vérification
      comprise), puis le disque, relu comme tout changement. Plus de mode « retour », plus de
      fenêtre `drifted` : l'écart qui la décrivait est retiré. Toujours 0 question par commit.
  - Mineurs :
    - m13-1 : les chemins non testés (M1, M5, M14, M17) disparaissent avec le mode « retour » ;
      M10 est tuée (`store_behind`) ;
    - m13-2 : un commit rattrape la base avant de figer son snapshot. Test
      `a_commit_while_the_store_is_behind_describes_the_commit` ;
    - m13-3 : l'ADR 0022 cite l'ADR 0015 ; le skill graph-model décrit le retour.
  - Relecture n° 14 (2026-10-17), contre-vérification : « pas prête », aucun bloquant, 1
    important, 4 mineurs. I13-1 et I13-2 sont fermés : 16 cas jumeaux sur 16 identiques, 25
    cas sur 25 avec le vrai watch, 0 question par commit.
  - Important :
    - I14-1 (défaut de la tâche 3.7, pas une régression) : le snapshot d'un commit ne décrit
      pas toujours l'arbre commité. Trois cas :
      - un fichier revenu à sa version commitée sans être relu : corrigé, l'index est remis au
        niveau du disque avant le gel. Test
        `files_back_to_their_committed_version_unseen_are_frozen_as_committed` ;
      - un manifeste modifié sans commit, ou un `src/lib.rs` présent d'un seul côté : reportés
        en Phase 5 par ta réponse à Q23 (tâche 5.4).
  - Mineurs :
    - m14-1 : un retour qui échoue fait réindexer tout aussitôt, au lieu d'attendre l'événement
      suivant (régression de ce tour). Test `a_return_that_fails_brings_the_disk_back_at_once` ;
    - m14-2 : test `a_commit_leaves_the_head_as_it_was` pour les chemins d'`Indexer::restore`
      (fichiers que le gel remet, résolution par nom des autres fichiers) ;
    - m14-3 : la mesure « 0 question par commit » est datée et attribuée au retour en deux
      temps ;
    - m14-4 : les tests du redémarrage ne laissent plus de dossiers temporaires.
  - Relecture n° 15 (2026-10-18), contre-vérification : « pas prête », aucun bloquant, 1
    important, 4 mineurs. Aucune régression : les corrections de la n° 14 tiennent (0 snapshot
    faux sur 35 avec le vrai watch, contre 10 à la n° 14) ; le report de Q23 est consigné.
  - Important, corrigé à ta demande (« maintenant, en Phase 4 »), avec un test qui échoue sans
    la correction :
    - I15-1 (défaut des tâches 3.5 et 3.7) : un fichier déjà indexé que `.gitignore` se met à
      sauter restait dans la tête et dans les snapshots de commit jusqu'au redémarrage ; un
      fichier qu'il ne sautait plus n'entrait qu'au commit suivant. Un fichier d'exclusion qui
      change (`.gitignore`, `.ignore`, `.git/info/exclude`, dont le dossier est maintenant
      surveillé) fait refaire le parcours : ce qu'il saute sort, ce qu'il ne saute plus entre.
      Avant le gel d'un commit, l'index refait aussi le parcours. (Relecture n° 16 : vrai
      seulement pour une racine en haut d'un dépôt ordinaire ; le worktree et la racine en
      sous-dossier sont corrigés depuis, m16-3 et m16-4.) Tests
      `a_file_an_ignore_file_skips_leaves_the_head`, `a_file_git_skips_now_is_left_out_of_a_commit`,
      `the_watch_follows_an_ignore_file` et `the_watch_follows_git_s_own_ignore_file`.
  - Mineurs :
    - m15-1 : test de l'échec de la mise au niveau du disque avant le gel : pas de snapshot,
      plutôt qu'un snapshot faux (`a_commit_snapshot_waits_for_the_disk`) ;
    - m15-2 : les chemins d'`Indexer::restore` sont testés chacun à part (nom retrouvé
      ailleurs, fichier supprimé, et dépendant). X5 (le retrait de la portée) est équivalente :
      le lot de structure du retour retire déjà le fichier et sa portée. (Faux, relevé par la
      relecture n° 16, m16-2 : sans ce retrait, la base refuse le lot du retour, et le repli
      perd la vérification des fichiers sales. Test ajouté.) ;
    - m15-3 : le rattrapage d'une base en retard est rapporté avec le rafraîchissement qui le
      fait, et un commit ne rattrape la base que s'il fige un snapshot, qui prévient les vues.
      (Inexact, relevé par la relecture n° 16, m16-1 et m16-5 : corrigé depuis, tout changement
      du dossier git rattrape la base, et les vues sont prévenues sans snapshot.) ;
    - m15-4 : skill graph-model, ligne de statut de l'ADR 0015, coût de la mise au niveau du
      disque dans l'ADR 0022, et la durée d'une consolidation sur un grand dépôt notée pour la
      Phase 8 (dans les Écarts de la Phase 4 seulement ; dans la section Phase 8 depuis la
      relecture n° 16, m16-9).
    - En corrigeant, un test de la watch réussissait pour une mauvaise raison (il écrivait le
      `.gitignore` avant la première indexation de la watch) : il attend maintenant cette
      indexation.
  - Relecture n° 16 (2026-10-18), contre-vérification : « prête avec réserves », aucun
    bloquant ni important, 9 mineurs. Les corrections de la n° 15 tiennent dans le cas courant
    (11 états de fichiers d'exclusion comparés à une indexation neuve : 0 différence).
  - Mineurs, chacun corrigé avec un test qui échoue sans la correction (mutation vérifiée) :
    - m16-1 (régression de ce tour) : un changement du dossier git sans nouveau commit ne
      rattrapait plus une base en retard. Le rattrapage passe en tête de la consolidation.
      Test `a_git_change_without_a_new_commit_catches_the_store_up` ;
    - m16-5 : la consolidation rapporte ce qu'elle a changé dans la tête en route (rattrapage,
      changements que la watch n'avait pas encore dits), et les vues sont prévenues même sans
      snapshot, ou après un échec. Tests `a_commit_without_a_snapshot_tells_what_changed_the_head`
      et `a_consolidation_tells_the_views_what_changed_the_head` ;
    - m16-2 : X5 n'était pas équivalente. Test
      `a_commit_keeps_the_verification_next_to_a_deleted_file` ;
    - m16-3 et m16-4 (défauts antérieurs, famille d'I15-1) : dans un dépôt git, le parcours
      applique les règles de git comme git, `info/exclude` du dossier commun d'un worktree
      compris ; la watch surveille aussi ce dossier et les dossiers entre le haut du dépôt et
      la racine ([ADR 0023](docs/adr/0023-fichiers-d-exclusion-suivis.md)). Tests
      `a_worktree_follows_the_main_repository_s_ignore_file` et
      `the_watch_follows_the_ignore_files_above_the_root` ;
    - m16-6 : tests de `.ignore`, d'un `.gitignore` imbriqué, et d'un fichier ignoré comparé à
      un fichier supprimé (tête, file, dette, puis questions posées) : G1, G3, G8, G9 et G10
      sont tuées. Tests `every_ignore_file_is_followed` et
      `an_ignored_file_goes_as_a_deleted_one` ;
    - m16-7 : le test Playwright du cadre attend que le cadre ne bouge plus, et vérifie que le
      point visé laisse passer le pointeur vers la carte. 160 sur 160 en répétition, et la
      suite d'accessibilité 160 sur 160 répétée quatre fois ;
    - m16-8 : coût quadratique d'un fichier créé ou supprimé (défaut antérieur), consigné pour
      la Phase 8 ;
    - m16-9 : statuts des ADR 0013 et 0015 (complétées par l'ADR 0023), durée de consolidation
      reportée dans la section Phase 8.
  - Relecture n° 17 (2026-10-18), contre-vérification : « prête avec réserves », aucun
    bloquant ni important, 7 mineurs, aucun venu de ce tour. Les neuf corrections de la n° 16
    tiennent, sondes rejouées et 23 mutations tuées par les tests du dépôt.
  - Mineurs :
    - m17-4 : trois mutations que seules les sondes tuaient, tuées maintenant (`.ignore`
      au-dessus de la racine, dossiers plus haut que le parent, relecture par le diff d'un
      commit quand `git status` échoue). Le test des dossiers au-dessus de la racine passe à
      deux niveaux ; test `a_commit_git_cannot_read_tells_what_its_diff_changed` ;
    - m17-5 : une indexation qui échoue fait relire la tête aux vues, comme une consolidation
      qui échoue : la tête a pu changer avant l'échec. Test
      `a_consolidation_tells_the_views_what_changed_the_head` ;
    - m17-3 (test antérieur) : le test de la grande carte attend jusqu'à 30 s sa première mise
      en page, au lieu des 5 s d'une action. Suite d'accessibilité : 40/40 dans 3 passages sous
      32 boucles de calcul ;
    - m17-6 : l'ADR 0022 décrit le rattrapage tel qu'il est depuis m16-1 et m16-5 ;
    - m17-1, m17-2 et m17-7 : consignés dans l'ADR 0023 (« Conséquences ») et dans les
      Écarts. Ce sont des limites de la crate `ignore` (worktree en chemins relatifs,
      sous-module), des sources d'exclusion appliquées sans être suivies, et une hypothèse de
      coût sous macOS.
  - Relecture n° 18 (2026-10-18), contre-vérification : « prête avec réserves ». Rien n'empêche
    « prête » au regard des critères de la Phase 4, de la spec et des ADR ; aucune régression,
    36 mutations rejouées (30 tuées par les tests du dépôt, 5 équivalentes). 4 mineurs, traités
    sans changer le comportement :
    - m18-3 (code antérieur) : un manifeste modifié et commité avant que la watch l'apporte
      n'était couvert par aucun test (mutation I-Z). Test
      `a_manifest_committed_unseen_is_read_from_the_commit`, qui compare la tête et le
      snapshot du commit à une indexation neuve ;
    - m18-1 et m18-2 : les limites consignées à la n° 17 sont décrites exactement (le worktree
      en chemins relatifs dépend du dossier courant du démon ; hors dépôt git, un `.gitignore`
      ou un `.ignore` au-dessus de la racine compte au parcours suivant) ;
    - m18-4 : l'hypothèse macOS est reportée dans la section Phase 8 ; les liens de l'ADR 0023
      citent les relectures n° 17 et 18.
  - Les quatre démons orphelins relevés par la relecture venaient de mes essais. Leur sortie
    d'erreur était fermée, et le défaut de journal d'avant m1 faisait mourir leurs tâches
    d'inactivité et de signaux, sans arrêter le démon. Arrêtés ; le correctif de m1 couvre ce
    cas.
  - Questions posées : m2 (Q17) et m18 (Q18), tranchées le 2026-10-16 : comportements
    actuels gardés.

Critère de sortie : la carte de Timonier s'affiche, reste stable quand tu modifies un fichier,
et passe le test des cinq secondes avec toi.

Décisions :
- [ADR 0019](docs/adr/0019-protocole-demon-extension.md) : protocole démon ↔ clients,
  un démon par projet, vues et révisions, file par client.
- [ADR 0020](docs/adr/0020-systeme-de-design.md) : système de design, jetons sur
  les variables de VSCode, couleurs des rôles, contrastes mesurés dans six thèmes.
- [ADR 0021](docs/adr/0021-carte-d-architecture.md) : carte par rôles, zoom
  sémantique, mise en page incrémentale, rendu inspiré des Blueprints, boîtes déplaçables.
- [ADR 0022](docs/adr/0022-verification-des-usages-par-le-lsp.md) : usages et
  dépendants vérifiés par le serveur de langage ; complète l'ADR 0014 (Q19).
- [ADR 0023](docs/adr/0023-fichiers-d-exclusion-suivis.md) : fichiers
  d'exclusion suivis, règles de git dans un dépôt, worktree et racine en sous-dossier ;
  complète les ADR 0013 et 0015.
Écarts :
- Les vues relisent toute la tête à chaque changement. Mesures de la relecture, sur un dépôt
  synthétique de 12 331 nœuds (2026-10-16) :
  - lecture de la tête : 28,5 ms ;
  - calcul d'une vue : 13,5 ms ;
  - `view/subscribe` : 149 ms et 3,5 Mio de JSON ;
  - sauvegarde → delta : p50 424 ms, anti-rebond de 300 ms compris.

  C'est acceptable pour le MVP. Un cache par vue viendra si un vrai dépôt le demande.
- Densité réglable (spec « Expérience et design ») : seuls les jetons de densité compacte
  existent. Sur la carte, les tailles des boîtes sont fixes, car la mise en page en dépend. Un
  réglage « confortable / compact » qui change ces tailles est reporté à la Phase 5, qui
  retouche la carte (relecture m12).
- Daltonisme (spec « palette sûre pour les daltoniens ») : il n'y a que quatre teintes libres
  de sens. En deutéranopie simulée, les teintes des barres de titre de l'interface et de la
  persistance se confondent dans les thèmes sombres (ΔE 0,5 à 0,7). Les icônes restent
  distinctes (ΔE ≥ 4,1), et le rôle est écrit : la couleur n'est jamais seule (ADR 0020).
- Une arête apparue après le premier affichage peut aller de droite à gauche jusqu'à
  « Revenir aux rôles » : elle ne relance pas la mise en page (ADR 0021).
- `view/displayed` est testé de chaque côté (webview, relais, démon), sans test d'intégration
  de bout en bout dans VSCode : la carte du projet de test ne dessine aucun fichier à
  l'ouverture.
- Tests unitaires de l'extension : le relais de vue, les messages, les chemins et les colonnes
  en ont. Le superviseur du démon et le relais LSP n'ont que des tests d'intégration, car ils
  pilotent un vrai processus et l'API de VSCode (relecture m20).
- Un fichier dont la hiérarchie d'appels n'est pas prête (serveur de langage en démarrage)
  sort de la file d'affinage sans être vérifié. Il y revient quand on l'ouvre ou l'affiche de
  nouveau.
- Un serveur de langage voit un fichier qu'aucun éditeur n'a ouvert avec le retard de son
  propre observateur de fichiers. Dans cet intervalle, une réponse peut s'appuyer sur l'ancien
  contenu d'un fichier que la question ne lit pas : ni l'extension ni le démon ne peuvent le
  voir (ADR 0022, « Conséquences »).
- Mesuré par la relecture n° 15, pour la Phase 8 : sur un dépôt synthétique de 40 000 fichiers,
  la consolidation d'un commit prend 39 à 50 s, surtout pour le recalcul des composants, et
  bloque le fil du pipeline pendant ce temps ; environ 2 s pour 10 000 fichiers.
- Mesuré par la relecture n° 16, pour la Phase 8 : créer ou supprimer un fichier coûte environ
  2 s à 10 000 fichiers (21 à 24 ms à 1 000), dont 1,8 s dans `Indexer::refresh` : un coût
  quadratique, antérieur à la Phase 4. Un fichier d'exclusion qui fait entrer ou sortir des
  fichiers coûte autant. La spec demande une mise à jour « immédiate à la sauvegarde » ;
  enregistrer un fichier existant reste à 1 ms.
- Le snapshot d'un commit est faux quand un manifeste n'est modifié que dans l'arbre de travail,
  ou quand `src/lib.rs` n'existe que d'un côté (défaut de la tâche 3.7). Reporté en Phase 5 par
  ta réponse à Q23 (tâche 5.4) : la carte montre la tête, juste ; seul le diff de revue lira ces
  snapshots.
- Un fichier devenu illisible garde ce qui pointe vers lui jusqu'au changement suivant qui
  modifie l'index, jusqu'à `editor/saved`, ou jusqu'à ce qu'on rouvre le fichier gardé :
  redevenu lisible, il ne fait pas d'événement (ADR 0022).
- Windows (named pipe) attend H8 : le socket et le verrou sont ceux d'Unix.
- La tâche 4.12 est close sur « prête avec réserves » (relecture n° 18), par ta décision (Q24).
- Fichiers d'exclusion (ADR 0023, relecture n° 17) :
  - dans un worktree créé en chemins relatifs (git 2.48 et plus), `info/exclude` n'est
    appliqué que si le démon a pour dossier courant le haut du worktree, ce que ni la CLI ni
    l'extension ne fixent ; dans un sous-module pris comme racine, il ne l'est jamais. Cause :
    la crate `ignore` 0.4.33 (relectures n° 17 et 18). Les worktrees que Timonier créera pour
    les agents devront l'être en chemins absolus, ou le démon prendre ce dossier courant
    (Q25 : plus tard) ;
  - le fichier d'exclusion global de git, un `.ignore` au-dessus du haut du dépôt et, hors
    dépôt git, un `.gitignore` ou un `.ignore` au-dessus de la racine comptent au parcours
    suivant (fichier d'exclusion suivi, commit ou redémarrage), mais leur propre changement
    n'en déclenche aucun ;
  - sous macOS, surveiller le haut d'un monorepo pourrait faire filtrer dans le processus les
    événements de tout le dépôt (notify 8.2, FSEvents) : hypothèse à mesurer, reportée dans
    la section Phase 8.

## Phase 5 — Diff de revue

Statut : en cours

- [ ] 5.1 Commande et bouton « Marquer comme revu », qui crée un snapshot `review_mark` (méthode
  JSON-RPC) avec une confirmation discrète — critère : test d'intégration.
- [ ] 5.2 Diff structurel depuis la dernière `review_mark` — critère : tests sur des paires de
  snapshots fixtures. Les arêtes pendantes, que les lectures brutes renvoient, sont filtrées
  (ADR 0009). Le diff montre :
  - composants ajoutés et supprimés ;
  - dépendances ajoutées et supprimées ;
  - cycles ajoutés (composantes fortement connexes du graphe des composants) ;
  - composants dont le fan-in ou le fan-out a bondi (seuil fixé par ADR) ;
  - un badge « modifié » pour un refactoring interne, où les déplacements comptent comme des
    modifications.
- [ ] 5.3 Rendu sur la carte : badges avec icône et texte (jamais la couleur seule), filtre
  « seulement les changements », résumé — critère : checklist d'accessibilité ; test des
  cinq secondes.
- [ ] 5.4 Snapshots de commit exacts (Q23) : le gel prend aussi les manifestes du commit et la
  présence de `src/lib.rs` — critère : tests d'un manifeste modifié sans commit (crate
  renommée, dépendance ajoutée) et d'un `src/lib.rs` présent d'un seul côté, dont le snapshot
  décrit le commit.
- [ ] 5.5 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie (proposé, à valider) : après « marquer comme revu », on ajoute à Timonier
une dépendance entre deux crates et un module ; la carte montre exactement ces changements, et
un cycle introduit dans une fixture est signalé.

Décisions : —
Écarts : —

## Phase 6 — MCP, initialisation et suivi

Statut : à faire

Prérequis : Q10 et Q11.

- [ ] 6.1 ADR — critère : ADR acceptées. Sujets :
  - SDK MCP (rmcp 3.5) et transport stdio ;
  - forme de l'en-tête de boîte aux lettres ;
  - formats versionnés (schémas JSON) de `.timonier/plan.json`, `todo.json` et
    `sessions/<id>.json`.
- [ ] 6.2 `timonier-mcp` : serveur MCP, mince proxy vers le démon (qu'il démarre au besoin),
  avec l'en-tête de boîte aux lettres dans chaque réponse (H10) — critère : test d'intégration
  client MCP → proxy → démon ; snapshot de la liste des outils.
- [ ] 6.3 Outils de suivi `session_start`, `session_end`, `plan_add_task`, `plan_update_task`,
  `todo_add`, `todo_complete`, plus les outils de lecture selon Q10. Écritures structurées et
  validées, le démon est le seul écrivain — critère : les entrées invalides et les transitions
  d'état interdites sont refusées (tests).
- [ ] 6.4 Les changements faits hors session sont marqués « non suivis » — critère : une
  modification sans session ouverte est marquée (test).
- [ ] 6.5 Dette de revue : chaleur par composant (code écrit en session et non couvert par une
  `review_mark` ultérieure ; changements hors session = non suivis), en calque sur la carte —
  critère : test sur fixtures ; checklist d'accessibilité.
- [ ] 6.6 Panneaux — critère : les panneaux se mettent à jour en direct quand l'agent appelle
  les outils (test d'intégration) ; test des cinq secondes. Contenu :
  - plan : jalons → tâches, avancement calculé, panneau latéral permanent ;
  - todo en direct ;
  - journal des sessions : objectif, tâches, composants touchés, diff structurel, tests
    (l'effet sur les performances arrive en Phase 8).
- [ ] 6.7 Le pont (H5) : où en est le plan, ce qui s'est passé depuis la dernière revue, où
  regarder — critère : test des cinq secondes.
- [ ] 6.8 `timonier init` — critère : tests sur des projets fixtures Rust et TS ; une deuxième
  exécution ne change rien ; `--dry-run` affiche le diff exact. Contenu :
  - détection : langages, gestionnaire de paquets, runner de tests, agents installés, état
    git ;
  - squelette de `.timonier/` ;
  - bloc géré, délimité par des marqueurs, dans `AGENTS.md` et ses équivalents ;
  - déclaration MCP dans `.mcp.json` et `.vscode/mcp.json` ;
  - hooks d'agent (refus de clore une session si la todo n'est pas à jour ou si le journal
    est vide) ;
  - hook git qui régénère `actual.json`, base SQLite ignorée par git ;
  - plan avec un jalon « Initialisation » et une première session ;
  - `--dry-run` et idempotence.
- [ ] 6.9 `timonier uninit` — critère : init puis uninit rend un arbre identique à l'original
  (test).
- [ ] 6.10 `timonier doctor` : vérifie et répare la configuration (MCP déclaré, bloc géré à
  jour, conventions présentes). Le test de fumée tracé arrive en Phase 8 (H6) — critère :
  chaque diagnostic et chaque réparation sont testés.
- [ ] 6.11 Packs Rust et TypeScript, volet init : détection du runner, configuration JUnit XML,
  conventions copiées dans `.timonier/conventions/` — critère : tests sur fixtures.
- [ ] 6.12 Détection dans VSCode d'un dépôt non configuré, avec une proposition discrète de
  lancer init — critère : test d'intégration.
- [ ] 6.13 Bascule : `timonier init` sur le dépôt Timonier. Ensuite, une session Timonier par
  tâche, et plan et todo tenus via MCP selon Q11 — critère : vérifiable dans le journal des
  sessions.
- [ ] 6.14 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie : Timonier pilote son propre développement.

Décisions : —
Écarts : —

## Phase 7 — Tableau blanc et intention

Statut : à faire

Réponses appliquées : Q3 (pas de tldraw) et Q6.

- [ ] 7.0 ADR sur le remplaçant de tldraw (React Flow ou Excalidraw, voir Q3), soumise à ta
  validation car la spec en parlait explicitement — critère : ADR acceptée, licence vérifiée.
- [ ] 7.1 Tableau blanc dans la webview, avec la bibliothèque retenue en 7.0 — critère : dessin
  libre, annulation et clavier fonctionnent dans le build de production du panneau, pas
  seulement en développement.
- [ ] 7.2 Formes typées optionnelles — critère : sérialisation testée. Correspondances :
  - boîte → composant voulu (nom, rôle) ;
  - flèche → arête `allowed`, `forbidden` ou `required` ;
  - zone → couche ;
  - note → contrainte en texte rattachée à un élément ;
  - note budget (latence p50/p95, mémoire max, allocations, nombre d'appels).
- [ ] 7.3 `.timonier/intent.json` version 1, conforme à la spec : sémantique séparée de la
  géométrie, IDs `C-` / `E-` / `L-` / `N-` stables, écrit seulement sur action de l'humain,
  ordre stable pour des diffs git lisibles, schéma JSON — critère : round-trip testé ; les
  IDs ne changent pas quand une forme est déplacée.
- [ ] 7.4 Interprétation du dessin libre (texte et vision), selon Q6 — critère : défini avec la
  réponse.
- [ ] 7.5 Outils MCP `get_intended_architecture` et `get_drift` — critère : tests
  d'intégration MCP.
- [ ] 7.6 Option « comparer à l'intention » sur la carte — critère : des fixtures d'intention et
  de graphe donnent la dérive attendue. Contenu :
  - ancrage boîte dessinée ↔ composant réel par nom et emplacement, calculé et jamais écrit ;
  - composants dessinés mis en évidence, composants manquants listés ;
  - arêtes interdites franchies signalées (une arête entre couches s'applique à tout leur
    contenu) ;
  - arêtes requises absentes signalées.
- [ ] 7.7 Relecture par `phase-reviewer` — critère : verdict « prête ».

Critère de sortie (proposé, à valider) : tu dessines trois boîtes et une flèche interdite pour
Timonier ; la carte les met en évidence, liste les manquants et signale la violation ; l'agent
lit l'intention via MCP.

Décisions : —
Écarts : —

## Phase 8 — Performances

Statut : à faire

- [ ] 8.1 ADR — critère : ADR acceptées. Sujets :
  - stockage des traces et des métriques ;
  - nom de la feature qui désactive l'instrumentation ;
  - niveau 1 sur macOS (H8) ;
  - compteurs matériels.
- [ ] 8.2 `timonier test -- <commande>`, niveau 1 — critère : test d'intégration sur un projet
  fixture. Contenu :
  - durée, CPU, mémoire max et I/O via cgroups v2 / procfs sur Linux, avec un repli sur
    macOS ;
  - durée par test lue dans le JUnit XML (quick-junit 0.8).
- [ ] 8.3 Injection de `OTEL_EXPORTER_OTLP_ENDPOINT` et `OTEL_RESOURCE_ATTRIBUTES`
  (`timonier.run_id`, `timonier.snapshot`) ; chaque test devient un span racine — critère :
  test.
- [ ] 8.4 `timonier-otlp` : récepteur gRPC 4317 et HTTP 4318 (tonic 0.14, opentelemetry-proto
  0.33), ports occupés gérés, rattachement au graphe par les attributs `code.*` — critère :
  test d'intégration avec un vrai exporteur OTLP.
- [ ] 8.5 Vérification de l'instrumentation : fonctions publiques jamais vues, spans sans
  `code.*`, surcoût estimé contre le niveau 1 ; outils MCP `get_instrumentation_gaps` et
  `get_instrumentation_conventions(lang)` — critère : une fixture aux trous connus les fait
  tous remonter.
- [ ] 8.6 Instrumentation — critère : `timonier doctor` produit une trace rattachée au graphe.
  Contenu :
  - Timonier s'instrumente lui-même, derrière la feature ;
  - volet instrumentation de `timonier init` pour Rust et TS (SDK OpenTelemetry, module
    d'initialisation) ;
  - test de fumée de `timonier doctor` (H6).
- [ ] 8.7 Vues — critère : test des cinq secondes. Contenu :
  - timeline par test (spans emboîtés, ressources du processus dessous, uPlot) ;
  - carte annotée (temps cumulé, nombre d'appels, arêtes épaissies selon le trafic) ;
  - évolution par commit ;
  - à ta demande (2026-10-18), s'inspirer des panneaux « Performances » des outils de
    développement de Chrome et de Firefox : un arbre des appels qu'on déplie, avec le temps
    passé en ms et les ressources utilisées à chaque niveau.
- [ ] 8.8 Budgets — critère : un budget dépassé volontairement est visible sur la carte et
  lisible via MCP. Contenu :
  - chaque note budget de l'intention est comparée à la médiane de plusieurs runs ;
  - un dépassement apparaît en rouge, avec une icône, sur la carte et dans le journal ;
  - outils MCP `get_budgets`, `get_budget_violations` et `get_perf(component)`.
- [ ] 8.9 Détection de régressions selon la méthode décidée dans la spec — critère : tests
  statistiques sur des jeux de données connus ; une fonction ralentie exprès passe de
  « probable » à « confirmée ». Méthode :
  - instructions CPU par compteurs matériels quand c'est possible, temps réel affiché ;
  - 5 exécutions, jusqu'à 15 si la variance est forte ; médiane et MAD ;
  - référence = médiane des 5 derniers snapshots de la branche principale ;
  - régression signalée si Mann-Whitney p < 0,01 et écart ≥ max(5 %, 3 × MAD) ;
  - « probable », puis « confirmée » après un nouveau run ;
  - référence réinitialisée si l'empreinte de la machine change.
- [ ] 8.10 Relecture par `phase-reviewer` — critère : verdict « prête ».

Reporté de la Phase 4 (Écarts) : le coût de Timonier lui-même sur un grand dépôt, mesuré par
les relectures n° 15 et 16 sur des dépôts synthétiques (build release, tmpfs) :
- consolidation d'un commit : 39 à 50 s à 40 000 fichiers, environ 2 s à 10 000, surtout pour
  le recalcul des composants ; le fil du pipeline est bloqué pendant ce temps ;
- fichier créé ou supprimé, ou fichier d'exclusion qui fait entrer ou sortir des fichiers :
  environ 2 s à 10 000 fichiers, coût quadratique (1,8 s dans `Indexer::refresh`) ;
- hypothèse non mesurée, à vérifier sous macOS (H8) : surveiller le haut d'un monorepo y fait
  filtrer dans le processus les événements de tout le dépôt (notify 8.2, FSEvents ; ADR 0023,
  relecture n° 17, m17-7).

Critère de sortie (proposé, à valider) : `timonier test` sur Timonier produit une timeline par
test, avec des spans rattachés aux nœuds du graphe ; une régression provoquée est détectée ;
un budget dépassé apparaît en rouge.

Décisions : —
Écarts : —

## Phase 9 — Fin du MVP

Statut : à faire

- [ ] 9.1 Inventaire des technologies, exposé par l'outil MCP `get_tech_inventory` — critère :
  sur Timonier, un SBOM valide ; les doublons d'une fixture sont détectés. Contenu :
  - lecture des manifestes (`Cargo.toml`, `package.json` ; `pyproject.toml` et `go.mod` si le
    temps le permet) ;
  - SBOM CycloneDX (cyclonedx-bom 0.8) : langages, frameworks, dépendances, versions,
    licences ;
  - CVE connues (source fixée par ADR, accès réseau) ;
  - doublons fonctionnels, et dépendances ajoutées depuis la dernière revue.
- [ ] 9.2 Processus de développement — critère : le scénario « Solo discipliné » fonctionne de
  bout en bout sur un dépôt de test. Contenu :
  - préréglages Solo rapide, Solo discipliné et Équipe ; `.timonier/workflow.json` ;
    panneau de réglages ; bloc géré dérivé du processus ;
  - GitHub via `gh` : un ticket par tâche, branche `tim/<ticket>-<slug>`, PR en fin de
    session avec une description générée ;
  - génération du workflow de CI ;
  - outils MCP `get_workflow`, `task_start`, `open_pull_request` et `link_ticket` ;
  - jetons dans le trousseau de l'IDE.
- [ ] 9.3 Coût de l'IA, selon Q7 : la télémétrie OpenTelemetry native de Claude Code est
  pointée par init vers le récepteur, puis les tokens sont comptés par session et par tâche.
  Sans source, la valeur est « non mesuré », jamais une estimation inventée — critère : une
  session réelle donne des tokens attribués.
- [ ] 9.4 Assistant « Nouveau projet piloté », selon Q8 — critère : défini avec la réponse.
- [ ] 9.5 Relecture complète du MVP par `phase-reviewer` contre la section « Découpage » —
  critère : verdict « prête ».
- [ ] 9.6 Critère de sortie du MVP selon la spec : un projet réel (Q9) piloté de bout en bout
  sans lire le code ligne à ligne — critère : constaté avec toi.

Décisions : —
Écarts : —

---

## Dépendances vérifiées

Vérifiées le 2026-10-14 sur crates.io, le registre npm et les dépôts officiels. Revérifier
avant ajout si plus d'un mois s'est écoulé.

| Paquet | Version | Licence | Remarque |
| ------ | ------- | ------- | -------- |
| tokio | 1.53.3 | MIT | |
| thiserror / anyhow | 2.0.21 / 1.0.104 | MIT OR Apache-2.0 | |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | MIT | |
| tracing-opentelemetry | 0.34.0 | MIT | Va avec opentelemetry 0.33 |
| opentelemetry, _sdk, -otlp, -proto | 0.33.1 | Apache-2.0 | Avant la 1.0 ; l'API traces est encore « Beta » |
| rusqlite | 0.40.2 | MIT | |
| rusqlite_migration / refinery | 2.6.0 / 0.10.0 | Apache-2.0 / MIT | rusqlite_migration retenu (ADR 0011) ; refinery écarté |
| tree-sitter / -rust / -typescript | 0.27.1 / 0.24.2 / 0.23.2 | MIT | Revérifiés le 2026-10-16 ; utilisés (ADR 0012) |
| notify / notify-debouncer-full | 8.2.0 / 0.7.0 | CC0-1.0 / MIT OR Apache-2.0 | notify 9 encore en RC ; notify vient par le débounceur, qui le réexporte |
| ignore / globset | 0.4.33 / 0.4.20 | Unlicense OR MIT | Revérifiés le 2026-10-16 |
| rayon | 1.12.0 | MIT OR Apache-2.0 | Vérifié le 2026-10-16 ; extraction parallèle (ADR 0013) |
| toml | 1.1.8 | MIT OR Apache-2.0 | Vérifié le 2026-10-16 ; manifestes des packs et de Cargo, features `std`, `serde`, `parse` |
| serde / serde_json / schemars | 1.0.229 / 1.0.154 / 1.2.3 | MIT (OR Apache-2.0) | |
| jsonrpsee | 0.26.1 | MIT | **Écarté** (ADR 0019) : serveurs HTTP et WebSocket seulement |
| ts-rs | 12.0.1 | MIT | Vérifié le 2026-10-16 ; types TypeScript du protocole (ADR 0019). specta écarté (2.0.0-rc.25) |
| async-lsp / tower-lsp-server | 0.2.4 / 0.23.0 | MIT OR Apache-2.0 | tower-lsp n'est plus maintenu ; lsp-types est figé |
| rmcp | 3.5.1 | Apache-2.0 (parties en MIT) | SDK Rust officiel de MCP |
| tonic / prost / axum | 0.14.6 / 0.14.4 / 0.8.9 | MIT / Apache-2.0 / MIT | |
| clap | 4.6.7 | MIT OR Apache-2.0 | |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 | |
| graphrs | 0.12.0 | MIT | **Écarté** (ADR 0016) : son Leiden ne part pas d'une partition initiale |
| quick-junit | 0.8.0 | Apache-2.0 OR MIT | |
| cyclonedx-bom | 0.8.1 | Apache-2.0 | |
| scip | 0.10.0 | Apache-2.0 | v2 ; dépend de rust-protobuf, pas de prost |
| procfs / cgroups-rs | 0.18.0 / 0.5.1 | MIT OR Apache-2.0 | |
| insta / proptest / tempfile / assert_cmd | 1.49.0 / 1.12.0 / 3.27.0 / 2.2.2 | Apache-2.0 / MIT OR Apache-2.0 | |
| @xyflow/react | 12.12.0 | MIT | Pas `reactflow` (figé). Revérifié et utilisé le 2026-10-16 (ADR 0021) ; zustand, classcat, @xyflow/system, use-sync-external-store en MIT |
| d3-drag, -zoom, -selection, -transition, -interpolate, -color, -dispatch, -timer / d3-ease | 3.x | ISC / BSD-3-Clause | Vérifiés le 2026-10-16 ; tirés par @xyflow/system, empaquetés dans la webview avec leur notice |
| scheduler | 0.28.0 | MIT | Vérifié le 2026-10-16 ; tiré par react-dom |
| elkjs | 0.12.0 | EPL-2.0 OR GPL-3.0-or-later | Utilisé sous EPL-2.0, sans worker (ADR 0021) ; la notice lie le paquet de cette version |
| tldraw | 5.5.2 | Propriétaire, avec clé | **Écarté** (Q3) |
| uplot / pixi.js | 1.6.32 / 8.22.0 | MIT | |
| react, react-dom | 19.3.0 | MIT | |
| vite / @vitejs/plugin-react / vitest | 8.3.4 / 6.1.2 / 5.0.4 | MIT | |
| typescript | 6.0.3 (7.0.2 existe) | Apache-2.0 | H11 |
| eslint / typescript-eslint / prettier | 10.12.0 / 8.71.2 / 3.9.10 | MIT | |
| @vscode/vsce / test-cli / test-electron | 4.0.0 / 0.0.15 / 3.1.0 | MIT | Node ≥ 22 |
| @types/vscode | 1.140.0 | MIT | VSCode stable : 1.141.2 |
| @vscode-elements/elements | 2.5.1 | MIT | `@vscode/webview-ui-toolkit` est déprécié |
| vscode-jsonrpc | 9.0.3 | MIT | Utilisé (ADR 0019) ; revérifié le 2026-10-16, sans dépendance |
| axe-core / @playwright/test | 4.14.0 / 1.64.1 | MPL-2.0 / Apache-2.0 | Développement seulement ; revérifiés et utilisés le 2026-10-16 (ADR 0020) |
| esbuild | 0.28.2 | MIT | Bundle de l'extension |
| @eslint/js / eslint-plugin-react-hooks | 10.0.1 / 7.1.1 | MIT | |
| @types/node / @types/react / @types/react-dom / @types/mocha | 24.19.2 / 19.3.0 / 19.3.0 / 10.0.10 | MIT | `@types/node` 24 = Node 24.21 de VSCode 1.141 |
| actions/checkout / setup-node / upload-artifact | v7.0.1 / v7.1.0 / v7.0.2 | MIT | Actions GitHub officielles, par tag majeur |
| Swatinem/rust-cache | v2.9.2 (`6323deb`) | LGPL-3.0 | Action tierce, épinglée par SHA |

Toolchain : Rust stable 1.99.1 (installée en local le 2026-10-16 ; 1.96 reste pour la MSRV), édition 2024 stable depuis 1.85 ;
Node 24 LTS (26 devient LTS le 2026-10-28) ; VSCode 1.142.0 pour les tests d'intégration et la
capture des thèmes.

## Journal

- 2026-10-14 — Phase 0 :
  - spec lue ;
  - formats Claude Code et dépendances vérifiés ;
  - CLAUDE.md, skills, hooks (23 tests verts), subagent et ADR 0001–0002 écrits ;
  - plan soumis pour validation.
- 2026-10-14 — Réponses à Q2–Q7 :
  - dépôt public de Nicolas Michel, créé plus tard ;
  - tldraw écarté ;
  - options recommandées retenues pour Q4, Q5, Q6 et Q7.

  Q1 (licence) a été tranchée ensuite : MIT.
- 2026-10-14 — Phase 1 :
  - squelette réalisé ;
  - relecture `phase-reviewer`, verdict « pas prête » : 1 bloquant, 5 importants, 9 mineurs,
    tous traités ;
  - CI verte au premier push sur Linux et macOS (run 38604588410) ;
  - reste ton contrôle visuel F5 (1.9).
- 2026-10-14 — Ajout de 1.15 à ta demande : icône Timonier dans la barre d'activité, avec une
  vue d'ensemble et le bouton « Ouvrir la carte » (ADR 0007).
- 2026-10-14 — Phase 1 validée par toi (« très bien pour l'icône, passe à la suite »). ADR 0001
  à 0005 et 0007 acceptées ; 0006 attend Q13. Phase 2 commencée.
- 2026-10-14 — Phase 2 :
  - ADR 0008 à 0011 ;
  - modèle (`timonier-core`) et stockage versionné (`timonier-store`) ;
  - trois relectures `phase-reviewer` :
    - n° 1 « pas prête » (1 bloquant, 6 importants) ;
    - n° 2 : 2 importants introduits ou laissés par les corrections ;
    - n° 3 « prête » ;
    - tout corrigé ;
  - 105 tests Rust verts ;
  - mesures consignées ;
  - Q14 et Q15 posées.
- 2026-10-16 — Phase 2 validée par toi (« tout me semble bien ») :
  - ADR 0008 à 0011 acceptées ; Q14 (fichiers hors package gardés) et Q15 (ADR acceptées à la
    validation de la phase) tranchées ;
  - Q13 : bilingue pour l'instant, ADR 0006 acceptée ;
  - Q12 reportée ;
  - Phase 3 commencée.
- 2026-10-16 — Phase 3 :
  - ADR 0012 à 0018 ;
  - packs Rust et TypeScript, extraction, résolution, cache, watcher, affinage LSP,
    consolidation au commit, composants (Leiden écrit par nous), rôles, compaction ;
  - relecture n° 1 « pas prête » (1 bloquant, 7 importants, 10 mineurs), tout corrigé ;
  - 192 tests Rust verts (un test de charge à part, en release) ;
  - mesures sur Timonier : 456 ms à froid, p95 de 10 ms par sauvegarde.
- 2026-10-16 — Phase 3 validée par toi (« commit push, vérifie que la CI passe, et ensuite passe
  à la Phase 4 ») :
  - ADR 0012 à 0018 acceptées ;
  - relecture n° 2 « pas prête » (1 bloquant, 4 importants, 5 mineurs), tout corrigé ;
  - 198 tests Rust verts ;
  - captures du plugin prises (VSCode 1.142, en français, thèmes sombre et clair) ;
  - Phase 4 commencée.
- 2026-10-16 — Relecture n° 3 de la Phase 3, arrivée après ta validation : « pas prête ».
  F1 (le watcher se nourrissait de ses propres lectures), P1 (3.12 cochée à tort) et m-f sont
  corrigés, avec deux tests de régression. 3.12 est décochée.
- 2026-10-16 — Contre-vérification : F1 et P1 corrigés ; m-f ne l'était qu'en partie (un crate
  vu d'abord par un autre parcours). Corrigé, avec un troisième test.
- 2026-10-16 — Phase 4 :
  - ADR 0019 à 0021 ;
  - démon JSON-RPC sur socket Unix, CLI `daemon`, extension qui le supervise, relais LSP,
    système de design, carte par rôles, mise en page stable, niveau de détail, navigation ;
  - `README.md` passé en anglais et enrichi, à ta demande (« en anglais je veux dire ») ;
  - test des cinq secondes réussi avec toi (4.11) ;
  - à ta demande, des couleurs par rôle (« quelques couleurs pour une meilleure
    lisibilité »), puis un rendu inspiré des Blueprints d'Unreal Engine ;
  - relecture n° 1 « pas prête » (aucun bloquant, 5 importants, 21 mineurs). Tout est corrigé,
    consigné ou posé en question (Q16 à Q19). En passant, j'ai trouvé deux défauts de plus :
    le démon mourait avec la fenêtre qui l'avait lancé, et les arêtes d'une boîte ouverte
    barraient le texte des autres boîtes.
- 2026-10-16 — Relecture n° 2 de la Phase 4 « pas prête » (aucun bloquant, 4 importants,
  17 mineurs). Tout est corrigé ou consigné, sauf le test des cinq secondes, à refaire avec
  toi sur le nouveau rendu. Q16 à Q18 tranchées ; Q19 et Q20 ouvertes ; Q17 à confirmer.
- 2026-10-17 — Tes réponses : test des cinq secondes n° 2 réussi ; Q17 confirmée (b) ; Q19 (b),
  « et on optimise le code à fond » ; Q20 (b), boîtes déplaçables.
  - 4.5 rouverte puis refaite (ADR 0022) : usages vérifiés par leurs définitions, dépendants
    nommés par les références, environ 670 questions pour 6 100 sites sur Timonier ;
  - boîtes déplaçables, épinglées, gardées par espace de travail (H13) ;
  - le banc de 4.10 saisit le fond : 54 i/s, comme avant le nouveau rendu.
- 2026-10-17 — Relecture n° 3 « pas prête » (aucun bloquant, 4 importants, 9 mineurs, tous sur
  l'ADR 0022 et les boîtes déplacées). Corrigés : plus de regroupement des chemins, position
  des noms génériques, vérifications périmées au redémarrage, coût des recherches mesuré et
  réduit, réponses douteuses écartées. Q21 posée.
- 2026-10-17 — Relecture n° 4 « pas prête » (1 important, 7 mineurs). Corrigés : recherche de
  dépendants périmée qui retombe sur le nom, doute réservé aux noms liés par un import,
  redémarrage face à un fichier changé, annulation des questions abandonnées, comptes des
  recherches dans le statut.
- 2026-10-17 — Relecture n° 5 « pas prête » (2 importants, 4 mineurs). Corrigés : rien n'est
  vérifié sur un contenu non enregistré, ni sur un changement pas encore indexé ; recherches de
  dépendants par symbole ; appels en détail revérifiés après un changement pendant l'absence ;
  fichiers vérifiés entiers plus redemandés ; chiffres mesurés de nouveau.
- 2026-10-17 — À ta demande, les commits de ce dépôt utilisent l'adresse « noreply » de
  GitHub (`git config --local user.email`), et non plus ton adresse personnelle.
- 2026-10-17 — Relecture n° 6 « pas prête » (1 important, 4 mineurs). Corrigés : une réponse
  dans `node_modules` ne garde plus la tâche, seuls les fichiers source suspendent les
  réponses (Q22 posée), une question perdue au changement de relais rend sa tâche, tests des
  mutations survivantes.
- 2026-10-17 — Relecture n° 7 « pas prête » (1 important, 4 mineurs). Corrigés : un client
  relais qui n'écrit plus cesse de relayer (le démon tournait à vide), le démon relit un
  fichier trouvé différent du disque, tests des mutations survivantes.
- 2026-10-17 — Relecture n° 8 « pas prête » (1 important, 6 mineurs). Corrigés : plus de
  boucle sur un fichier illisible, une question perdue rend sa tâche aussitôt, le changement de
  relais libère ce qui était gardé, tests des mutations survivantes, tests sans fichiers
  laissés dans `/tmp`.
- 2026-10-17 — Relecture n° 9 « pas prête » (2 importants, 4 mineurs). Corrigés : « Extension
  Tests » (F5) retrouve son projet, ce que l'affinage doit survit au redémarrage (migration
  0004), tests des mutations survivantes.
- 2026-10-17 — Relecture n° 10 « pas prête » (1 important, 5 mineurs). Corrigés : « Extension
  Tests » (F5) passe et ne laisse rien, la dette s'écrit dans un ordre sûr et seulement ce qui
  change, un commit ne relance plus de recherches.
- 2026-10-17 — Relecture n° 11 « pas prête » (1 important, 6 mineurs). Corrigés : l'optimisation
  des commits (m10-5) est retirée, elle faisait manquer des dépendants ; un lot et sa dette
  s'écrivent ensemble ; F5 compile le démon ; tests des chemins qui n'en avaient pas.
- 2026-10-17 — Relecture n° 12 « pas prête » (1 important, 5 mineurs). Corrigés : un commit ne
  coûte plus aucune question pour ce qui revient tel quel (sa vérification est gardée), une
  mise à jour qui échoue deux fois ne perd plus rien, tests des chemins d'échec.
- 2026-10-17 — Tes réponses : Q21 « oui » (positions gardées sur ta machine, boîtes qui restent
  en place) ; l'adresse « noreply » de GitHub aussi dans ta configuration Git globale. À ta
  demande, les 11 commits publiés sont réécrits avec cette adresse, signés, mêmes arbres,
  mêmes messages et mêmes dates (`main` : fc60270 → 5f1152a, poussée forcée) ; l'ancien
  historique reste dans la branche locale `backup/before-email-rewrite`.
- 2026-10-17 — Relecture n° 13 « pas prête » (2 importants, 3 mineurs). Corrigés : le retour de
  l'arbre de travail après un commit se fait en deux temps (l'état d'avant le gel, puis le
  disque), ce qui suit les vrais changements sans coûter de question ; un commit rattrape une
  base en retard avant de figer son snapshot.
- 2026-10-18 — Relecture n° 14 « pas prête » (1 important, 4 mineurs). Corrigés : l'index est
  remis au niveau du disque avant le gel d'un commit, un retour qui échoue est rattrapé aussitôt,
  tests de `Indexer::restore`. Ta réponse à Q23 : les deux autres cas de snapshots de commit
  faux sont pour la Phase 5 (tâche 5.4).
- 2026-10-18 — Relecture n° 15 « pas prête » (1 important, 4 mineurs), sans régression. À ta
  demande, I15-1 corrigé en Phase 4 : un fichier d'exclusion qui change fait refaire le
  parcours, et l'index refait le parcours avant le gel d'un commit.
- 2026-10-18 — Tes réponses : Q22 (a), la pause des vérifications tant qu'un fichier source n'est
  pas enregistré est gardée ; le serveur Vite du 16 octobre est arrêté ; la branche locale
  `backup/before-email-rewrite` est supprimée.
- 2026-10-18 — Ta demande pour la Phase 8 : les vues de performance s'inspirent des panneaux
  « Performances » de Chrome et de Firefox (arbre des appels dépliable, temps en ms, ressources) ;
  consignée dans la tâche 8.7.
- 2026-10-18 — Relecture n° 16 « prête avec réserves » (9 mineurs). Corrigés : un changement du
  dossier git rattrape toujours une base en retard, et les vues sont prévenues même sans
  snapshot ; dans un worktree et dans un sous-dossier, les fichiers d'exclusion sont suivis
  (ADR 0023) ; tests ajoutés ; test Playwright stabilisé. Le coût d'un fichier créé ou
  supprimé sur un grand dépôt est noté pour la Phase 8.
- 2026-10-18 — Relecture n° 17 « prête avec réserves » (7 mineurs, aucun venu du tour
  précédent). Corrigés : trois trous de test, une indexation qui échoue prévient les vues, le
  test de la grande carte tient sous charge, ADR 0022 à jour. Limites des fichiers
  d'exclusion consignées dans l'ADR 0023.
- 2026-10-18 — Relecture n° 18 « prête avec réserves », sans rien qui bloque « prête » au regard
  des critères. Traités : un test pour un manifeste commité avant d'être relu, les limites de
  l'ADR 0023 décrites exactement, l'hypothèse macOS reportée en Phase 8.
- 2026-10-18 — Tes réponses : Q24, clore 4.12 sur « prête avec réserves » ; Q25, le dossier
  courant du démon pour les worktrees en chemins relatifs, plus tard. Phase 4 terminée, en
  attente de ta validation.
- 2026-10-18 — Phase 4 validée par toi (« commit push et passe à la suite ») :
  - ADR 0019 à 0023 acceptées ;
  - 18 relectures, la dernière « prête avec réserves », close ainsi par ta décision (Q24) ;
  - 320 tests Rust, 148 tests unitaires Node, 19 tests d'intégration et 40 tests
    d'accessibilité verts ;
  - Phase 5 commencée.
