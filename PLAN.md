# PLAN — Timonier (MVP)

Plan de réalisation du MVP défini dans [docs/spec.md](docs/spec.md) (section « Découpage »),
selon les phases du prompt de démarrage. La spec fait autorité. Les décisions techniques non
couvertes par la spec sont dans [docs/adr/](docs/adr/README.md).

Format (voir [ADR 0002](docs/adr/0002-format-de-plan-md.md) et
[ADR 0035](docs/adr/0035-taches-dans-plan-json.md)) :

- une section `## Phase N — Titre` par phase, avec une ligne `Statut : à faire | en cours | terminée` ;
- une puce par tâche, `N.k Titre — critère : …`, et son détail en dessous ;
- en fin de phase, une ligne `Décisions :` et une ligne `Écarts :`.

**Depuis la bascule (2026-10-20, 6.13, Q11), l'état des tâches n'est plus ici** : chaque phase
est un jalon de `.timonier/plan.json`, chaque tâche une tâche, tenues par les outils MCP de
Timonier et montrées dans la vue « Plan, todo et sessions ». Ce fichier garde leur critère et
leur détail, les questions, les hypothèses, les décisions, les mesures et les relectures. Le
hook de démarrage de session lit `plan.json`.

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

**Q8 — Assistant « Nouveau projet piloté »** (Phase 9) — ✅ répondu le 2026-10-21 : **en
Phase 9**. L'assistant de VSCode crée un projet : choix de la stack, générateur officiel
(`cargo new`, `npm create`…), puis `timonier init`, `doctor` et le test de fumée.
- Il est décrit dans « Initialisation » mais absent de la liste du MVP et du prompt.
- La détection à l'ouverture d'un dépôt non configuré existe depuis la Phase 6.

**Q9 — Projet pilote du critère de sortie du MVP** (Phase 9) — ✅ répondu le 2026-10-21 : **un
nouveau projet**, créé et développé par un agent sous Timonier (en Rust ou en TypeScript, avec des
tests), qui sert aussi d'épreuve à l'assistant de Q8. Lequel exactement : à choisir ensemble au
début de 9.6.
- Critère : « un projet réel piloté de bout en bout sans lire le code ligne à ligne ».

**Q10 — Outils MCP de lecture du suivi** (Phase 6) — ✅ répondu le 2026-10-20 : **les
ajouter**. `get_plan`, `get_todo` et `get_session`, en lecture seule, avec le même en-tête de
boîte aux lettres que les autres outils.

**Q11 — `PLAN.md` et `.timonier/plan.json`** (Phase 6) — ✅ répondu le 2026-10-20 :
**les tâches dans `plan.json`**. Jalons et tâches (phases 6 à 9, et les phases faites comme
jalons terminés) passent dans `plan.json`, seule source, tenue par les outils MCP et montrée
dans le panneau. `PLAN.md` garde ce qui est pour toi : questions, hypothèses, décisions,
mesures, journal des relectures. Le hook de début de session lira `plan.json` (à la bascule,
6.13).

**Q12 — Identifiant d'éditeur sur la Marketplace VSCode** (avant toute publication) — ✅
répondu le 2026-10-20 : **`NicolasMichel`**, créé par toi sur la Marketplace. Le paquet porte
`publisher: NicolasMichel`, et l'identifiant de l'extension devient `NicolasMichel.timonier`
(tests d'intégration compris). Reporté jusque-là (2026-10-16), avec `nicolas-a-michel`
provisoire.

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

**Q84 — Les commandes de `.timonier/tests.json`** (audit de sécurité du 2026-10-21, constat 1) —
✅ répondu le 2026-10-21 : **approuvées par poste**. Timonier garde hors du dépôt l'empreinte
des commandes qu'il a écrites ou que le développeur a approuvées ; une commande inconnue n'est
pas lancée : il la montre et demande `--approve` une fois ; `init` approuve ce qu'il écrit.

**Q83 — Le préréglage par défaut** (9.2) — ✅ répondu le 2026-10-21 : **Solo rapide**. Commits
directs sur la branche principale ; Solo discipliné et Équipe se choisissent dans les réglages.

**Q82 — La fusion automatique de Solo discipliné** (9.2) — ✅ répondu le 2026-10-21 :
**l'auto-merge de GitHub** (`gh pr merge --auto --squash`) ; GitHub fusionne quand la CI générée
passe ; Timonier propose d'activer « Allow auto-merge » et la protection de branche.

**Q81 — L'accès à GitHub** (9.2) — ✅ répondu le 2026-10-21 : **la connexion de `gh`**. Timonier
ne stocke aucun jeton ; sans connexion, il propose `gh auth login`.

**Q80 — Le dépôt de test du critère de 9.2** — ⏸ mis de côté le 2026-10-21 (« laisse ça de côté
pour le moment ») : les tests passent par un faux `gh` ; le scénario réel sur GitHub attend.

**Q79 — Les CVE de l'inventaire** (9.1) — ✅ répondu le 2026-10-21 : **OSV.dev à la demande**.
Le démon interroge OSV seulement quand l'agent ou le développeur demande l'inventaire, jamais en
tâche de fond ; un réglage le coupe ; les réponses sont gardées 24 h et le rapport dit leur date.
Interroger OSV lui envoie les noms et versions des dépendances.

**Q78 — Où `timonier init` met `@opentelemetry/api`** (8.6, relecture n° 1, m6) — ✅ répondu le
2026-10-21 : **en `dependencies`**. Le module de production l'importe ; le SDK et l'exporteur
restent en `devDependencies` (complète Q72).

**Q77 — Un run sur un arbre de travail modifié** (8.9, relecture n° 1, I6) — ✅ répondu le
2026-10-21 : **marqué, hors référence**. Le run garde l'empreinte de l'arbre modifié ; il n'entre
jamais dans la référence de la branche principale, et ne confirme une régression que si
l'empreinte est la même.

**Q76 — Le temps affiché sur la carte** (8.7, relecture n° 1, I7) — ✅ répondu le 2026-10-21 :
**cumulé, propre en détail**. Comme la spec : le temps cumulé sur la boîte ; le temps propre
dans l'infobulle et le panneau de détail.

**Q75 — Ce que mesurent p50 et p95 d'un budget** (8.8) — ✅ répondu le 2026-10-21 : **chaque
appel entrant**. Chaque span du composant appelé depuis l'extérieur est un appel ; p50 et p95
sur leur durée dans un run, puis la médiane sur les 5 derniers runs avec traces ; « appels » les
compte ; « mémoire max » est le pic du processus pendant eux ; une flèche, les appels de la
source vers la cible ; sans attache, la durée des tests (ADR 0052).

**Q74 — Quand un budget dépassé apparaît sur la carte** (8.8) — ✅ répondu le 2026-10-21 :
**toujours**. La boîte porte une marque rouge avec une icône dès que la carte est ouverte ; le
détail chiffré est dans le calque Performances et via MCP ; rien tant que tout tient (ADR 0052).

**Q73 — Où vivent les vues de performances** (8.7) — ✅ répondu le 2026-10-21 : **calque et
panneau**. La carte annotée est un calque « Performances » de la carte (bouton et commande) ;
la timeline par test, l'arbre des appels et l'évolution par commit sont dans un panneau à part,
ouvert depuis la carte, le pont ou la palette (ADR 0051).

**Q72 — Jusqu'où `timonier init` touche le projet pour l'instrumentation** (8.6) — ✅ répondu
le 2026-10-21 : **manifestes et deux fichiers**. `init` ajoute le SDK au manifeste entre
marqueurs (Cargo.toml : dépendances optionnelles et feature `otel` ; package.json :
devDependencies), écrit un module d'initialisation et un test de fumée ; `--dry-run` montre le
diff, `uninit` retire tout ; `doctor` lance le test de fumée et vérifie qu'un span se rattache au
graphe (ADR 0050).

**Q71 — Le critère de sortie de la Phase 8** — ✅ répondu le 2026-10-21 : **validé tel quel**.

**Q70 — Les compteurs d'instructions refusés** (8.1) — ✅ répondu le 2026-10-21 : **repli et
doctor**. Sur Ubuntu (`perf_event_paranoid` = 4), `perf_event_open` est refusé sans réglage
système : la détection de régressions se fait alors sur le temps CPU, et le dit ; `timonier
doctor` explique la commande `sysctl` qui les active, sans la lancer.

**Q69 — Les IDs écrits à la main** (relecture n° 1 de la Phase 7, I2) — ✅ répondu le
2026-10-21 : **accepter en lecture**. Un `intent.json` écrit avec des IDs nommés comme l'exemple de
la spec (`C-parser`, `L-domain`) se lit et se dessine ; le tableau blanc continue de numéroter ce
qu'il crée (`C-1`) et garde les IDs existants (ADR 0039).

**Q68 — L'ancrage d'une boîte dessinée** (7.5) — ✅ répondu le 2026-10-21 : **nom souple, puis
emplacement**. Casse, espaces, tirets et soulignés ignorés ; un paquet dont le nom finit par le nom
dessiné compte aussi ; entre plusieurs candidats, le plus proche des composants déjà ancrés de la
même couche, sinon le plus haut ; sans correspondance, « manquant » (ADR 0041).

**Q67 — Ce que l'agent reçoit du dessin libre** (7.4) — ✅ répondu le 2026-10-21 : **une image
et des emplacements**. Le démon dessine le croquis en PNG (traits et formes), que l'agent reçoit
avec l'intention ; en texte, où est chaque trait (couche, composant proche), pour un agent sans
vision. Rien ne revient sur le tableau (ADR 0040).

**Q66 — Comment le dessin persiste** (7.3) — ✅ répondu le 2026-10-20 (« le dessin doit
forcément être persistant ») : **enregistré tout seul dans `intent.json`**, après chaque geste ;
l'agent lit toujours le dernier état, même un dessin en cours (ADR 0039).

**Q65 — Comment une note s'attache** (7.2) — ✅ répondu le 2026-10-20 : **posée dessus, ou
choisie**. Une note créée avec un élément sélectionné, ou posée sur une boîte ou une zone, s'y
attache ; ses propriétés permettent de changer l'élément, au clavier aussi (ADR 0038).

**Q64 — Ce que dit une flèche** (7.2) — ✅ répondu le 2026-10-20 : **une dépendance seulement**
(`depends_on`, tout lien du code compte) ; seule la politique se choisit.

**Q63 — Le rôle d'une boîte** (7.2) — ✅ répondu le 2026-10-20 : **texte libre**, une phrase sur
ce que fait le composant, comme l'exemple de la spec.

**Q62 — La couche d'une boîte** (7.2) — ✅ répondu le 2026-10-20 : **en étant dessinée dedans**.
La couche se déduit de la zone qui contient la boîte ; la déplacer hors de la zone l'en sort.

**Q61 — Critère de sortie de la Phase 7** (2026-10-20) — ✅ répondu le 2026-10-20 : **accepté
tel quel**.

**Q60 — Le remplaçant de tldraw** (7.0) — ✅ répondu le 2026-10-20 : **React Flow et
perfect-freehand** (ADR 0036). Excalidraw est écarté : polices chargées depuis un CDN, CSP à
ouvrir (worker, `'unsafe-eval'`), canevas non accessible, formes typées sans validation.

**Q59 — Quand publier l'extension** (2026-10-20) — ✅ répondu le 2026-10-20 : **à la fin du
MVP (Phase 9)**. Jusque-là, F5 et le `.vsix` local ; une tâche de la Phase 9 la porte.

**Q58 — Comment l'utilisateur obtient `timonier`** (2026-10-20) — ✅ répondu le 2026-10-20 :
**un `.vsix` par plateforme**. Chaque paquet (linux-x64, linux-arm64, darwin-x64, darwin-arm64)
embarque le binaire construit par la CI ; rien à installer à part. Les agents et les hooks le
trouvent par un chemin que l'extension expose, ou par une commande qui l'installe dans le
`PATH` (à fixer par ADR en Phase 9).

**Q57 — Le tableau blanc vide à l'initialisation** (relecture n° 2 de la Phase 6) — ✅ répondu
le 2026-10-20 : **rien avant la Phase 7**. `timonier init` ne crée pas `intent.json` tant que son
format n'existe pas (7.0) ; la Phase 7 ajoutera la création du tableau vide à `init`.

**Q56 — Un commentaire seul** (relecture n° 1 de la Phase 6, m1) — ✅ répondu le 2026-10-20 :
**la structure seule**. Seul ce qui change le graphe (un nœud, une arête) compte comme non suivi
et en dette de revue ; un commentaire ou une mise en forme, non (ADR 0029, 0030).

**Q55 — « Configuré »** (relecture n° 1 de la Phase 6, M3) — ✅ répondu le 2026-10-20 :
**`init.json`**. Un projet est configuré quand `timonier init` est passé (`.timonier/init.json`) ;
un `plan.json` écrit par un agent sans `init` ne suffit pas, il manquerait le bloc géré et les
hooks. Précise Q18.

**Q54 — Ce que `uninit` fait du suivi** (relecture n° 1 de la Phase 6, M2) — ✅ répondu le
2026-10-20 : **le garder, et le dire**. `uninit` retire la configuration (blocs, serveur MCP,
hooks, conventions, `init.json`…) mais garde `plan.json`, `todo.json`, `sessions/` et
`intent.json`, qui appartiennent au projet ; `--purge` les efface aussi, après confirmation au
terminal. L'arbre identique reste vrai quand rien n'a été suivi après `init`.

**Q53 — Comment VSCode lance `init`** (Phase 6, 6.12) — ✅ répondu le 2026-10-20 : **dans un
terminal VSCode**. Le bouton « Configurer » ouvre un terminal intégré qui lance
`timonier init` : la détection, les questions (Q51) et ce qui est fait sont visibles.
Révisé le 2026-10-21 par toi (T-118) : le bouton du pont ne passe plus par un terminal ; à côté
de « Configurer », une case par agent (ceux trouvés cochés) et `timonier init --yes --agents …`
en arrière-plan, sa sortie dans le journal de Timonier. La commande de palette garde le terminal.

**Q52 — Proposer de configurer un dépôt** (Phase 6, 6.12) — ✅ répondu le 2026-10-20 : **dans le
pont, avec un badge**. Pas de pop-up : le pont dit en haut « Ce projet n'est pas configuré pour
vos agents » avec un bouton « Configurer », l'icône Timonier porte un badge tant que ce n'est pas
fait, et une commande de palette « Timonier : Configurer le projet ».

**Q51 — Les questions de `timonier init`** (Phase 6, 6.8) — ✅ répondu le 2026-10-20 : **au
terminal, avec des options**. Ce que la détection ne tranche pas est demandé au terminal ; chaque
réponse a son option (`--agents`, `--yes`…) pour les scripts et l'extension VSCode (6.12).

**Q50 — Quand écrire `actual.json`** (Phase 6, 6.8) — ✅ répondu le 2026-10-20 : **un hook
pre-commit**. Avant le commit, les boîtes et dépendances de la tête sont écrites et ajoutées au
commit. Limite admise : il reflète l'arbre de travail, pas exactement l'index.

**Q49 — Le hook d'arrêt de l'agent** (Phase 6, 6.8) — ✅ répondu le 2026-10-20 : **bloquer une
fois, avec la raison**. Session ouverte dont la todo est vide ou a des éléments non cochés :
« mets la todo à jour ou appelle session_end ». Du code changé sans session : « ouvre une
session ». Une seule fois par arrêt : l'agent n'est jamais coincé.

**Q48 — Les agents que configure `timonier init`** (Phase 6, 6.8) — ✅ répondu le 2026-10-20 :
**Claude Code, Copilot, Cursor et Codex**, quand ils sont détectés. Gemini CLI plus tard.

**Q47 — Où vit le pont** (Phase 6, 6.7) — ✅ répondu le 2026-10-20 : **la vue d'ensemble, à
gauche**. Le pont remplace la vue d'ensemble de la barre latérale principale ; plan, todo et
sessions restent à droite pour le détail.

**Q46 — Un clic dans « Où regarder »** (Phase 6, 6.7) — ✅ répondu le 2026-10-20 : **ouvre la
carte dessus**. La carte s'ouvre ou se montre, déplie ce qu'il faut pour atteindre la boîte,
la centre et la sélectionne.

**Q45 — Classer « Où regarder »** (Phase 6, 6.7) — ✅ répondu le 2026-10-20 : **les alertes
puis la dette**. D'abord ce qui demande une décision (tâche bloquée, fichier de `.timonier/`
illisible, cycle ajouté), puis les boîtes les plus en dette de revue, celles touchées hors
session (non suivies) en tête ; cinq éléments au plus, le reste sur la carte.

**Q44 — Montrer la dette de revue** (Phase 6, 6.5) — ✅ répondu le 2026-10-20 : **un calque
à activer**. Un bouton « Dette de revue » dans la barre d'outils, et une commande. Activé,
chaque boîte prend un fond plus ou moins soutenu, en trois paliers du ton d'attention, avec
« 6 fichiers à relire » écrit dessus ; le non suivi a sa marque, des hachures et le mot
« non suivi » ; la légende explique les paliers. Hors calque, rien ne change.

**Q43 — Mesurer la dette de revue** (Phase 6, 6.5) — ✅ répondu le 2026-10-20 : **les fichiers
changés**. Pour chaque boîte, les fichiers changés pendant les sessions depuis la dernière
revue, rapportés à ses fichiers.

**Q42 — Comment finir la Phase 5** (relecture n° 7) — ✅ répondu le 2026-10-19 : **clore si
mineurs**. Après les corrections de la relecture n° 7, une relecture n° 8 ; si elle ne trouve
plus que des cas limites de regroupement ou des mineurs, ils sont notés comme écarts, et la
phase est close avec réserves une fois le test des cinq secondes passé, comme la Phase 4 (Q24).

**Q41 — Deux choix de l'ADR 0024 sur les déplacements** (Phase 5, relecture n° 7, m-2) — ✅
répondu le 2026-10-19 : **les garder**. Quand l'appelant et l'appelé bougent tous deux, chacun
compte pour son propre déplacement ; un fichier rangé dans un dossier nouveau d'une autre crate
emporte ses dépendances de crate. Les deux suivent Q36.

**Q40 — Un fichier renommé dans son dossier** (Phase 5, relecture n° 7, I-1) — ✅ répondu le
2026-10-19 : **pas un déplacement**. Un fichier resté dans son dossier ne bouge pas : s'il
change de composant (son nouveau nom rejoint celui de la majorité de ses voisins, ADR 0016),
c'est le fait du regroupement. Aucune dépendance ni aucun cycle ; le fichier et ses boîtes
restent « Modifiés ».

**Q39 — Un fichier regroupé au commit et modifié, qui appelle un autre fichier de la même
boîte** (Phase 5, relecture n° 5, m-2) — ✅ répondu le 2026-10-19 : **le noter comme écart**.
Le cas est rare (un regroupement au commit, et une modification qui change d'appelé à
l'intérieur de la même boîte) ; le résumé peut alors lister une dépendance retirée et une
ajoutée pour le même code. Pas de comparaison fichier par fichier de plus.

**Q38 — Les boîtes « dont aucune n'est un regroupement » de Q36** (Phase 5, relecture n° 5,
m-2) — ✅ répondu le 2026-10-19 : **les boîtes du fichier**. Un déplacement compte si ni la
boîte que le fichier quitte, ni celle qu'il rejoint, n'est un regroupement. Un fichier déplacé
de x vers un dossier nouveau w (un regroupement) ne fait donc ni « x → z retirée » ni
« w → z ajoutée » ; le seul fichier de x déplacé dans y, x disparaissant, ne fait pas
« y → z ajoutée ».

**Q37 — La crate qui cède ou reçoit les fichiers d'une crate qui arrive ou part** (Phase 5,
relecture n° 4, m-1) — ✅ répondu le 2026-10-19 : **oui, comme un déplacement**. Comme un
fichier déplacé marque ses deux boîtes, les fichiers d'une crate qui arrive ou part marquent la
boîte qu'ils quittent ou rejoignent : la crate racine, par exemple, est « Modifiée ».

**Q36 — Ce qui explique une dépendance** (Phase 5, relecture n° 4, I-1 et I-2) — ✅ répondu le
2026-10-19 : **comparer les arêtes**. Une dépendance n'est ajoutée ou retirée que si une arête
qui la porte (appel, import, dépendance d'un manifeste) apparaît ou disparaît dans un fichier
changé (Q28), ou si un fichier passe d'une boîte à une autre dont aucune n'est un regroupement.
Un regroupement n'ajoute ni ne retire jamais de dépendance, même si un fichier a changé
ailleurs. Les lignes « Nouveau » de la carte suivent la même règle. Précise Q28, lu jusque-là
fichier par fichier.

**Q35 — Une crate qui apparaît ou disparaît avec son manifeste** (Phase 5, relecture n° 3,
I-b) — ✅ répondu le 2026-10-19 : **compter le manifeste**. Une crate n'est pas un
regroupement automatique : elle vient de son manifeste, qui est du code. Elle est « Nouvelle »
quand son manifeste arrive, « Supprimée » (dans le résumé, Q26) quand il part, sauf
renommage. Les boîtes qui la contiennent sont modifiées, comme pour un fichier. Les composants
gardent la règle de Q33 ; Q34 reste (une crate devenue externe est « Modifiée »).

**Q34 — Une crate qui perd son manifeste** (Phase 5, relecture n° 2, m-4) — ✅ répondu le
2026-10-18 : **« Modifiée »**. Un paquet qui perd tous ses fichiers ou en gagne (il passe
d'interne à externe, ou l'inverse) prend le badge « Modifié » et entre dans le résumé. Une
dépendance retirée est nommée comme à la revue.

**Q33 — Une boîte qui apparaît ou disparaît sans que son code arrive ou parte** (Phase 5,
relecture n° 2, I-1) — ✅ répondu le 2026-10-18 : **seulement si du code part ou arrive**. Un
composant est toujours un regroupement automatique, refait au commit (ADR 0016). Une boîte
n'est donc « Nouvelle » que si elle apporte un fichier nouveau, ou si elle n'en a aucun (un
paquet externe). Elle n'est « Supprimée » que si un de ses fichiers a été supprimé, ou si elle
n'en avait aucun. Sinon, elle est « Modifiée » si elle tient un fichier changé ou déplacé. Un
dossier renommé avant le commit ne laisse donc aucune boîte supprimée. Limite acceptée : un
dossier vidé dans un autre n'est pas listé comme supprimé.

**Q32 — Densité réglable de la carte : quand** (Phase 5, relecture n° 1, I4) — ✅ répondu le
2026-10-18 : **maintenant, en Phase 5**. Un réglage « confortable » ou « compacte » change la
taille des boîtes et les espacements ; la mise en page suit.

**Q31 — Le filtre des changements : estomper ou masquer** (Phase 5, relecture n° 1, m7) — ✅
répondu le 2026-10-18 : **estomper**. Rien ne bouge, l'inchangé reste visible en fond ; le
libellé devient « Mettre en avant les changements ».

**Q30 — Composants qui apparaissent ou disparaissent par regroupement automatique** (Phase 5,
relecture n° 1, B1) — ✅ répondu le 2026-10-18 : **les ignorer**. Comme pour Q28, le diff ne
montre que ce qu'un changement de code explique : un composant qui ne fait que regrouper des
fichiers inchangés n'a ni badge ni ligne dans le résumé.

**Q29 — Critère de sortie de la Phase 5** — ✅ répondu le 2026-10-18 : **validé tel quel**.

**Q28 — Dépendance changée par la seule vérification du serveur de langage** (Phase 5) — ✅
répondu le 2026-10-18 : **l'ignorer**. Le diff de revue ne montre que ce qu'un changement de
code explique : une dépendance ajoutée ou retirée compte si un des fichiers qui la portent a
changé depuis la revue. La carte montre toujours le graphe le plus précis.

**Q27 — Avant la première revue** (Phase 5) — ✅ répondu le 2026-10-18 : **pas de diff**. Tant
que rien n'a été marqué comme revu, la carte se montre telle quelle, et un message invite à
« Marquer comme revu » pour suivre les changements à partir de là.

**Q26 — Où montrer ce qui a disparu depuis la dernière revue** (Phase 5) — ✅ répondu le
2026-10-18 : **dans le résumé seul**. La carte ne montre que ce qui existe, avec ses badges ; les
composants et dépendances supprimés sont listés dans le résumé du diff.

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

- 0.1 Lire `docs/spec.md` en entier — critère : contradictions et manques relevés dans
  « Questions ouvertes ».
- 0.2 Vérifier dans la documentation officielle le format actuel des skills, hooks,
  subagents et `settings.json` — critère : sources et date citées dans l'ADR 0001.
- 0.3 Vérifier existence, version et licence des paquets nommés par la spec — critère :
  tableau « Dépendances vérifiées » plus bas.
- 0.4 Initialiser le dépôt git (branche `main`) et écrire `.gitignore` — critère :
  `.claude/state/` et `.claude/settings.local.json` sont ignorés.
- 0.5 Écrire `CLAUDE.md` — critère : vision, carte du dépôt, commandes, règles, pointeurs,
  hooks documentés avec leur test manuel.
- 0.6 Écrire les sept skills (`rust-daemon`, `graph-model`, `language-pack`,
  `vscode-webview-ui`, `mcp-tools`, `otel-instrumentation`, `adr`) — critère : frontmatter
  conforme à la doc, avec une description qui dit quand les utiliser.
- 0.7 Écrire les hooks (`.claude/settings.json`, `.claude/hooks/`) — critère :
  `node --test .claude/hooks/hooks.test.mjs` vert, et chaque script exécuté à la main avec
  une entrée réelle.
- 0.8 Écrire le subagent `phase-reviewer` — critère : outils en lecture seule (`Read`,
  `Grep`, `Glob`, `Bash`), rapport en français.
- 0.9 Écrire les ADR 0001 et 0002, l'index `docs/adr/README.md` et le modèle d'ADR —
  critère : fichiers présents, index à jour.
- 0.10 Rédiger et soumettre `PLAN.md` — critère : chaque tâche a un critère vérifiable.
- 0.11 Vérification par l'humain dans une nouvelle session (hooks, skills, subagent,
  CLAUDE.md) — critère de sortie du prompt :
  - le message de démarrage « Timonier · Phase … » s'affiche ;
  - `/hooks` liste SessionStart, UserPromptSubmit, PreToolUse, PostToolUse et Stop ;
  - `/skills` liste les sept skills ;
  - `/agents` liste `phase-reviewer` ;
  - `CLAUDE.md` suffit à reprendre le travail.
- 0.12 Validation de ce plan et réponses à Q1–Q7 — critère : réponses consignées ici. Feu
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

- 1.1 Ajouter la licence MIT (« Copyright (c) 2026 Nicolas Michel ») — critère : fichier
  `LICENSE` présent ; `license = "MIT"` dans `Cargo.toml` et `package.json` ; `cargo metadata`
  et `npm pkg get license` renvoient `MIT`.
- 1.2 Fixer la toolchain : `rust-toolchain.toml` (stable, rustfmt, clippy), édition 2024 et
  `rust-version` dans `[workspace.package]`, `rustfmt.toml` — critère : `cargo fmt --all
  --check` vert ; ADR toolchain et MSRV (stable actuelle 1.99, 1.96 installée en local).
- 1.3 Créer le workspace Cargo avec les sept crates (`timonier-core`, `-store`, `-index`,
  `-daemon`, `-otlp`, `-mcp`, `-cli`) — critère : `cargo build --workspace` vert ; ADR sur
  le découpage, le sens des dépendances et le crate qui porte les types du protocole.
- 1.4 Binaire `timonier` et alias `tim` (clap 4.6), avec `--version` et `--help` — critère :
  `timonier --version` et `tim --version` impriment la même chaîne (test assert_cmd) ; ADR
  sur le mécanisme d'alias.
- 1.5 Lints du workspace (`[workspace.lints]`) — critère : `cargo clippy --workspace
  --all-targets -- -D warnings` vert.
- 1.6 Un test de fumée par crate — critère : `cargo test --workspace` vert.
- 1.7 Monorepo Node — critère : `npm ci && npm run lint && npm run typecheck && npm test`
  verts ; ADR gestionnaire de paquets et bundler de l'extension (H12). Contenu :
  - `package.json` racine, workspaces `editors/vscode` et `webview` ;
  - scripts `build`, `test`, `lint`, `typecheck`, `format` ;
  - TypeScript 6.0.x strict (H11), ESLint en configuration plate avec typescript-eslint,
    prettier.
- 1.8 `webview/` : application React 19 + Vite 8 qui affiche un état vide (tokens `--tim-*`
  minimaux sur les variables VSCode) — critère : `npm run build -w webview` produit le
  bundle ; test vitest vert.
- 1.9 `editors/vscode/` : extension avec la commande « Timonier : Ouvrir la carte », qui
  ouvre un panneau webview chargeant le bundle de `webview/` sous une CSP stricte (nonce,
  `localResourceRoots`) — critère : F5 (« Run Extension ») ouvre le panneau vide en thème
  clair et sombre ; un test d'intégration `@vscode/test-cli` exécute la commande et vérifie
  la création du panneau.
- 1.10 Packaging — critère : `npx @vscode/vsce package` produit un `.vsix` que
  `code --install-extension` installe.
- 1.11 Activer les hooks sur le squelette — critère : test manuel documenté dans
  `CLAUDE.md` (une erreur de type introduite bloque la fin de tour une fois). Changements :
  - le hook Stop appelle le script `typecheck` racine ;
  - le hook de formatage trouve prettier local.
- 1.12 CI GitHub Actions sur ubuntu-latest et macos-latest — critère : chaque étape du
  workflow passe en local sous Linux. Le vert sur les deux OS sera constaté au premier push
  sur le dépôt public créé par toi (Q2). Étapes :
  - Rust : vérification du format, clippy `-D warnings`, `cargo test` ;
  - Node : `npm ci`, lint, typecheck, tests, build, package ;
  - tests de l'extension sous xvfb (Linux) ;
  - tests des hooks ;
  - caches cargo et npm.
- 1.13 `README.md` minimal en français (ce qu'est Timonier, état, comment construire) et
  commandes de `CLAUDE.md` à jour — critère : relu.
- 1.15 Icône Timonier dans la barre d'activité, ajoutée à ta demande du 2026-10-14 —
  critère : un conteneur Timonier avec une icône 24×24 d'une seule couleur, une vue « Vue
  d'ensemble » dont l'accueil propose « Ouvrir la carte », et un bouton de carte dans sa barre
  de titre ; tests unitaires du manifeste et test d'intégration verts ; ADR 0007.
  - Constat : 19 tests unitaires et 5 tests d'intégration verts.
  - L'icône a été rendue pixel par pixel à 24 px, et la version à 6 rayons retenue.
  - Le rendu réel est à regarder avec F5, en même temps que 1.9.
- 1.14 Relecture par `phase-reviewer` — critère : verdict « prête », ou réserves traitées.
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
- L'éditeur de la Marketplace (`publisher`) était provisoire : `nicolas-a-michel` ; c'est
  `NicolasMichel` depuis le 2026-10-20 (Q12).
- Les tests de fumée des crates sans code (store, index, otlp, daemon, mcp) ne prouvent que la
  compilation et le lien avec core. Ils seront remplacés par de vrais tests dans les phases
  suivantes.

## Phase 2 — Modèle de graphe et stockage

Statut : terminée (validée le 2026-10-16)

- 2.1 Écrire les ADR — critère : ADR écrites (statut « Proposé ») avant le code qui en
  dépend, puis acceptées quand tu valides la phase (procédure du skill `adr`). Sujets :
  - place des composants : consigne la réponse à Q4, option (a) ;
  - classement de fiabilité des sources pour la fusion ;
  - sémantique des snapshots `working_tree` ;
  - outil de migration (`rusqlite_migration` 2.6 ou `refinery` 0.10) ;
  - pragmas SQLite.
- 2.2 `timonier-core` : les types `Node`, `NodeKind`, `SymbolKind`, `Edge`, `EdgeKind`,
  `Provenance`, la confiance, le poids et le span, sérialisables (serde) — critère :
  round-trip JSON testé pour chaque type.
- 2.3 Identité stable : symbole SCIP ou `lang:qualified_name` — critère : tests unitaires
  et proptest (`parse(format(id)) == id`).
- 2.4 Invariant d'arbre strict : un seul parent, pas de cycle — critère : un proptest
  génère arbres et reparentages, et toute violation est refusée par une erreur typée.
- 2.5 Règles de fusion multi-sources — critère : tests en table :
  - une arête vue par TreeSitter et Lsp garde la provenance Lsp ;
  - une arête TreeSitter contredite par le Lsp est supprimée ;
  - `Intended` n'est jamais fusionnée.
- 2.6 Détection des déplacements par signature et hash du corps normalisé, avec un seuil
  fixé par ADR — critère : un fichier renommé, une fonction déplacée entre deux fichiers et
  une fonction renommée à corps égal donnent « déplacé », jamais « supprimé + ajouté ».
- 2.7 `timonier-store` — critère : migrations testées sur base vide ; test de lectures
  concurrentes pendant les écritures. Contenu :
  - schéma SQLite : nœuds, arêtes avec `valid_from` / `valid_to`, `snapshot(seq, commit_sha,
    timestamp, kind)`, état par fichier (hash, « vérifié par LSP au hash X ») ;
  - migrations ;
  - WAL, écrivain unique sur un thread dédié, lecteurs en lecture seule.
- 2.8 Mise à jour par différence d'un fichier — critère : réappliquer un fichier identique
  modifie 0 ligne ; modifier un symbole ferme un intervalle et en ouvre un.
- 2.9 Snapshots `commit`, `review_mark` et `working_tree` — critère : création, liste et
  cohérence des intervalles testées.
- 2.10 Requête de diff entre deux snapshots : ajouts, suppressions, modifications,
  déplacements — critère : tests sur fixtures ; un déplacement de fichier est rapporté comme
  déplacement.
- 2.11 Arêtes entre composants agrégées par requête depuis les arêtes de symboles, jamais
  stockées à la main — critère : test du poids agrégé attendu.
- 2.12 Mesure de référence : insertion et diff sur un graphe synthétique — critère :
  chiffres consignés dans ce plan.
- 2.13 Relecture par `phase-reviewer` — critère : verdict « prête ». Obtenu à la troisième
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

- 3.1 ADR sur le format des packs de langage : manifeste versionné, requêtes `.scm`,
  heuristiques, emplacement — critère : ADR écrite (« Proposé »), acceptée quand tu valides la
  phase (Q15) ; skill `language-pack` mis à jour.
  - [ADR 0012](docs/adr/0012-format-des-packs-de-langage.md) : packs dans
    `crates/timonier-index/packs/<langue>/`, incorporés au binaire ; contrat des captures
    vérifié au chargement.
  - Restent du code : la table des grammaires (une ligne par grammaire) et les écosystèmes
    `cargo` et `npm`.
- 3.2 Packs Rust et TypeScript : grammaires (tree-sitter 0.27.1, tree-sitter-rust 0.24.2,
  tree-sitter-typescript 0.23.2), requêtes de définitions, d'imports et
  d'appels/références — critère : snapshots insta des nœuds et arêtes extraits de fixtures
  Rust et TS.
  - Trois instantanés relus ligne à ligne (Rust, TS, TSX), dans
    `crates/timonier-index/tests/snapshots/`.
  - La résolution des chemins en arêtes est testée à part
    (`crates/timonier-index/src/project.rs`), dont les réexportations Rust et TS.
- 3.3 Noms qualifiés stables et hash de corps normalisé, par langage — critère : reformater
  un fichier ne change aucune identité (test).
  - Testé en Rust et en TypeScript : un fichier reformaté garde chaque nom qualifié, chaque
    signature et chaque hash de corps.
  - Réordonner les définitions ne change aucune identité (test).
  - Un déplacement de fonction entre fichiers et un renommage de fichier donnent « déplacé »
    dans le diff (test de bout en bout, `crates/timonier-daemon/tests/indexing.rs`).
- 3.4 Indexation initiale parallélisée, avec un cache persistant (hash par fichier) —
  critère : une deuxième indexation sans changement ne reparse aucun fichier (compteur).
  - Cache dans SQLite (migration 0002, [ADR 0013](docs/adr/0013-extraction-resolution-et-cache.md)).
  - Seconde indexation : 0 fichier reparsé ; au redémarrage sur la même base, 0 ligne écrite
    (tests).
- 3.5 Vitesse 1, à la sauvegarde — critère : latence mesurée selon H2. Contenu :
  - watcher (notify 8.2 + notify-debouncer-full 0.7), anti-rebond ~300 ms ;
  - respect de `.gitignore` (crate `ignore`) ;
  - hash du fichier, reparse tree-sitter incrémental, mise à jour par différence.

  Constat (2026-10-16, build release, Ryzen 9 5950X), sur une copie du dépôt Timonier
  (71 fichiers) et 200 sauvegardes : chiffres de 3.11 (p50 2,5 ms, p95 10,4 ms, max 24 ms) ;
  - la mesure va du début de la passe (fin de l'anti-rebond) au commit SQLite ;
  - outil : `cargo run --release -p timonier-daemon --example index_bench`.

  Le watcher réel est testé de bout en bout : sauvegarde, anti-rebond, graphe mis à jour.
- 3.6 Vitesse 2, en arrière-plan, selon Q5 — critère : test avec un serveur LSP réel ou un
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
- 3.7 Vitesse 3, au commit : détection du commit, re-clustering complet initialisé par la
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
- 3.8 Composants — critère : stabilité mesurée (une petite modification change moins de
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
- 3.9 Rôles architecturaux et dépendances externes — critère : sur Timonier, les rôles
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
- 3.10 Compaction au-delà d'environ 50 000 arêtes de symboles — critère : test de charge
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
- 3.11 Mesures sur le dépôt Timonier — critère : indexation à froid ≤ 3 s (H3) et mise à
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
- 3.12 Relecture par `phase-reviewer` — critère : verdict « prête ». **Non atteint.**
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

- 4.1 ADR sur le protocole — critère : ADR écrite (« Proposé »), acceptée à la validation
  de la phase (Q15). [ADR 0019](docs/adr/0019-protocole-demon-extension.md). Sujets :
  - socket local, un démon par projet avec verrou, découverte ;
  - méthodes et versionnement ;
  - génération des types partagés Rust → TypeScript ;
  - jsonrpsee 0.26 ou implémentation dédiée.
- 4.2 Serveur JSON-RPC 2.0 sur socket Unix : `initialize`, abonnement et désabonnement par
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
- 4.3 CLI `timonier daemon start|stop|status` — critère : tests assert_cmd.
  - Plus `run`, le démon au premier plan, que `start` et l'extension lancent.
  - Comportement (projet trouvé en remontant, journal, codes de sortie) : ADR 0019, « CLI ».
  - Tests (`crates/timonier-cli/tests/daemon.rs`) :
    - `status` sans démon sort avec le code 3, et `stop` sans démon n'est pas une erreur ;
    - `start`, un second `start`, `status`, le journal, `stop`, puis plus de socket ;
    - le projet est trouvé depuis un sous-dossier ;
    - `run` au premier plan : un second `run` le voit, et SIGTERM l'arrête proprement.
- 4.4 Extension : lancement et supervision du démon (redémarrage avec backoff, canal de
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
- 4.5 Relais du LSP de l'IDE vers le démon, si Q5 le retient — critère : définitions et
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
- 4.6 Système de design minimal, avant la première vue — critère : contraste AA vérifié
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
- 4.7 Carte (React Flow 12.12 + elkjs 0.12) — critère : sur Timonier, la vue d'ouverture
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
- 4.8 Mise en page stable, avec les positions précédentes comme contraintes d'elkjs —
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
- 4.9 Navigation — critère : test d'intégration de l'ouverture du code ; checklist
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
- 4.10 Rendu par niveau de détail, avec agrégation au-delà d'un seuil de nœuds — critère :
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
- 4.11 Test des cinq secondes avec toi — critère : réussi et consigné.
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
- 4.12 Relecture par `phase-reviewer` — critère : verdict « prête ». **Atteint avec
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
  snapshots. Corrigé en Phase 5 (ADR 0025).
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

Statut : terminée (validée par toi le 2026-10-20 ; 5.5 close avec réserves, Q42)

- 5.1 Commande et bouton « Marquer comme revu », qui crée un snapshot `review_mark` (méthode
  JSON-RPC) avec une confirmation discrète — critère : test d'intégration.
  - Méthode `review/mark` (protocole version 3), passée par le fil du pipeline entre deux
    étapes ; une base en retard est rattrapée d'abord, et le disque relu (une sauvegarde faite
    juste avant le clic est dans la revue) ; `daemon/status` donne la dernière revue. Le gel
    garde l'empreinte de chaque fichier (migration 0005, ADR 0024).
  - Commande « Timonier : Marquer comme revu », bouton dans le résumé de la carte ;
    confirmation dans la barre d'état pendant 4 s.
  - Tests : `a_client_marks_the_head_as_reviewed` (vrai démon),
    `a_review_mark_catches_the_store_up_first`, `a_review_mark_keeps_the_content_of_each_file`,
    et l'intégration « marks the head as reviewed, from the palette » dans un vrai VSCode.
- 5.2 Diff structurel depuis la dernière `review_mark` — critère : tests sur des paires de
  snapshots fixtures. Les arêtes pendantes, que les lectures brutes renvoient, sont filtrées
  (ADR 0009). Le diff montre :
  - composants ajoutés et supprimés ;
  - dépendances ajoutées et supprimées ;
  - cycles ajoutés (composantes fortement connexes du graphe des composants) ;
  - composants dont le fan-in ou le fan-out a bondi (seuil fixé par ADR) ;
  - un badge « modifié » pour un refactoring interne, où les déplacements comptent comme des
    modifications.
  - `timonier_core::review` (pur) et `timonier_core::graph`, partagé avec les vues. Règles et
    seuil dans l'ADR 0024 : boîtes = composants et paquets ; dépendances entre boîtes sœurs ;
    Q28 (une dépendance ne compte que si un fichier qui la porte a changé) ; Q30 (un
    regroupement de fichiers inchangés n'est rien) ; fichiers déplacés et boîtes renommées
    retrouvés sous leur nouvel identifiant ; bond = au moins +3 et +50 %.
  - 47 tests sur des paires de snapshots fixtures (`review.rs`). Mutations : 14 au premier
    passage, puis 8 sur les renommages et Q30, puis 12 sur Q33, Q34 et les symboles déplacés
    (relecture n° 2 ; une équivalente retirée du code), puis 16 sur Q35, les boîtes regroupées,
    les cycles et les bonds (relecture n° 3), puis 21 sur la comparaison des arêtes, Q37 et les
    paquets (relecture n° 4 ; dont 10 reprises des passages d'avant sur le code réécrit), puis
    18 sur Q37 et Q38, les dépendances qui suivent une crate et les cycles (relecture n° 5),
    puis 11 sur le placement des arêtes aux deux bouts (relecture n° 6 ; et les jeux d'avant
    qui s'appliquent encore, relancés), puis 3 sur Q40 et les deux survivantes de la
    relecture n° 7, puis 3 sur les trous de tests de la relecture n° 8. Relancées toutes
    ensemble sur le code final (2026-10-19) : 62 tuées, aucune survivante ; 39 autres visaient
    du code réécrit depuis. Trois mutations du premier passage sont équivalentes (relecture
    n° 1) : regrouper les cycles par le seul parent de la source, retirer le test
    d'appartenance au graphe des cycles, compter une arête dont un bout contient l'autre. Le fil des vues garde la revue en mémoire et
    calcule le diff à chaque lecture de la tête ; les vues portent les marques et le résumé
    (protocole version 3). Test de bout en bout sur un vrai démon :
    `a_view_shows_what_changed_since_the_review` (une dépendance entre deux crates et un
    module ajoutés après la revue).
  - Coût, mesuré le 2026-10-19 après la relecture n° 6 (`view_bench`, build release ; d'une
    passe à l'autre, les chiffres varient d'un tiers). Deux cas extrêmes : la tête comparée à
    elle-même, tous fichiers changés ; puis une revue où chaque composant et chaque fichier a
    un autre identifiant (tout renommé). Les espaces de travail synthétiques sont générés par
    le bench lui-même (`synthetic:CxDxF` : C crates de D dossiers de F fichiers, appels dans
    chaque crate et vers la précédente, `examples/support/synthetic.rs`) :
    - Timonier (3 694 nœuds) : 2,9 à 3,4 ms tous fichiers changés, 7,7 à 8,8 ms tout
      renommé ; vue entièrement ouverte avec le diff en 2,6 à 3,3 ms ;
    - `synthetic:50x8x5` (2 450 fichiers, 6 958 nœuds) : 14 à 15 ms et 25 à 28 ms (84,4 ms
      avec l'appariement quadratique d'avant la relecture n° 2) ; vue entièrement ouverte en
      8 à 10 ms au lieu de 3,4 ms ;
    - `synthetic:125x8x5` (6 125 fichiers, 17 390 nœuds, environ 1 140 boîtes) : 38 à 50 ms et
      78 à 122 ms ; vue entièrement ouverte en 30 à 44 ms au lieu de 11 à 14 ms ;
    - graphe de la revue gardé en mémoire, mesuré dans un processus neuf lancé par le bench
      (croissance de la mémoire résidente pendant le chargement, transitoire compris : au
      plus) : 6,3 Mio pour Timonier, 7,6 Mio pour 6 958 nœuds, 18,7 Mio pour 17 390. L'écart
      mesuré avant dans le même processus (1,6 à 2 Mio) ne valait rien : la mémoire libérée
      par les chargements d'avant était réutilisée (relecture n° 6, m-3).
    - Les chiffres d'avant, sur un dépôt synthétique de 12 407 nœuds dont le générateur n'a pas
      été gardé (19,3 ms pour le diff, 4,4 Mio), sont remplacés par ceux-ci (relecture n° 4,
      m-2).
- 5.3 Rendu sur la carte : badges avec icône et texte (jamais la couleur seule), filtre
  « seulement les changements », résumé — critère : checklist d'accessibilité ; test des
  cinq secondes.
  - Fait :
    - un badge par boîte, dans sa barre de titre : « Nouveau », « Modifié », « Cycle »,
      « Fan-in 4 → 7 », chacun avec sa forme, au ton d'attention ;
    - un rôle fermé dit « N modifiés » et marque chacun de ses membres changés ;
    - une ligne nouvelle ou dans un cycle ajouté prend la couleur d'attention et une marque
      écrite à mi-chemin (pas d'épaisseur : elle est pour le trafic) ;
    - le résumé, en bandeau au-dessus de la carte, donne la revue de départ, les comptes, les listes à la
      demande (suppressions comprises, Q26 ; chaque boîte nommée avec celle qui la contient), et
      les boutons « Mettre en avant les changements » et « Marquer comme revu » ; avant la
      première revue, il dit quoi faire (Q27) ;
    - le filtre estompe ce qui n'a pas changé, sans rien déplacer (Q31) ; il est gardé avec
      l'état de la page, et sans effet avant la première revue ;
    - commandes de palette pour le filtre et pour les détails du résumé ;
    - la vue s'ajuste autour des panneaux ; sur une carte étroite (600 px), la légende ne garde
      que son titre et le résumé replié ses comptes ;
    - densité réglable (Q32, [ADR 0026](docs/adr/0026-densite-de-la-carte.md)) : réglage
      `timonier.map.density`, confortable ou compacte.
  - Checklist d'accessibilité (`webview/test/a11y/review.a11y.ts`, 14 tests) :
    - [x] contraste AA dans les six thèmes capturés : texte à 4,5:1, formes des badges et
      lignes à 3:1 ; filtre actif, le texte et les lignes qui restent en avant aussi (les lignes
      estompées reculent, et ne sont plus mesurées) ; carte compacte comprise
      (`density.a11y.ts`, 7 tests) ;
    - [x] chaque action au clavier et en commande (« Marquer comme revu », filtre, détails) ;
    - [x] focus visible, liste du résumé atteignable au clavier (axe l'a relevé, corrigé) ;
    - [x] jamais la couleur seule : une forme et un mot par changement, une coche sur le filtre
      actif ;
    - [x] noms accessibles : boîtes, rôles et lignes disent ce qui a changé ;
    - [x] aucune animation ajoutée.
  - Reste : le test des cinq secondes avec toi. Protocole :
    - écran : la page de démonstration `?map=timonier&review=1`, en français, thème Dark 2026,
      à l'ouverture (rôles fermés), en capture d'écran ; ses données de revue sont faites à la
      main (dont une crate supprimée, `timonier-legacy`, qui n'existe pas) ;
    - cinq secondes, puis deux questions : « que montre cet écran ? », « que ferais-tu
      ensuite ? » ;
    - réussi si la réponse dit que l'écran montre ce qui a changé depuis la dernière revue, et
      que l'action suivante est de regarder ce qui a changé (une boîte marquée, les détails du
      résumé) ou de marquer comme revu.
  - Test des cinq secondes n° 1 (2026-10-20, toi, capture `?map=timonier&review=1`, Dark 2026,
    français) : **échoué**. Réponse : « on voit l'architecture du logiciel, et on peut
    visiblement éditer ou ajouter des choses ». L'architecture est vue, mais pas « ce qui a
    changé depuis la revue ». Les marques sont lues comme des actions : le crayon de
    « Modifié » se lit « éditer », le plus de « Nouveau » se lit « ajouter ». Le résumé, en
    bas à droite, n'est pas vu. Correction à faire, puis un nouveau test avec quelqu'un qui
    n'a pas vu l'écran.
  - Corrigé (2026-10-20, avec ton accord) :
    - « Modifié » est un point plein, « Nouveau » une étoile pleine : plus de crayon ni de
      « + » ; un rôle fermé dit « 2 modifiés » au lieu d'un chiffre seul (d'abord « 2 à
      revoir », remplacé à ta demande) ;
    - le résumé est un bandeau en haut, au-dessus de la carte, sur toute sa largeur : « Depuis
      votre revue du … : » suivi des comptes ; les actions à droite, plus grandes, « Marquer
      comme revu » en bouton principal (couleurs des boutons principaux de VSCode) ;
    - la légende explique les marques (« Changements depuis la revue ») dès qu'il y a une
      revue ;
    - les cartes de rôle s'élargissent (300 px, 268 px en compact) pour garder leur nom
      entier à côté de « N modifiés » ;
    - tests : `review.a11y.ts` (le bandeau au-dessus de la carte, la légende, « modifié ») ;
      61 tests d'accessibilité passent dans les six thèmes ;
    - nouvelle capture : `/mnt/travail/timonier-revue-phase5.png` (hors du dépôt).
  - Validé par toi le 2026-10-20 sur la capture corrigée (« c'est mieux comme ça ») : le test
    des cinq secondes n° 2, avec quelqu'un qui n'a pas vu l'écran, n'a pas été fait. À refaire
    à la première occasion ; noté en réserve.
- 5.4 Snapshots de commit exacts (Q23) : le gel prend aussi les manifestes du commit et la
  présence de `src/lib.rs` — critère : tests d'un manifeste modifié sans commit (crate
  renommée, dépendance ajoutée) et d'un `src/lib.rs` présent d'un seul côté, dont le snapshot
  décrit le commit.
  - [ADR 0025](docs/adr/0025-paquets-du-commit.md) : les paquets du commit, découverts sur une
    superposition de ses manifestes et de ses bibliothèques ; tous les fichiers replacés le
    temps du gel, leur vérification LSP retirée puis rendue.
  - Tests, chacun comparant le snapshot à une indexation neuve d'un clone du commit, et la tête
    à une indexation neuve de l'arbre de travail : crate renommée, dépendance ajoutée,
    `src/lib.rs` supprimé puis créé d'un seul côté, crate ajoutée et paquet supprimé depuis le
    commit ; et `a_commit_with_other_packages_keeps_the_verification` (la vérification revient
    entière, rien n'est redemandé). 7 mutations tuées, une équivalente retirée du code.
  - Après la relecture n° 1 : `a_commit_with_other_packages_gives_the_components_back` (les
    fichiers retrouvent leurs composants sous le paquet de l'arbre de travail) et
    `a_commit_snapshot_ignores_a_hidden_manifest`. Coût mesuré le 2026-10-19 (`commit_bench`,
    build release, `synthetic:50x8x5`, 2 450 fichiers) : 697 ms pour un commit sous d'autres
    paquets, contre 216 ms aux mêmes paquets (ADR 0025).
  - Après la relecture n° 2 : `a_commit_snapshot_ignores_a_manifest_the_walk_skips` (un
    manifeste suivi dans un dossier ignoré ne fait pas d'autres paquets : la vérification LSP
    du commit reste).
- 5.5 Relecture par `phase-reviewer` — critère : verdict « prête ». Atteint avec réserves
  (Q42) : la relecture n° 8 ne trouve ni bloquant ni important, et conclut « prête avec
  réserves » une fois le test des cinq secondes passé ; ses mineurs sont corrigés ou notés en
  écarts.
  - Relecture n° 1 (2026-10-18) : « pas prête », 1 bloquant, 4 importants, 12 mineurs. Tes
    réponses : Q30 (regroupements ignorés), Q31 (le filtre estompe), Q32 (densité maintenant).
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - B1 : un composant né d'un regroupement de fichiers inchangés n'est plus « Nouveau »
      (Q30) ; test `a_regrouping_of_unchanged_files_is_no_change` ;
    - I1 : un dossier renommé est « modifié », avec ses liens (fichiers déplacés retrouvés par
      leur contenu, boîtes par la majorité de leurs fichiers) ; un paquet renommé aussi ; tests
      `a_folder_renamed_is_modified_with_its_links`, `a_package_renamed_is_modified_and_keeps_its_dependencies` ;
    - I2 : le filtre est sans effet avant la première revue ; test Playwright ;
    - I3 : les marques de cycle et les listes du résumé sont tenues côté démon ; test
      `a_view_shows_the_cycles_the_jumps_and_what_went` ;
    - I4 : densité réglable faite (Q32) ;
    - m1 : tests du bond d'une boîte nouvelle, d'un paquet externe, d'une boîte nouvelle qui
      en absorbe une disparue ; les mutations restantes sont équivalentes (listées en 5.2) ;
    - m2 : composants rendus au retour d'un commit sous d'autres paquets, manifeste caché ;
    - m3, m4, m5 : ADR 0024, 0025 et PLAN.md alignés sur le code ;
    - m6 : panneaux à 600 px ; test Playwright ;
    - m7 : libellé « Mettre en avant les changements » (Q31) ;
    - m8 : boîtes du résumé nommées avec celle qui les contient ;
    - m9 : commande de palette pour les détails du résumé ;
    - m10 : la revue relit le disque ; test `a_review_mark_takes_what_the_disk_holds` ;
    - m11 : la tête, la revue et les empreintes lues dans une même transaction ; test
      `reads_in_one_transaction_see_one_state` ;
    - m12 : mémoire mesurée (mesure refaite après la relecture n° 6, dans un processus neuf :
      au plus 7,6 Mio pour une revue de 6 958 nœuds, 5.2).
  - Relecture n° 2 (2026-10-19) : « pas prête », sans bloquant ; 1 important, 8 mineurs. Le
    critère de sortie (Q29) vérifié par le relecteur sur une copie de Timonier. Tes réponses :
    Q33 et Q34.
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - I-1 : un dossier renommé avant le commit laissait une boîte « supprimée ». Q33 : une
      boîte n'est nouvelle que si elle apporte un fichier nouveau (ou n'en a aucun), supprimée
      que si un de ses fichiers l'est (ou si elle n'en avait aucun) ; sinon modifiée si elle
      tient un fichier changé ou déplacé. Tests
      `a_folder_renamed_before_the_commit_leaves_no_box_gone`,
      `a_new_box_of_moved_files_is_modified_not_new` ;
    - m-1 : les symboles d'un fichier déplacé le suivent ; ouvert jusqu'à eux, ses liens ne
      sont plus « nouveaux ». Tests `the_symbols_of_a_moved_file_keep_their_links`,
      `two_symbols_of_one_name_are_told_apart_by_their_place` ;
    - m-2 : appariement des renommages linéaire en fichiers, mesuré (5.2) ; ADR 0024 corrigée ;
      test `a_box_is_renamed_only_into_one_of_its_kind` ;
    - m-3 : un gel `review_mark` en échec rafraîchit les vues ;
    - m-4 (Q34) : une crate qui perd son manifeste est modifiée, et la dépendance retirée nommée
      comme à la revue. Tests `a_crate_that_loses_its_manifest_is_modified`,
      `a_crate_without_its_manifest_is_modified_and_named_as_it_was` ;
    - m-5 : les deux README (libellé de Q31, Q33, réglage de densité) ;
    - m-6 : messages de la carte dans un module pur (`mapHost.ts`, 5 tests : densité à
      l'ouverture et au changement de réglage, bouton « Marquer comme revu ») ; densité de la
      page partagée avec la démo (`density.ts`). Trouvé en l'écrivant : la webview acceptait
      une densité inconnue ; elle la refuse ;
    - m-7 : les manifestes du commit filtrés comme le parcours les lit (ADR 0025) ; test
      `a_commit_snapshot_ignores_a_manifest_the_walk_skips` ;
    - m-8 : skill graph-model complété.
  - Relecture n° 3 (2026-10-19) : « pas prête », sans bloquant ; 2 importants, 3 mineurs.
    Critère de sortie revérifié sur un vrai démon ; un dossier renommé avant puis après le
    commit, aussi. Ta réponse : Q35.
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - I-a : un dossier renommé avant le commit laissait ses dépendances « retirées ». Une boîte
      qu'un regroupement seul a faite ou défaite (`ReviewDiff::regrouped`) emporte ses
      dépendances : pour elles, un déplacement n'explique rien, une modification si (Q33). Les
      lignes de la carte suivent la même règle ; les cycles sont cherchés dans les boîtes de la
      tête et comptent quand une dépendance ajoutée les ferme ; les bonds ne comptent que ce
      qu'un changement explique. Tests
      `a_folder_renamed_before_the_commit_keeps_its_dependencies`,
      `a_regrouped_box_keeps_what_its_edits_bring`,
      `a_cycle_through_a_regrouped_box_counts_when_an_edit_closes_it`,
      `a_regrouping_makes_no_jump`, et `a_new_box_of_moved_files_is_modified_not_new` complété ;
    - I-b (Q35) : une crate vient et part avec son manifeste, « Nouvelle » ou « Supprimée » ;
      test `a_crate_comes_and_goes_with_its_manifest` ;
    - m-a : un paquet nouveau, supprimé ou changé par son manifeste compte pour les boîtes qui
      le contiennent ; test `a_crate_changed_by_its_manifest_marks_the_boxes_above` ;
    - m-b : le générateur est dans `view_bench` (`synthetic:CxDxF`), chiffres refaits (5.2) ;
    - m-c : écart du fichier déplacé et modifié précisé.
  - Relecture n° 4 (2026-10-19) : « pas prête », sans bloquant ; 2 importants, 2 mineurs.
    Critère de sortie revérifié sur un vrai démon. Tes réponses : Q36 et Q37.
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - I-1 et I-2 (Q36) : les arêtes elles-mêmes sont comparées, sous les identifiants de la
      tête. Une dépendance, ou une ligne de la carte, ne change qu'avec une arête apparue ou
      disparue dans un fichier modifié (un déplacement n'en est pas un), ou une arête présente
      des deux côtés qui suit un fichier déplacé, ou un paquet changé par son manifeste, entre
      deux boîtes qui ne sont pas des regroupements. Les bonds partent des dépendances de la
      revue, avec celles ajoutées et sans celles retirées. Tests :
      `a_folder_renamed_before_the_commit_keeps_its_dependencies` et
      `a_regrouped_box_keeps_what_its_edits_bring` complétés (un commentaire ailleurs),
      `a_cycle_through_a_regrouped_box_counts_when_an_edit_closes_it` complété,
      `a_regrouping_of_packages_brings_no_dependency`,
      `a_file_moved_between_two_boxes_moves_its_dependencies` (et l'arête du serveur seul
      sur le fichier déplacé), le cycle qu'une arête du serveur seul fermerait
      (`a_cycle_that_grows_is_added_and_one_the_server_alone_closed_is_not`) ;
    - m-1 (Q37) : les fichiers d'une crate qui arrive ou part marquent la boîte qu'ils
      quittent ou rejoignent ; test `the_crate_that_gives_or_takes_the_files_is_modified` ;
    - m-2 : `commit_bench` (nouvel exemple) et `view_bench synthetic:` refont tous les
      chiffres synthétiques ; le générateur est partagé (`examples/support/synthetic.rs`).
    - Trouvé par les mutations : une arête que le serveur seul trouve dans un fichier déplacé
      comptait comme apportée par le déplacement ; corrigé et testé.
  - Relecture n° 5 (2026-10-19) : « pas prête », sans bloquant ; 1 important, 3 mineurs. Les
    corrections de la relecture n° 4 tiennent, revérifiées sur un vrai démon ; critère de
    sortie aussi. Tes réponses : Q38 et Q39.
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - I-1 : les dépendances d'une crate dont le manifeste part ou revient suivent ses fichiers
      dans la crate racine (Q36, Q37). Ses symboles, renommés avec la crate, sont retrouvés par
      leur place (fichier inchangé) ; un bout qui change de boîte est jugé aux deux bouts.
      Test `the_dependencies_follow_the_files_into_the_root_crate`, et une dépendance portée
      par le seul manifeste dans `a_crate_changed_by_its_manifest_marks_the_boxes_above` ;
    - m-1 : les trois mutations survivantes sont tuées ; test
      `a_cycle_counts_what_the_code_took_and_not_what_the_server_took` ;
    - m-2 (Q38) : un déplacement compte si ni la boîte que le fichier quitte, ni celle qu'il
      rejoint, n'est un regroupement ; test
      `a_move_into_or_out_of_a_regrouping_brings_no_dependency`. Le cas limite de Q39 est
      noté comme écart ;
    - m-3 : libellés des chiffres corrigés (environ 1 140 boîtes) ; ce qui change de boîte est
      rassemblé une fois par diff (la vue entièrement ouverte avait ralenti).
  - Relecture n° 6 (2026-10-19) : « pas prête », sans bloquant ; 1 important, 4 mineurs. Les
    corrections de la relecture n° 5 tiennent (vrai démon) ; critère de sortie revérifié.
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - I-1 : dans un espace de plus de 12 crates, renommer une crate, ce qui refait le
      regroupement, faisait lister de fausses dépendances et une ligne « nouvelle » en haut de
      la carte. Un paquet seulement renommé reste la même boîte, ni lui ni ses fichiers ne
      bougent ; et chaque arête gardée est placée aux deux bouts : un bout ne compte que s'il
      a lui-même changé de boîte au niveau de la dépendance, entre deux boîtes qui ne sont pas
      des regroupements, et pas au-dessus de son paquet resté le même (là, seul le regroupement
      du paquet l'a déplacé). Tests `a_crate_renamed_while_the_packages_regroup_changes_no_dependency`,
      `a_file_renamed_in_its_folder_moves_no_dependency`,
      `a_folder_moved_to_another_crate_takes_its_dependencies` (trouvé en refondant : un
      dossier déplacé d'une crate à l'autre ne portait plus ses dépendances avec une
      comparaison de la seule boîte la plus proche), `a_move_inside_a_crate_that_regroups_stays_inside`,
      `a_file_clustered_elsewhere_does_not_move` ;
    - m-1 : une dépendance portée par des appels entre symboles seuls suit le fichier
      déplacé ; testée dans `a_file_moved_between_two_boxes_moves_its_dependencies` ;
    - m-2 : code redondant retiré (les fichiers des paquets n'ont qu'un identifiant) ;
    - m-3 : mémoire mesurée dans un processus neuf (5.2), au plus 7,6 Mio pour 6 958 nœuds ;
    - m-4 : quand les deux bouts bougent, chacun compte pour son propre déplacement (Q38
      appliquée à chaque bout) ; écrit dans l'ADR 0024 et testé.
  - Relecture n° 7 (2026-10-19) : « pas prête », sans bloquant ; 1 important, 2 mineurs. Les
    corrections de la relecture n° 6 tiennent (vrai démon, dont la crate renommée dans un
    espace de 20 crates) ; critère de sortie revérifié. Tes réponses : Q40, Q41, Q42.
  - Corrigés, chacun avec un test qui échoue sans la correction :
    - I-1 (Q40) : un fichier renommé dans un dossier découpé en composants rejoignait le
      composant de ses voisins, ce qui comptait comme un déplacement (fausses dépendances, faux
      cycle, jusqu'au commit). Un fichier resté dans son dossier ne bouge plus. Test
      `a_file_renamed_in_its_flat_folder_does_not_move` ;
    - m-1 : les deux mutations survivantes sont tuées ; tests
      `a_move_inside_a_renamed_crate_that_regroups_stays_inside`,
      `a_line_follows_the_called_file` ;
    - m-2 (Q41) : les deux choix de l'ADR 0024 gardés, la phrase sur le dossier nouveau
      précisée (elle ne vaut qu'à l'intérieur d'une crate).
  - Relecture n° 8 (2026-10-19) : ni bloquant ni important ; 4 mineurs. Les corrections de la
    relecture n° 7 tiennent (vrai démon) ; critère de sortie revérifié. Verdict : « prête avec
    réserves » une fois le test des cinq secondes passé (Q42).
  - Mineurs :
    - m-1, corrigé : une crate qui perd son manifeste, avec un fichier renommé dans son
      dossier, ne portait plus sa dépendance dans la crate racine ; test dans
      `the_dependencies_follow_the_files_into_the_root_crate` ;
    - m-2, noté en écart : un fichier renommé et modifié dans un dossier découpé en
      composants (ADR 0024, Écarts ci-dessous) ;
    - m-3, corrigé : deux trous de tests ouverts par Q40 comblés
      (`a_move_that_keeps_its_component_brings_no_dependency`, et l'arête du serveur seul
      dans `a_file_renamed_in_its_flat_folder_does_not_move`) ; mutations relancées toutes
      ensemble, compte de 5.2 corrigé ;
    - m-4, corrigé : retour à la ligne du skill graph-model.
  - Réserves (Q42) : les écarts ci-dessous.

Critère de sortie (validé le 2026-10-18, Q29) : après « marquer comme revu », on ajoute à
Timonier une dépendance entre deux crates et un module ; la carte montre exactement ces
changements, et un cycle introduit dans une fixture est signalé.

Décisions :
- [ADR 0024](docs/adr/0024-diff-de-revue.md) (« Accepté ») : diff de revue, gel par le fil du
  pipeline, empreintes gardées à la revue, dépendances entre boîtes sœurs, seuil des bonds,
  protocole version 3.
- [ADR 0025](docs/adr/0025-paquets-du-commit.md) (« Accepté ») : snapshot de commit avec les
  paquets du commit (Q23) ; complète les ADR 0015 et 0022.
- [ADR 0026](docs/adr/0026-densite-de-la-carte.md) (« Accepté ») : densité de la carte (Q32).
Écarts :
- Un fichier à la fois déplacé et modifié n'est pas retrouvé : il est « supprimé » à un chemin
  et « nouveau » à l'autre (ADR 0024, alternatives écartées). Après le commit, sa boîte suit
  la majorité de ses fichiers. Avant, un dossier renommé dont un fichier a aussi été modifié
  laisse sa boîte « supprimée » (un de ses fichiers est parti, Q33), et ce fichier est
  « nouveau ». Renommé et modifié dans un dossier découpé en composants, il rejoint le
  composant de ses voisins : jusqu'au commit, le résumé peut lister des dépendances et un
  cycle entre composants de ce dossier (relecture n° 8, m-2).
- Un dossier vidé dans un autre n'est pas listé comme supprimé, ni ses dépendances comme
  retirées (Q33, Q36).
- Un cycle qu'un regroupement seul a fait n'est pas signalé ; un cycle qu'une dépendance
  ajoutée ferme l'est, au niveau que la carte montre, même entre composants regroupés au même
  commit.
- Un fichier regroupé au commit et modifié, qui appelle un autre fichier de la même boîte, fait
  lister une dépendance retirée et une ajoutée pour un code qui dépend toujours de cette boîte
  (Q39).
- Les listes du résumé sont du texte : un clic n'y mène pas encore à la boîte sur la carte.
- Q28 n'est testé qu'avec des fixtures et sur un vrai démon sans serveur de langage : pas de
  bout en bout avec rust-analyzer.
- La densité réglable, reportée de la Phase 4, est faite dans cette phase (Q32).
- Le gel `review_mark` relit le disque (66 ms pour 10 000 fichiers, mesuré à la relecture
  n° 15 de la Phase 4) : le clic attend ce temps.

## Phase 6 — MCP, initialisation et suivi

Statut : terminée (2026-10-20 ; ADR 0027 à 0035 acceptées par toi, en attente de ton feu vert
pour la Phase 7)

Prérequis : Q10 et Q11, répondues le 2026-10-20.

- 6.1 ADR — critère : ADR acceptées. Sujets :
  - SDK MCP (rmcp 3.5) et transport stdio ;
  - forme de l'en-tête de boîte aux lettres ;
  - formats versionnés (schémas JSON) de `.timonier/plan.json`, `todo.json` et
    `sessions/<id>.json`.
  - Écrites (« Proposé ») : [ADR 0027](docs/adr/0027-serveur-mcp.md) (rmcp 3.5.1 sur stdio,
    proxy à deux méthodes `tools/list` et `tools/call`, catalogue et validation dans le démon),
    [ADR 0028](docs/adr/0028-en-tete-boite-aux-lettres.md) (en-tête structuré et texte),
    [ADR 0029](docs/adr/0029-formats-du-suivi.md) (formats versionnés, états d'une tâche,
    sessions figées en snapshots, une session ouverte à la fois). Acceptées à ta validation de
    la phase.
  - Acceptées (2026-10-20) : les ADR 0027 à 0035 de la phase, toutes, à ta validation ; l'ADR
    0002 est remplacée pour les tâches par la 0035.
- 6.2 `timonier-mcp` : serveur MCP, mince proxy vers le démon (qu'il démarre au besoin),
  avec l'en-tête de boîte aux lettres dans chaque réponse (H10) — critère : test d'intégration
  client MCP → proxy → démon ; snapshot de la liste des outils.
  - `timonier mcp` (CLI) trouve ou démarre le démon sans rien écrire sur stdout, puis
    `timonier-mcp` relaie `tools/list` et `tools/call` vers les méthodes du même nom du démon
    (protocole version 4) ; le catalogue, la validation, les écritures et l'en-tête sont dans le
    démon (`timonier_core::tools`, ADR 0027, 0028).
  - Tests : `crates/timonier-cli/tests/mcp.rs`, un vrai client MCP (`rmcp`) → `tim mcp` → un
    vrai démon sur un projet temporaire : la liste des outils égale l'instantané
    (`crates/timonier-core/tests/tools.json`, test `the_catalogue_is_the_contract`), l'en-tête
    dans la structure et en tête du texte de chaque résultat, erreurs comprises, une session
    complète écrite dans `.timonier/`. Le nom du client MCP devient l'agent de la session.
- 6.3 Outils de suivi `session_start`, `session_end`, `plan_add_task`, `plan_update_task`,
  `todo_add`, `todo_complete`, plus les outils de lecture selon Q10. Écritures structurées et
  validées, le démon est le seul écrivain — critère : les entrées invalides et les transitions
  d'état interdites sont refusées (tests).
  - Formats et règles de l'ADR 0029 dans `timonier_core::tracking` (types, transitions, bornes,
    identifiants jamais réutilisés) ; le démon les applique (`timonier_daemon::tracking`) :
    fichiers relus à chaque appel, écrits en entier par renommage, un fichier cassé hors du
    démon refuse les écritures en le disant ; une session ouverte à la fois ; début et fin
    figés en snapshots `working_tree` par le fil du pipeline.
  - Lecture (Q10) : `get_plan` (avec l'avancement calculé de chaque jalon), `get_todo`,
    `get_session`.
  - Tests : `tracking.rs` du cœur (10 : identifiants, transitions toutes listées, bornes,
    réouverture avec une note, fichiers invalides, champs inconnus refusés, schémas stricts),
    `tools.rs` (4), `tracking.rs` du démon (4 : une session entière, refus avec ce qu'il faut
    faire, fichier cassé puis réparé à la main, identifiants de session).
- 6.4 Les changements faits hors session sont marqués « non suivis » — critère : une
  modification sans session ouverte est marquée (test).
  - À chaque `session_start`, les fichiers que le diff des snapshots touche depuis la fin de la
    session précédente (ou le premier snapshot) vont dans `sessions/untracked.json`, avec les
    deux snapshots et la session qui suit (ADR 0029, schéma `docs/schemas/untracked.schema.json`).
  - Tests : dans le démon, les écarts de deux sessions de suite ; de bout en bout
    (`crates/timonier-cli/tests/mcp.rs`), une modification faite hors session est marquée, une
    faite pendant une session ne l'est pas.
- 6.5 Dette de revue : chaleur par composant (code écrit en session et non couvert par une
  `review_mark` ultérieure ; changements hors session = non suivis), en calque sur la carte —
  critère : test sur fixtures ; checklist d'accessibilité.
  - Tes réponses : Q43 (les fichiers changés) et Q44 (un calque à activer). Règles et paliers
    dans l'[ADR 0030](docs/adr/0030-dette-de-revue.md) : depuis la dernière revue, les fichiers
    que le diff des snapshots touche dans chaque session, et hors session (écarts de
    `untracked.json`, et depuis la fin de la dernière session) ; par boîte, ses fichiers à relire
    sur ses fichiers ; paliers sous le tiers, sous les deux tiers, au-delà.
  - Démon : `timonier_daemon::debt`, calculé par le fil des vues avec la tête (les fenêtres closes
    gardées en mémoire), `ViewNode.debt { files, of, untracked }` dans la vue (protocole 4).
  - Carte : bouton « Dette de revue » dans la barre d'outils et commande de palette ; une boîte
    fermée dit « 9 fichiers à relire » à la place de son nombre d'éléments, un rôle fermé
    « 12 à relire » dans son titre (la somme de ses membres) ; le corps prend l'un des trois
    paliers du ton d'attention ; le non suivi a une marque hachurée et le mot « non suivi » ; la
    légende explique les paliers et les hachures. Dans le calque, la dette seule (pas les
    marques du diff de revue). L'état est gardé avec la page. La barre d'outils passe à la
    ligne avant de courir sous la légende.
  - Tests : `debt.rs` (comptes par boîte), `a_view_shows_the_review_debt` sur un vrai démon (une
    session crée de la dette, un changement hors session de la dette non suivie, une revue
    l'efface), `mapModel` et `uiState` (paliers, somme d'un rôle, état gardé), et
    `webview/test/a11y/debt.a11y.ts` (8 : bascule, mots, noms accessibles, légende, AA dans les
    six thèmes).
  - Checklist d'accessibilité :
    - [x] contraste AA dans les six thèmes, texte à 4,5:1 sur les paliers (le texte atténué
      d'une boîte en dette prend la couleur du texte), marques à 3:1 ;
    - [x] au clavier et en commande (« Afficher ou masquer la dette de revue sur la carte ») ;
    - [x] noms accessibles : « 12 files to read again, untracked. » ;
    - [x] jamais la couleur seule : des mots sur chaque boîte, des hachures et un mot pour le
      non suivi ;
    - [x] aucune animation.
  - Trouvé en route : la mesure du texte des tests d'accessibilité comptait les spans sans
    texte (icônes, nuancier) ; elle les laisse aux marques (`measure.ts`).
- 6.6 Panneaux — critère : les panneaux se mettent à jour en direct quand l'agent appelle
  les outils (test d'intégration) ; test des cinq secondes. Contenu :
  - plan : jalons → tâches, avancement calculé, panneau latéral permanent ;
  - todo en direct ;
  - journal des sessions : objectif, tâches, composants touchés, diff structurel, tests
    (l'effet sur les performances arrive en Phase 8).

  Fait (2026-10-20) :
  - [ADR 0031](docs/adr/0031-vue-du-suivi.md). Protocole version 5 : `tracking/subscribe`
    répond l'état entier (`TrackingView` : jalons avec l'avancement calculé par le démon, todo
    de la session ouverte, les 50 dernières sessions, fichiers illisibles) ; `tracking/update`
    le renvoie entier à chaque changement : après chaque `tools/call`, après chaque changement
    du graphe tant qu'une session est ouverte, et quand `.timonier/` change hors du démon (une
    seconde au plus, regardé seulement tant qu'un panneau est abonné).
  - Diff d'une session : le diff de revue (ADR 0024) entre ses deux snapshots, qui gardent le
    contenu de leurs fichiers (`Store::freeze_keeping_contents`, `forget_contents` ;
    `Reader::reviewed_contents` devient `kept_contents`) ; écrit à la fin dans le journal
    (`changes` : fichiers, boîtes ajoutées, modifiées, retirées, dépendances ajoutées et
    retirées ; schéma `docs/schemas/session.schema.json` refait), en direct pour la session
    ouverte (`timonier_daemon::journal`).
  - Interface : une vue webview « Plan, todo et sessions » dans la barre latérale secondaire
    (voir plus bas), une seconde page de la webview
    (`webview/tracking.html`, construite à part dans `dist/tracking/` : sous la CSP par nonce,
    une page ne peut charger qu'un seul script), un relais (`trackingRelay.ts`, un abonnement
    pour les trois vues, refait à chaque connexion). Jalons repliables avec barre de
    progression et mots ; une forme par état de tâche (anneau, demi-disque, triangle, disque
    coché, anneau barré) ; todo avec la tâche servie ; journal repliable, la session ouverte
    dépliée, composants touchés avec les marques du diff de revue.
  - Tests : `TrackingView::new` (ordre, session ouverte en direct, plafond), magasin (contenus
    gardés à travers une revue puis oubliés), `the_tracking_follows_the_agent_live` sur un
    vrai démon (deux connexions : le panneau suit l'agent, le diff de la session ouverte suit
    le code et finit à l'identique dans le journal, un `plan.json` cassé puis réparé à la
    main), `TrackingPanel.test.tsx` (16), `trackingRelay.test.ts` (7), et le test
    d'intégration VSCode « shows the plan and the todo live as the agent calls the tools » :
    les pages des vues Plan et Todo accusent la révision qui porte l'appel de l'agent.
  - Checklist d'accessibilité (`webview/test/a11y/tracking.a11y.ts`, 20) :
    - [x] contraste AA des trois panneaux dans les six thèmes (texte, icônes, barre de
      progression, anneau de focus) ;
    - [x] au clavier : jalons, tâches et sessions sont des `details` natifs ;
    - [x] noms accessibles : la barre de progression dit « Phase 6 : 2 of 5 tasks done », chaque
      tâche et chaque élément de todo disent leur état en mots ;
    - [x] jamais la couleur seule : une forme par état, des mots pour en cours, bloquée,
      abandonnée ;
    - [x] seule animation : le chevron qui tourne et la barre qui avance, coupés par
      `prefers-reduced-motion`.
  - Test des cinq secondes n° 1 (2026-10-20, toi, capture des trois panneaux côte à côte,
    Dark Modern, français) : **échoué en partie**. Réponse : « on voit un résumé de
    l'avancement à gauche, et le plan au centre en résumé, on dirait un plan plus détaillé sur
    la colonne tout à droite ». Le plan est lu ; la todo est prise pour un résumé du plan, le
    journal pour un plan détaillé. Causes : la capture posait les panneaux côte à côte sans
    leurs en-têtes de vue (VSCode les empile sous « PLAN », « TODO », « SESSIONS ») ; la todo
    s'ouvrait sur l'objectif sans dire de quoi il s'agit, et le journal répétait toute la todo
    de la session ouverte.
  - Précisé par toi : « le titre est trompeur », l'objectif de la session d'exemple (« Show the
    plan, the todo and the journal in the side bar »), présent en gros sur deux colonnes.
  - Corrigé (2026-10-20) : dans la todo, l'objectif n'est plus un titre mais une ligne
    discrète, « Session en cours : … », la liste est l'essentiel ; l'exemple a un objectif
    sans rapport avec les panneaux (« Cache the extraction of unchanged files ») ; dans le
    journal, la session ouverte ne donne que son compte (« 2 sur 4 faits · le détail est dans
    le panneau Todo ») ; la page de démo montre les trois panneaux empilés sous leurs
    en-têtes, comme la barre latérale (`?tracking=all`). Test n° 2 à faire.
  - Test des cinq secondes n° 2 (2026-10-20, toi, capture empilée) : **réussi** (« c'est
    mieux comme ça »), avec une demande : Sessions à droite, Plan et Todo dans la même colonne.
    Fait : Sessions passe dans un conteneur de la barre latérale secondaire (ADR 0031), Plan et
    Todo restent sous la vue d'ensemble ; le test d'intégration ouvre les trois et les voit
    suivre l'agent ; la démo `?tracking=all` montre les deux barres.
  - Puis, à ta demande (« il faudrait que les colonnes soient collées »), entre tout à droite,
    tout à gauche, deux barres ou un onglet, tu as choisi tout à droite : une seule vue « Plan,
    todo et sessions » dans la barre latérale secondaire, deux colonnes collées (le plan et la
    todo, puis les sessions, chacune sous son titre) dès 36 rem de large, empilées sinon ; la
    vue d'ensemble reste à gauche. Tests : `TrackingPanel.test.tsx` (18), `tracking.a11y.ts`
    (27, dont les colonnes collées à 700 px et empilées à 300 px), intégration (la vue suit
    l'agent). Capture : `/mnt/travail/timonier-panneaux-phase6-4.png`.
  - Test des cinq secondes n° 3 (2026-10-20, toi, sur la vue livrée : une vue, deux colonnes
    collées, `/mnt/travail/timonier-vue-suivi.png`, Dark Modern, français) : **réussi**.
    Réponse : « je vois l'avancement des phases du plan, ce qui est fait dans la todo et ce qui
    est encore en attente, les composants touchés par les modifs en cours de la session ; je
    regarderais sûrement la fenêtre de chat de Claude ou l'architecture ». Les trois parties
    sont lues ; la suite naturelle est l'agent ou la carte. 6.6 refermée (relecture n° 3, M1).
  - Écarts : les tests du journal attendent `timonier test` (Phase 8) : le journal le dit ;
    une session finie avant ce changement n'a pas de `changes` (« Non enregistré »).
- 6.7 Le pont (H5) : où en est le plan, ce qui s'est passé depuis la dernière revue, où
  regarder — critère : test des cinq secondes.

  Fait (2026-10-20) :
  - Tes réponses : Q45 (les alertes puis la dette), Q46 (un clic ouvre la carte sur la boîte),
    Q47 (la vue d'ensemble, à gauche). [ADR 0032](docs/adr/0032-le-pont.md), protocole version 6.
  - Démon (`timonier_daemon::bridge`) : l'état du suivi porte le résumé du diff de revue, la
    dette du projet, les sessions commencées depuis la revue (à la seconde : une session de la
    seconde de la revue compte avant) et « Où regarder », cinq éléments au plus : fichiers
    illisibles, tâches bloquées, cycles ajoutés, puis les boîtes en dette, non suivies d'abord,
    la plus précise quand toute la dette d'une boîte tient dans une fille ; chaque boîte avec
    son chemin depuis le haut de la carte. Recalculé à chaque changement du graphe.
  - Carte : commande `reveal { target, path }` : ouvre le rôle du haut du chemin, déplie le
    chemin, centre et sélectionne la boîte (`uiState.reveal`).
  - Interface : la vue d'ensemble devient une vue webview (partie `bridge` de la page du
    suivi) : « Plan » (jalon en cours, barre, tâches en cours et bloquées, session ouverte),
    « Depuis votre revue du … » (comptes du diff, sessions, fichiers à relire ; « Ouvrir la
    carte », « Marquer comme revu »), « Où regarder » (alertes en mots avec le triangle
    d'attention, boîtes cliquables, les non suivies marquées d'un filet et du mot). Commande
    « Où regarder : montrer sur la carte une boîte à relire » (liste dans la palette).
  - Tests : `bridge.rs` (3 : classement, plafond et chemin, sessions depuis la revue),
    `the_tracking_follows_the_agent_live` étendu (dette et « Où regarder » après la session,
    remis à zéro par une revue), `TrackingPanel.test.tsx` (pont, 5), `uiState.test.ts`
    (reveal, 2), intégration VSCode « shows the bridge, and reveals a box on the map from it ».
  - Checklist d'accessibilité (`tracking.a11y.ts`, 34 en tout) :
    - [x] contraste AA du pont dans les six thèmes ;
    - [x] au clavier : « Ouvrir la carte », « Marquer comme revu », puis chaque boîte ; et
      la commande de palette ;
    - [x] noms accessibles : « timonier-daemon 9 files to read again · untracked », barre de
      progression nommée ;
    - [x] jamais la couleur seule : triangle et mots pour les alertes, mot « non suivi » ;
    - [x] aucune animation de plus.
  - Capture : `/mnt/travail/timonier-pont.png`.
  - Test des cinq secondes n° 1 (2026-10-20, toi, capture du pont, Dark Modern, français) :
    **réussi en partie**. Réponse : « je vois ce que je dois relire depuis le dernier lot de
    modifications et le bouton pour ouvrir la carte, avec un résumé des changements ». Deux
    questions sur trois sont lues (depuis la revue, où regarder) ; l'état du plan, en haut,
    n'est pas cité. Accepté tel quel par toi (2026-10-20) : le plan a son propre panneau, à
    droite.
- 6.8 `timonier init` — critère : tests sur des projets fixtures Rust et TS ; une deuxième
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

  Fait (2026-10-20) :
  - Tes réponses : Q48 (Claude Code, Copilot, Cursor, Codex), Q49 (bloquer une fois), Q50
    (pre-commit), Q51 (au terminal, avec des options). [ADR 0033](docs/adr/0033-timonier-init.md),
    protocole version 7 ; formats des agents lus dans leurs documentations officielles.
  - `timonier-cli`, module `init` : détection (langages, gestionnaires, runners, git et
    `core.hooksPath`, agents par le `PATH`, `~/`, les extensions VSCode et le projet), plan de
    modifications pur, diff unifié (`similar` 2.7.0), écriture ; `.timonier/init.json` garde
    ce qui a été créé et l'original de ce qui a été changé.
  - Écrit : bloc géré dans `AGENTS.md` (et import `@AGENTS.md` dans un `CLAUDE.md` existant),
    serveur MCP dans `.mcp.json`, `.vscode/mcp.json`, `.cursor/mcp.json`, `.codex/config.toml`,
    hook d'arrêt dans `.claude/settings.json`, `.github/hooks/timonier.json`,
    `.codex/hooks.json` (`timonier hook stop`, démon : `agent/stop`), hook git `pre-commit`
    (`timonier hook pre-commit` : `actual/write`, puis `git add`), `.timonier/.gitignore` ; le
    plan « Initialisation » et une première session par les outils du démon, agent
    `timonier-init`. Un JSON avec commentaires, un hook git qui n'est pas du shell ou un
    gestionnaire de hooks sont laissés, et `init` dit quoi ajouter à la main.
  - `actual.json` (`timonier_core::actual`, schéma `docs/schemas/actual.schema.json`) : boîtes
    et dépendances agrégées comme sur la carte, triées, sans date.
  - Démon : `daemon/status` dit quand la première indexation est finie (`indexed`) ; le hook
    pre-commit l'attend (30 s au plus).
  - Tests : `detect.rs` (2), `edits.rs` (5), `plan.rs` (3 : deuxième exécution vide, fichiers
    illisibles laissés et dits, diff exact sans écriture), `init.rs` (réponse au terminal),
    `the_daemon_says_when_to_hold_an_agent_and_writes_the_actual_architecture` sur un vrai
    démon, et `crates/timonier-cli/tests/init.rs` sur des projets git temporaires Rust et
    TypeScript : `--dry-run` montre le diff et n'écrit rien, une deuxième exécution ne change
    rien, ce qui existait est gardé, `actual.json` entre dans le commit, le hook d'arrêt
    bloque une fois dans le format de chaque agent.
  - Écarts :
    - `--dry-run` liste les appels au démon (la session, le plan) sans en montrer le diff :
      leur contenu dépend de l'heure et des snapshots ;
    - avant toute session, un changement ne se distingue pas de l'indexation initiale : le
      hook d'arrêt ne tient compte du code changé qu'après une première session (celle
      d'`init`) ;
    - Cursor n'a pas de hook d'arrêt (il ne sait pas bloquer un arrêt) ; Gemini CLI attend
      (Q48).
- 6.9 `timonier uninit` — critère : init puis uninit rend un arbre identique à l'original
  (test).
  - Fait (2026-10-20) : `timonier uninit [--dry-run]` reprend `.timonier/init.json` : un
    fichier créé par `init` part, sauf ce que le développeur y a ajouté depuis ; un fichier
    changé perd la part de Timonier et revient exactement à son original quand rien d'autre
    n'y a changé ; `.timonier/` part en entier si `init` l'a créé (le démon est arrêté
    d'abord). Tests : `uninit_gives_the_original_tree_back` (Rust et TypeScript, octet pour
    octet, le hook git compris) et `uninit_keeps_what_the_developer_added`.
  - Corrigé après la relecture n° 1 (M2), ta réponse Q54 : le suivi du projet (`plan.json`,
    `todo.json`, `sessions/`, `intent.json`) reste dès qu'il y a eu du travail après `init`,
    avec `.timonier/.gitignore` ; il ne part que s'il ne tient que ce qu'`init` y a écrit (sa
    session, son jalon : l'arbre identique tient), ou avec `--purge`, confirmé au terminal
    (taper `purge`) ou par `--yes`. Tests : `uninit.rs` (2 : chaque fichier perd seulement sa
    part ; le suivi part seulement quand `init` seul l'a écrit, ou purgé, le tableau blanc
    compris) et `uninit_keeps_the_tracked_work_unless_purged` de bout en bout.
- 6.10 `timonier doctor` : vérifie et répare la configuration (MCP déclaré, bloc géré à
  jour, conventions présentes). Le test de fumée tracé arrive en Phase 8 (H6) — critère :
  chaque diagnostic et chaque réparation sont testés.
  - Fait (2026-10-20) : `timonier doctor [--fix | --check]` refait le plan d'`init` avec les
    agents de `.timonier/init.json` : chaque fichier qu'il changerait encore est un
    diagnostic (bloc géré absent ou ancien, import de `CLAUDE.md`, serveur MCP non déclaré,
    hook d'arrêt ou hook git absent, `.gitignore`, profil nextest, `tests.json`, conventions),
    et l'appliquer en est la réparation ; s'y ajoutent `timonier` absent du `PATH`, un démon
    qui ne démarre pas, un fichier de suivi illisible, et ce qu'`init` ne sait pas écrire
    (JSON avec commentaires, gestionnaire de hooks, runner manquant), dits avec quoi faire.
    Au terminal il demande avant de réparer (comme Q51) ; `--fix` répare sans demander ;
    `--check` ne répare rien et dit par son code de sortie si tout est en ordre (CI).
  - Tests : `doctor.rs` (5 : projet jamais initialisé, configuration en ordre, chaque
    diagnostic puis sa réparation, runner et conventions, ce qui ne se répare pas) et
    `doctor_finds_and_repairs_the_setup` de bout en bout (démon qui ne démarre pas,
    `todo.json` cassé, `--check`, `--fix`).
- 6.11 Packs Rust et TypeScript, volet init : détection du runner, configuration JUnit XML,
  conventions copiées dans `.timonier/conventions/` — critère : tests sur fixtures.
  - Fait (2026-10-20) : [ADR 0034](docs/adr/0034-packs-volet-init.md). La section `[tests]`
    des packs, réservée (ADR 0012), devient une liste de runners essayés dans l'ordre. Rust :
    cargo-nextest, profil `timonier` écrit entre marqueurs dans `.config/nextest.toml`
    (`cargo test` n'a pas de JUnit stable). TypeScript : vitest
    (`--reporter=junit --outputFile.junit=…`), `node --test` (`--test-reporter=junit`),
    jest (une note : il lui faut `jest-junit`), essayés sur vitest 5.0.4 et Node 24.18.
    `init` écrit `.timonier/tests.json` (lu par `timonier test` en Phase 8) et copie
    `conventions/instrumentation.md` du pack dans `.timonier/conventions/<langage>/` ; le
    bloc d'`AGENTS.md` y renvoie. `.timonier/.gitignore` garde `runs/` hors de git.
  - Tests : `builtin_packs_describe_their_runners_and_conventions`, `detect.rs`, et sur les
    fixtures Rust et TypeScript (`crates/timonier-cli/tests/init.rs`) : profil nextest,
    `tests.json`, conventions, la note quand cargo-nextest manque.
  - Écart : le SDK OpenTelemetry et le module d'initialisation arrivent en Phase 8 (H6) ; les
    conventions nomment déjà leurs paquets (versions vérifiées le 2026-10-20).
- 6.12 Détection dans VSCode d'un dépôt non configuré, avec une proposition discrète de
  lancer init — critère : test d'intégration.
  - Fait (2026-10-20), tes réponses Q52 et Q53 : l'état du suivi dit si le projet est
    configuré (`configured` : `.timonier/init.json` est là ; protocole version 8, le fichier
    regardé avec les autres). Tant qu'il ne l'est pas, le pont dit en haut « Ce projet n'est
    pas configuré pour vos agents » avec un bouton « Configurer », et sa vue porte un badge
    (sur l'icône Timonier). « Configurer », comme la commande « Timonier : Configurer le
    projet pour vos agents », ouvre le terminal « Timonier init » qui lance `timonier init`.
    Le badge part dès que le projet est configuré.
  - Tests : `TrackingPanel.test.tsx` (la proposition, puis plus rien), `trackingRelay.test.ts`
    (les observateurs de l'état), `launch.test.ts` (la commande citée pour le shell),
    `tracking.a11y.ts` (la proposition en AA dans les six thèmes, premier arrêt du clavier),
    le démon (`configured` suit `init.json`), et l'intégration VSCode « offers to set the
    project up, runs init in a terminal, and stops offering ».
  - Écart : VSCode ne pose un badge que sur une vue déjà ouverte ; tant que la barre Timonier
    n'a pas été ouverte dans la fenêtre, l'icône n'en porte pas.
  - « Configuré » se juge d'après `init.json`, pas `plan.json` : ta réponse Q55 (relecture
    n° 1, M3) précise Q18.
- 6.13 Bascule : `timonier init` sur le dépôt Timonier. Ensuite, une session Timonier par
  tâche, et plan et todo tenus via MCP selon Q11 — critère : vérifiable dans le journal des
  sessions.
  - Fait (2026-10-20), à ton accord :
    - `timonier` réinstallé depuis le dépôt (`cargo install --path crates/timonier-cli`) :
      celui du `PATH` datait du 14 et ne connaissait ni `mcp` ni `hook`.
    - `timonier init --agents claude` (ton choix : Claude Code seul) après relecture du
      `--dry-run` : bloc géré dans `AGENTS.md`, import dans `CLAUDE.md`, `.mcp.json`, hook
      d'arrêt ajouté à `.claude/settings.json` (8 lignes, le reste intact), profil nextest,
      conventions, `tests.json`, hook git `pre-commit`, et la session
      `S-2026-10-20-1` (`timonier-init`). Une deuxième exécution ne change rien.
    - En route, corrigé sur ce vrai dépôt : un JSON existant garde l'ordre de ses clés et sa
      mise en page (`init/ordered.rs`, à la manière de Prettier) ; TypeScript détecté par
      `tsconfig.base.json` ; le pack TypeScript reconnaît un script de test qui passe par les
      workspaces (une information, pas une faute) ; `doctor` sépare ce qui est à faire de ce
      qui est bon à savoir ; `.timonier/` est dans `.prettierignore` (fichiers écrits par
      Timonier).
    - [ADR 0035](docs/adr/0035-taches-dans-plan-json.md) : les phases et les tâches de ce plan
      repris dans `.timonier/plan.json` par les outils MCP (session `S-2026-10-20-2`, agent
      `timonier-migration`) : 11 jalons, 108 tâches dans leur état ; ici, les cases retirées,
      le détail gardé (ta réponse) ; le hook de début de session lit `plan.json` ; le
      relecteur lit l'état dans `plan.json`.
  - Vérifié (2026-10-20) : depuis la bascule, le travail s'est fait en sessions Timonier tenues
    par les outils MCP (`S-2026-10-20-3` pour 6.14, `S-2026-10-20-4` pour 6.1 et le commit,
    `S-2026-10-20-5` pour cette vérification), le plan et la todo aussi (`plan_update_task`,
    `todo_add`, `todo_complete`). Après le redémarrage de Claude Code avec le correctif B1,
    `S-2026-10-20-5` porte l'agent `claude-code` ; les deux d'avant, `mcp-client` (écart dit).
  - À faire par toi, si tu veux le JUnit des tests Rust : `cargo install cargo-nextest --locked`
    (`timonier doctor` le signale).
- 6.14 Relecture par `phase-reviewer` — critère : verdict « prête ».
  - Relecture n° 1 (2026-10-20) : « pas prête ». Tous les contrôles verts (Rust, Node, a11y
    109, intégration 23). Constats :
    - B1 : la session ouverte par Claude Code (`S-2026-10-20-3`) a pour agent `mcp-client` :
      le proxy ne lisait le nom qu'à la poignée de main, et le cycle de vie MCP 2026-07-28 le
      porte dans les métadonnées de chaque requête. Corrigé : `RequestContext::client_info()`
      (rmcp) lit les deux ; test `a_client_named_in_its_requests_metadata_names_the_session`
      (sans `initialize`, nom dans `_meta`) ; ADR 0027 précisée. `S-2026-10-20-3` garde son
      agent : le journal ne se réécrit pas.
    - M1 : `init` ne crée pas le tableau blanc vide de la spec : écart dit (ADR 0033, plus
      bas).
    - M2 : `uninit` effaçait tout `.timonier/`, plan migré compris, sans demander : ta
      réponse Q54, corrigé (6.9).
    - M3 : « configuré » d'après `init.json` et non `plan.json` (Q18) : ta réponse Q55.
    - m1 : un commentaire seul n'est ni non suivi ni en dette : ta réponse Q56, gardé.
    - m2 : une erreur du démon lui-même (verrou empoisonné, démon arrêté) rendait une erreur
      JSON-RPC sans en-tête : le démon la rend maintenant en erreur d'outil `internal`, avec
      l'en-tête (test `a_failure_of_the_daemon_is_a_tool_error_with_the_header`). Reste sans
      en-tête une panne de la liaison au démon (le démon absent) : l'en-tête vient du démon,
      il n'y en a pas de vrai à donner.
    - m3 : onze critères tronqués sur « … : » complétés par « voir PLAN.md », par
      `plan_update_task` ; les titres de plus de 200 caractères finissent par « … ».
    - m4, m5, m6 : écarts dits plus bas. m7 : `similar` ajouté au tableau. m8 : tests
      unitaires de `uninit.rs` et `hook.rs` (la décision du hook d'arrêt isolée et testée).
      m9 : ADR 0033 alignée sur les textes écrits (« Initialise Timonier »).
  - Relecture n° 2 (2026-10-20), contre-vérification : « pas prête ». Contrôles verts. Les
    corrections de la n° 1 vérifiées, sauf :
    - Bloquant : quand le démon (celui de VSCode) a créé `.timonier/` avant `init`, `uninit`
      gardait le suivi écrit par `init` seul, ne rendait pas l'arbre d'origine, et disait le
      suivi « là avant ». Corrigé : `init` note s'il y avait un suivi avant lui
      (`tracked_before`, un plan ou une session), et `uninit` en juge, pas du dossier ; le
      dossier ne part entier que s'il est d'`init`, sinon seuls `plan.json`, `todo.json`,
      `sessions/` (et `intent.json` sous `--purge`) partent. Test
      `uninit_takes_back_what_init_wrote_into_a_folder_the_daemon_made` (démon lancé avant
      `init`, un commit par le hook).
    - Majeur : depuis la bascule, le pont et le hook prenaient la Phase 0 pour le jalon en
      cours (des tâches restées à faire dans des phases closes). Corrigé : le jalon d'une
      tâche en cours, sinon le dernier qui a une tâche faite s'il n'est pas fini, sinon le
      suivant, sinon le premier non fini ; même règle dans `Bridge.tsx` et le hook, testée des
      deux côtés.
    - Majeur : `uninit` laissait `.timonier/actual.json` : il part avec la configuration.
    - Mineurs corrigés : messages de `--purge` justes ; `init` lit les identifiants que les
      outils rendent ; `journal.rs` a son test unitaire ; l'ADR 0002 dit « remplacement
      proposé » ; la liste du README ; les comptes de tests ; le tableau blanc vide : ta
      réponse Q57.
    - Mineurs restants, dits : le journal de `S-2026-10-20-3` liste les onze tâches dont j'ai
      corrigé le critère (un `plan_update_task` rattache la session à sa tâche) ; le test des
      cinq secondes de la vue finale (une vue, deux colonnes) reste à faire.
  - Relecture n° 3 (2026-10-20), contre-vérification : « pas prête », aucun bloquant, contrôles
    verts, les corrections de la n° 2 tiennent.
    - Majeur : 6.6 était faite sans le test des cinq secondes de la vue livrée (une vue, deux
      colonnes). T-76 rouverte (`plan_update_task`, avec une note) jusqu'à ce test.
    - Mineurs corrigés : `init.json` d'un projet initialisé avant `tracked_before` ne change
      plus à la deuxième exécution (le champ, faux, n'est pas écrit) ; `uninit` retire
      `actual.json` sans condition, comme le dit l'ADR 0033 (test) ; le pont et le hook comptent
      un jalon fini de la même façon (toutes ses tâches faites ou abandonnées) ; la règle du
      jalon en cours est écrite dans les ADR 0032 et 0035 ; le hook ne plante plus sur un
      `plan.json` lisible mais incomplet, il lit alors `PLAN.md` (test) ; Q57 à la ligne
      « Décisions ».
  - Relecture n° 4 (2026-10-20), contre-vérification : **« prête »**, aucun bloquant ni majeur,
    contrôles verts, les corrections de la n° 3 tiennent ; l'écart m5 de la n° 1 vérifié sur
    une copie de la base (aucun nœud ni arête au snapshot 6). Mineurs corrigés : le hook dit
    quand `plan.json` ne se lit pas (l'état des tâches est inconnu, `PLAN.md` ne le tient plus,
    test) ; un commentaire de code en anglais ; le cas « tous les jalons finis » écrit dans
    l'ADR 0035 ; les décomptes du journal de `S-2026-10-20-3` ; les captures disparues dites ;
    la première phrase du README au présent. Dit en écart : une session a servi deux tâches.

Critère de sortie : Timonier pilote son propre développement.

Décisions :
- ADR 0027 à 0035 (« Proposé ») ; tes réponses Q43 à Q57.
Écarts :
- Le journal ne montre pas encore d'effet sur les tests ni sur les performances (Phase 8) ; le
  SDK OpenTelemetry et le module d'initialisation non plus (6.11, H6).
- `init --dry-run` liste les appels au démon (session, plan) sans leur diff (6.8).
- Avant toute session, un changement de code ne se distingue pas de l'indexation initiale : le
  hook d'arrêt n'en tient compte qu'après la première session, celle d'`init` (6.8).
- Cursor n'a pas de hook d'arrêt, il ne sait pas bloquer un arrêt ; Gemini CLI attend (Q48).
- `init` ne crée pas le tableau blanc vide avant la Phase 7 : ta réponse Q57.
- Le badge du pont n'apparaît que sur une vue déjà ouverte dans la fenêtre (6.12).
- Bascule (6.13) : les tâches reprises de `PLAN.md` ont pour session la reprise
  (`S-2026-10-20-2`, sans travail propre), pas celles qui les ont faites (relecture n° 1, m4) ;
  et du travail a eu lieu sans session entre la reprise et le redémarrage de Claude Code
  (09:58 à 11:46, hooks, `init/`, `pack.rs`, `PLAN.md`, `CLAUDE.md`), sans écart non suivi
  enregistré, ce qui laisse penser qu'il ne changeait pas la structure (m5).
- `S-2026-10-20-3`, première session de Claude Code, a pour agent `mcp-client` (B1, corrigé
  ensuite), et liste les onze tâches dont elle n'a corrigé que le critère (relecture n° 2) ; elle
  a aussi servi le test des cinq secondes de 6.6 (T-76), rouverte pendant la relecture : une
  session pour deux tâches (relecture n° 4).
- Les captures des tests des cinq secondes n° 1 et 2 de 6.6 et de 6.7
  (`/mnt/travail/timonier-panneaux-phase6-4.png`, `/mnt/travail/timonier-pont.png`) ont disparu
  de `/mnt/travail/` (un redémarrage) ; les réponses restent consignées, et la capture du test
  n° 3 (`/mnt/travail/timonier-vue-suivi.png`) existe.
- Une panne de la liaison au démon remonte au client MCP sans en-tête de boîte aux lettres
  (relecture n° 1, m2).

## Phase 7 — Tableau blanc et intention

Statut : terminée (validée par toi le 2026-10-21, « commit et push et passe à la suite » ; ADR
0037 à 0042 acceptées). Restent de tes mains, notés : ton propre dessin pour le critère de sortie
(joué par Claude sur une copie, 7.7) et les tests des cinq secondes du tableau blanc et de la
comparaison. La relecture n° 8 a conclu « prête ».

Réponses appliquées : Q3 (pas de tldraw), Q6, Q57, Q60 à Q69.

- 7.0 ADR sur le remplaçant de tldraw (React Flow ou Excalidraw, voir Q3), soumise à ta
  validation car la spec en parlait explicitement — critère : ADR acceptée, licence vérifiée.
  - Fait : ADR 0036, acceptée le 2026-10-20 (Q60) : React Flow (`@xyflow/react` 12.12.0, MIT)
    pour les formes typées, perfect-freehand 1.2.3 (MIT, sans dépendance) pour le dessin libre.
    Paquet d'Excalidraw 0.18.1 lu sans l'exécuter : polices sur `esm.sh` par défaut (13 Mo à
    embarquer), un worker de module, `new Function` dans un module WebAssembly, 18 Mo de build.
- 7.1 Tableau blanc dans la webview, avec la bibliothèque retenue en 7.0 — critère : dessin
  libre, annulation et clavier fonctionnent dans le build de production du panneau, pas
  seulement en développement.
  - Fait : ADR 0037. Troisième page `whiteboard.html` (`--mode whiteboard`, `dist/whiteboard/`),
    panneau `timonier.whiteboard`, commande « Ouvrir le tableau blanc » et une commande de palette
    par action (message `whiteboard` de l'hôte, `WhiteboardCommand` généré depuis Rust, protocole
    du démon inchangé). Chaque trait est un nœud React Flow encré par perfect-freehand 1.2.3 ;
    outils sélection (V), crayon (P), gomme (E) ; historique de 200 pas (Ctrl+Z, Ctrl+Maj+Z,
    Ctrl+Y) ; le dessin est gardé dans l'état de la webview jusqu'à 7.3.
  - Tests : modèle et historique (vitest, 11) ; `webview/test/production/whiteboard.prod.ts`
    (Playwright, 12) sur le build de production servi dans la page de l'extension, sous sa CSP :
    aucune violation, dessin à la souris, gomme, annulation et rétablissement, rechargement,
    sélection, déplacement aux flèches et suppression au clavier, barre d'outils, français, puis
    WCAG AA dans les 6 thèmes ; test d'intégration VSCode (le panneau dit « prêt », les commandes
    arrivent). `npm run test:a11y` construit la page avant (121 tests).
  - Mesure : `npm run bench:whiteboard -w webview` (`webview/test/perf/whiteboard.perf.ts`, build
    de production, Chromium, 2026-10-21) : 1 000 traits chargés en 0,31 s, un trait ajouté en
    27 ms, une annulation en 35 ms ; 3 000 traits : 0,63 s, 63 ms, 95 ms. (La première mesure, du
    2026-10-20, n'était pas versionnée : relecture n° 1, I4.)
  - Écart : l'ADR 0036 disait un calque SVG sous les nœuds ; chaque trait est un nœud (ADR 0037),
    pour garder sélection, focus et noms de React Flow.
- 7.2 Formes typées optionnelles — critère : sérialisation testée. Correspondances :
  - boîte → composant voulu (nom, rôle) ;
  - flèche → arête `allowed`, `forbidden` ou `required` ;
  - zone → couche ;
  - note → contrainte en texte rattachée à un élément ;
  - note budget (latence p50/p95, mémoire max, allocations, nombre d'appels).
  - Fait : ADR 0038, Q62 à Q65. Outils Boîte (B), Zone (Z), Flèche (A, deux clics), Note (N),
    Budget (M) ; panneau des propriétés (nom, rôle, taille, politique, texte, attache, limites,
    « Nouvelle flèche vers ») ; au clavier, un outil de forme activé par Entrée pose la forme au
    milieu de la vue, et les commandes de palette « Ajouter … » font de même. IDs `C-`, `L-`,
    `E-`, `N-` numérotés par préfixe, jamais réutilisés. Couche calculée depuis le dessin ; une
    zone déplacée emporte son contenu ; une boîte supprimée emporte ses flèches et détache ses
    notes, en un pas. État de la page version 2 (la 1 se relit).
  - Tests : vitest, 19 sur le tableau blanc, dont 8 sur les formes : IDs, sérialisation aller-retour de
    chaque sorte, refus d'un état incohérent (référence vers rien, mauvais préfixe, limite
    négative, politique inconnue), lecture d'un état 7.1, couche, suppression en cascade,
    modification en un pas ; Playwright sur le build de production (14 au total) : formes au
    pointeur (zone, boîtes, couche, flèche et politique, note rattachée, zone qui emporte sa
    boîte, suppression et annulation), formes au clavier seul (boîte, note, budget, flèche par
    les propriétés, rechargement), WCAG AA dans les 6 thèmes avec des formes et le panneau
    ouvert ; test d'intégration (commande « Ajouter une boîte »).
  - Écart : Suppr est une touche du tableau, pas le raccourci de React Flow, qui ne se
    déclenchait pas sur une boîte reliée et aurait fait deux pas d'annulation (ADR 0038).
- 7.3 `.timonier/intent.json` version 1, conforme à la spec : sémantique séparée de la
  géométrie, IDs `C-` / `E-` / `L-` / `N-` stables, écrit seulement sur action de l'humain,
  ordre stable pour des diffs git lisibles, schéma JSON — critère : round-trip testé ; les
  IDs ne changent pas quand une forme est déplacée.
  - Fait : ADR 0039, Q66. Format dans `timonier_core::intent` (schéma
    `docs/schemas/intent.schema.json`, types TypeScript générés) : `layers`, `components` (avec
    leur couche, calculée depuis le dessin), `edges`, `notes`, `budgets`, puis `sketch`
    (`shapes`, `strokes`, `next`). Validation entière, ordre par ID, points d'un trait sur une
    ligne. Le démon seul écrit, par `intent/write` ; `intent/read` (protocole 9). L'extension
    enregistre 250 ms après le dernier geste, réessaie sans démon, écrit à la fermeture, recharge
    la page si le fichier change ailleurs ; la page dit « Enregistré » ou « Non enregistré », et
    n'écrase pas un fichier illisible. `init` écrit le tableau vide (Q57), `uninit` le reprend
    tant qu'il est vide.
  - Relecture n° 1 : l'exemple littéral de la spec était refusé (IDs nommés) ; Q69, il se lit
    maintenant (`reads_the_specs_example_and_writes_it_back_the_same` le lit tel quel, complété de
    la couche `L-infra` que son arête vise).
  - Tests : Rust, le format (5 : un tableau relu et réécrit à l'identique, forme déplacée
    = une ligne de diff et mêmes IDs, refus de 12 incohérences, tableau vide), le démon (lecture,
    écriture, ordre, réécriture sans changement, refus, fichier illisible), `init` / `uninit`
    (tableau vide, gardé une fois dessiné) ; vitest, la conversion (4 : sens séparé de la
    géométrie, aller-retour, IDs gardés au déplacement, intent incohérent) ; node:test,
    l'enregistrement (8 : délai, page rechargée, nouvel essai, refus, changement extérieur, fermeture,
    fichier illisible, démon arrivé après la page) ; Playwright, envoi après chaque geste, fichier
    changé ailleurs, état d'enregistrement, fichier illisible ; intégration VSCode : une boîte
    ajoutée par la palette arrive dans `intent.json` par le vrai démon, et son annulation aussi.
- 7.4 Interprétation du dessin libre (texte et vision), selon Q6 — critère : défini avec ta
  réponse Q67 : le croquis rendu en PNG (traits, formes et leurs IDs) et l'emplacement de chaque
  trait en texte (couche, composant proche), testés sur des fixtures ; rien ne revient sur le
  tableau.
  - Fait : ADR 0040. `timonier_daemon::sketch::render` (tiny-skia 0.12.0) : noir sur blanc,
    couches tiretées, composants, flèches selon leur politique, notes, traits, IDs écrits en
    police de points ; 1 600 pixels au plus. `Intent::stroke_places` dans `timonier-core`.
    Exemple `cargo run -p timonier-daemon --example sketch [intent.json] [out.png]`.
  - Tests : emplacements (4 traits : sur un composant, près de lui, dans une couche seule, hors
    de tout) ; image (couleurs aux bons endroits, même image pour le même tableau, aucune pour un
    tableau vide, plus grand côté borné, IDs écrits point par point).
- 7.5 Outils MCP `get_intended_architecture` et `get_drift` — critère : tests
  d'intégration MCP.
  - Fait : ADR 0041, Q68. `get_intended_architecture` (structure, places des formes et des traits,
    image PNG en contenu MCP, `image: false` pour s'en passer) ; `get_drift` (ancrages, manquants,
    arêtes interdites franchies avec leurs dépendances, requises absentes, non vérifiables),
    calculée par `timonier_core::drift` depuis `intent.json` et l'architecture de la tête. Le bloc
    d'`AGENTS.md` dit quand les appeler ; `doctor` dit quand le tableau blanc manque.
  - Tests : `drift` (3 : ancrage par nom souple et par place, interdites et requises entre couches
    et composants, rien de dessiné), base64 (RFC 4648), démon (les deux outils, avec et sans image,
    fichier illisible refusé) ; intégration MCP (client MCP → `tim mcp` → démon réel sur un
    workspace de deux crates : l'image arrive par le proxy, la flèche interdite `app` → `store`
    est franchie par l'appel réel, « Web » manque, le fichier n'est pas réécrit) ; `doctor`.
- 7.6 Option « comparer à l'intention » sur la carte — critère : des fixtures d'intention et
  de graphe donnent la dérive attendue. Contenu :
  - ancrage boîte dessinée ↔ composant réel par nom et emplacement, calculé et jamais écrit ;
  - composants dessinés mis en évidence, composants manquants listés ;
  - arêtes interdites franchies signalées (une arête entre couches s'applique à tout leur
    contenu) ;
  - arêtes requises absentes signalées.
  - Fait : ADR 0042. Méthode `intent/drift` (dérive, noms dessinés, lignée des boîtes citées) ;
    bouton « Comparer à l'intention » et commande de palette ; bandeau (ton, compte, manquants,
    interdites franchies avec leurs dépendances, requises absentes, « Ouvrir le tableau blanc ») ;
    une icône dans la barre de titre de la boîte qui le montre (coche cerclée : dessinée ; dans un
    cercle tireté : en contient une plus profonde ; relecture n° 3), et sur les membres d'une carte
    de rôle ; les mots dans son nom, son infobulle et le panneau de détails ; lien franchissant une
    arête interdite au ton de violation avec le mot « interdite », pas plus épais (ADR 0021), dans
    la légende aussi.
    Rafraîchie 500 ms après chaque révision et chaque changement d'`intent.json`. Un tableau vide
    n'est « rien de dessiné », pour la carte et pour `get_intended_architecture` / `get_drift`
    (trouvé par le test d'intégration : le tableau vide d'`init`).
  - Tests : Rust, une intention et un graphe de code donnent la dérive attendue (le critère ;
    `a_whiteboard_and_a_graph_give_the_expected_drift`), noms et lignées, tableau vide ; démon
    (`intent/drift`) ; vitest, où se montre une boîte (groupe, boîte, boîte qui la contient) et
    quel lien porte la traversée (3) ; node:test, l'alimentation de la carte (2) ; Playwright sur
    la démo de Timonier (bouton, bandeau, ouverture du tableau, marques, lien, légende, français,
    WCAG AA dans les 6 thèmes, puis les cas des relectures : 18) ; intégration VSCode : la carte compare avec le vrai démon,
    rien de dessiné puis un composant ancré sur `pkg:npm/fixture` et un manquant.
- 7.7 Relecture par `phase-reviewer` — critère : verdict « prête ».
  - Relecture n° 1 (2026-10-21) : « prête avec réserves », aucun bloquant, 5 importants, 8
    mineurs ; tous les contrôles verts (Rust 471, vitest 134, node:test 83 + 27, a11y 134,
    intégration 25). Corrigé :
    - I1 : le démon et la page n'avaient pas les mêmes règles (bornes des limites, `pen: false`,
      longueurs en caractères, taille des numéros) ; un fichier accepté par le démon pouvait
      s'ouvrir vide puis être écrasé. Mêmes règles des deux côtés, jeu de fichiers commun
      (`crates/timonier-core/tests/intent/`, lu par Rust et vitest), et un fichier que la page ne
      sait pas dessiner est signalé au lieu d'être montré vide ;
    - I2 : Q69 ; PLAN.md disait l'exemple de la spec relu à tort ;
    - I4 : le banc du tableau blanc est versionné, chiffres refaits ;
    - m1 : schéma JSON avec les motifs d'ID, la version et les bornes ; m2 : les noms comparés
      gardent leurs accents ; m3 : le skill disait « calque SVG » ; m4 : écarts complétés ; m6 :
      noté dans l'ADR 0039 ; m7 : « Non enregistré » est un statut, plus une alerte répétée ;
      m8 : un démon d'une autre version se dit comme tel (« refuse this one… `timonier daemon
      stop` »), et s'arrête sans poignée de main (à partir de cette version) ;
    - trouvé en jouant le critère de sortie sur une copie : le badge « Dessiné : Daemon » était
      coupé dans une boîte fermée ; une boîte dit « Dessiné », son nom et le panneau de détails
      disent lequel, et le test vérifie qu'il tient dans la boîte.
    Restent à toi : I3 (dessiner dans ce dépôt pour le critère de sortie) et I5 (les tests des
    cinq secondes du tableau blanc et de la comparaison) ; m5 est un écart (ci-dessous).
  - Relecture n° 2 (2026-10-21) : « prête avec réserves », aucun bloquant, 2 importants, 6
    mineurs ; les corrections de la n° 1 tiennent ; contrôles verts (Rust 474, vitest 136,
    node:test 83 + 27, a11y 134, intégration 25). Corrigé :
    - I-1 : l'image envoyée à l'agent effaçait ou déformait les IDs nommés (« L- », « C-NE ») ;
      police complète (capitales, chiffres, tiret, souligné ; un caractère inconnu est un carré
      creux), testée ;
    - I-2 : une carte de rôle fermée coupait son dernier membre ; plus de ligne d'intention : une
      coche sur chaque membre qui porte un composant dessiné ; les en-têtes ouverts et les boîtes
      fermées disent le mot seul, la ligne d'une boîte ne passe plus à la ligne ; « Contient du
      dessiné » devient « Dessiné dedans » (il était tronqué) ; tests de tenue ;
    - m-1 : motifs d'ID du schéma sans chiffres seuls nommés, longueurs des noms et des textes ;
      m-2 : la page range les IDs comme le démon ; m-3 : pas d'ID au-delà de 2⁵³ − 1 ; m-4 : une
      place de trait dans `sketch.shapes` est refusée ; m-5 : la CLI ne dit « autre version »
      que sur le refus de version, et dit comment arrêter un démon plus ancien ;
    - m-6, pour toi : l'exemple littéral de la spec reste refusé, non pour ses IDs mais parce que
      son arête vise `L-infra`, une couche qu'il ne déclare pas.
  - Relecture n° 3 (2026-10-21) : « prête avec réserves », aucun bloquant, 1 important, 8 mineurs ;
    contrôles verts (Rust 475, vitest 138, a11y 134, intégration 25). Corrigé :
    - I-1 : la correction n° 2 avait déplacé le défaut : avec la couche de dette, en densité
      compacte ou repliée, le badge « Dessiné » d'une boîte fermée tombait à 0 px ou se lisait
      « Dessiné… ». La marque devient une icône dans la barre de titre (coche cerclée, ou dans un
      cercle tireté pour « contient »), qui a toujours la place et reste au zoom lointain (m-2) ;
      la ligne du bas redevient ce qu'elle était avant la Phase 7. Tests de tenue : dette et revue,
      compact et revue, en anglais et en français. En thème clair, la coche verte sur la teinte
      de la barre de titre n'avait que 2,91:1 : un jeton `--tim-color-conformity-on-header` ;
    - m-1 : les membres d'une carte de rôle distinguent « dessiné » et « contient » par la forme,
      la légende explique les deux ; m-6 : un lien interdit et nouveau garde ses deux marques ;
      m-4 : le conseil d'arrêt vise le seul démon du dépôt (`pkill -f -- "daemon run --root …"`) ;
      m-7 : PLAN.md à jour ; m-8 : la borne des numéros (2⁵³ − 1) est dans la description du
      schéma, une expression régulière ne la porte pas ; m-3 : noté dans l'ADR 0040 (deux IDs qui
      ne diffèrent que par la casse s'écrivent pareil sur l'image).
  - Relecture n° 4 (2026-10-21) : « prête avec réserves », aucun bloquant, 1 important, 7
    mineurs ; contrôles verts (Rust 475, vitest 138, a11y 138, intégration 25). Corrigé :
    - I-1 : le lien interdit était deux fois plus épais, contre l'ADR 0021 acceptée (l'épaisseur
      est au trafic mesuré, Phase 8) ; il garde l'épaisseur des autres, testé ;
    - m-1 : un lien interdit et nouveau (ou d'un cycle ajouté) garde ses deux marques, testé ;
      m-2 : la marque de revue et l'icône d'intention ne se touchent plus ; m-4 : le conseil
      d'arrêt dit `pgrep` puis `kill` au lieu d'un `pkill` qui pouvait viser trop large, et le
      refus de version a son test côté CLI ; m-5 : le nom d'une carte de rôle dit, membre par
      membre, quel composant dessiné il porte, et chaque icône de membre a son infobulle ; m-6 :
      le README dit l'icône ;
    - écarts notés : m-3, avec la revue, le nom d'une boîte fermée étroite est coupé plus tôt
      (le badge de revue depuis la Phase 5, l'icône ajoute 16 px) ; son nom entier reste dans son
      nom accessible, son infobulle et le panneau de détails ; m-7, antérieur à la Phase 7 : le
      titre d'un rôle ouvert dans un cadre étroit se réduit, pour une phase suivante.
  - Relecture n° 5 (2026-10-21) : « prête avec réserves », aucun bloquant, 1 important, 4
    mineurs ; contrôles verts (Rust 476, vitest 138, a11y 139, intégration 25). Corrigé :
    - I-1 : `daemon/shutdown` admis avant la poignée de main changeait une règle de l'ADR 0019
      acceptée sans ADR ; noté dans l'ADR 0039 (« complète l'ADR 0019 »), et la ligne de statut
      de l'ADR 0019 y renvoie ;
    - m-1 : sur un lien interdit et nouveau, l'étiquette double passait sous une boîte voisine et
      cachait le mot ; la marque de revue y est son icône seule, et un test vérifie que le mot
      « forbidden » se voit (`elementFromPoint`) ; m-2 : textes périmés corrigés ; m-3 : la ligne
      « Écarts » de la phase reprend ceux de la relecture n° 4 ; m-4 : un test vérifie que la
      marque de revue et l'icône d'intention ne se chevauchent pas.
  - Relecture n° 6 (2026-10-21) : « prête avec réserves », aucun bloquant, 1 important, 2
    mineurs. Corrigé :
    - I-1 : le correctif m-1 de la n° 5 ne tenait pas, et son test ne pouvait pas le voir : React
      Flow dessine les boîtes par-dessus les étiquettes, et l'étiquette du lien inverse couvrait
      le mot. Le mot se place le long de son lien, hors du milieu, là où aucune boîte ne le couvre
      (`map/labelSpot.ts`, testé), dans le calque au-dessus des boîtes (`ViewportPortal`) ; le test
      mesure chaque mot « forbidden » en cinq points, toutes les étiquettes rendues mesurables,
      sur quatre tailles de vue ; l'ADR 0042 note cette seule étiquette sur une boîte ;
    - m-1 : le décompte des tests ; m-2 : l'index des ADR dit que la 0039 complète la 0019 ;
    - en passant : le cercle tireté de « contient » se distingue mieux du cercle plein.
  - Relecture n° 7 (2026-10-21) : « prête avec réserves », aucun bloquant, 1 important, 3
    mineurs ; contrôles verts (Rust 476, vitest 141, a11y 143, intégration 25). Corrigé :
    - I-1 : le mot du lien `timonier-core → timonier-store` flottait au milieu de la boîte, sur un
      tronçon caché de son lien, contre sa barre de titre, et le test ne le mesurait pas à trois
      tailles sur quatre. Le mot ne se pose plus que sur un tronçon visible de son lien, là où il
      couvre le moins (une barre de titre pèse 50 fois un corps de boîte) ; le test cadre toute la
      carte, exige chaque mot mesuré, et vérifie qu'aucun ne couvre un nom, une marque ou un titre
      ni ne se pose sur une partie cachée de son lien, sur cinq cas (quatre tailles, compact) ;
    - m-1 : les obstacles se calculent une fois par mise en page, plus à chaque image d'un
      déplacement ou d'un zoom ; m-2 : le cas compact est dans le test ; m-3 : l'ADR 0021 et
      l'index renvoient à l'ADR 0042.
  - Relecture n° 8 (2026-10-21) : « prête », aucun bloquant ni important, 1 mineur ; contrôles
    verts (Rust 476, vitest 142, a11y 144, intégration 25) ; le correctif I-1 tient sur 125
    configurations. m-1 : quand tout autre tronçon couvrirait une barre de titre, le mot du lien
    `timonier-core → timonier-store` revient au milieu, sur l'étiquette « Cycle » du lien
    inverse ; dernier recours voulu (il porte lui-même l'icône de cycle, le lien inverse garde sa
    couleur et son nom), désormais écrit dans l'ADR 0042.
  - Critère de sortie joué par Claude sur une copie jetable du dépôt (2026-10-21, à ta
    demande, ton tableau blanc n'étant pas le sien) : le vrai tableau blanc de production
    (zone « Cœur », boîtes « Daemon », « Store », « Web », flèche interdite Daemon → Store, un
    trait autour de Web), écrit par `intent/write` d'un vrai démon ; la carte comparée dit
    « 3 composants dessinés, 2 trouvés dans le code, 1 manquant · 1 arête interdite franchie »,
    liste Web et « Daemon → Store : 6 dépendances », marque `timonier-daemon` et `timonier-store`
    et signale le lien au ton de violation, avec le mot « interdite » ; l'agent, par `tim mcp`, lit l'intention, l'image et la dérive (« Forbidden
    edge E-1 crossed: C-1 Daemon → C-2 Store, 6 dependencies »). Reste ton dessin à toi.

Critère de sortie (validé le 2026-10-20, Q61) : tu dessines trois boîtes et une flèche interdite pour
Timonier ; la carte les met en évidence, liste les manquants et signale la violation ; l'agent
lit l'intention via MCP.

Décisions : ADR 0036 à 0042.
Écarts : 7.1, les traits sont des nœuds React Flow et non un calque à part (ADR 0037) ; 7.2,
Suppr est une touche du tableau et non le raccourci de React Flow (ADR 0038) ; les réglages du
panneau des propriétés (nom, rôle, taille, politique, attache, limites, « Nouvelle flèche vers »)
se font au clavier dans le panneau, sans commande de palette chacun : les outils, l'ajout des
formes, l'annulation et la suppression en ont une (relecture n° 1, m5) ; avec la revue, le nom
d'une boîte fermée étroite est coupé un peu plus tôt par l'icône d'intention, son nom entier
restant dans son nom accessible, son infobulle et le panneau de détails (relecture n° 4, m-3) ;
le titre d'un rôle ouvert dans un cadre étroit se réduit, défaut antérieur à la Phase 7
(relecture n° 4, m-7).

## Phase 8 — Performances

Statut : terminée (validée par toi le 2026-10-21)

- 8.1 ADR — critère : ADR acceptées. Sujets :
  - stockage des traces et des métriques ;
  - nom de la feature qui désactive l'instrumentation ;
  - niveau 1 sur macOS (H8) ;
  - compteurs matériels.

  Fait (2026-10-21) : ADR 0043 (base à part `.timonier/perf.sqlite`, rétention de 50 runs,
  200 000 spans par run), 0044 (feature `otel` du binaire, active par défaut, export seulement
  si `OTEL_EXPORTER_OTLP_ENDPOINT` ; `code.function.name` déduit par le récepteur pour les
  spans `tracing`, qui ne l'écrit pas), 0045 (relais mesureur `timonier __measure` : `getrusage`
  partout, scope systemd utilisateur sous Linux quand cpu et memory sont délégués ; JUnit par
  `junit-parser`), 0046 (compteur d'instructions `perf-event` sur l'arbre entier, repli sur le
  temps CPU, Q70). Proposées, acceptées à la validation de la phase (Q15).
- 8.2 `timonier test -- <commande>`, niveau 1 — critère : test d'intégration sur un projet
  fixture. Contenu :
  - durée, CPU, mémoire max et I/O via cgroups v2 / procfs sur Linux, avec un repli sur
    macOS ;
  - durée par test lue dans le JUnit XML (junit-parser 1.7, voir « Dépendances vérifiées »).

  Fait (2026-10-21) :
  - `timonier test [--junit F] [--root D] [-- <commande>]` ; sans commande, les runners de
    `.timonier/tests.json` (`sh -c`), un run chacun ; le JUnit vient de `--junit`, sinon du
    runner dont c'est la commande ; un rapport plus ancien que le run n'est pas lu (et le run
    le dit). Code de sortie : celui de la commande (127 si elle ne se lance pas).
  - Relais `timonier __measure` (ADR 0045) : `getrusage(RUSAGE_CHILDREN)` (CPU, plus gros
    processus, blocs) ; sous Linux, scope systemd utilisateur quand `systemd-run` est là et que
    cpu et memory sont délégués (`memory.peak` de l'arbre, `cpu.stat`, `io.stat` si io est
    délégué, échantillons toutes les 100 ms) ; `TIMONIER_MEASURE=rusage` force le repli.
    Compteur d'instructions `perf-event` sur l'arbre (ADR 0046) ; refusé ici
    (`perf_event_paranoid` = 4), le run le dit et `timonier doctor` donne la commande `sysctl`
    (une note, pas un constat : `--check` reste vert).
  - Le démon garde le run (`run/record`, sans changer le numéro du protocole : une méthode de
    plus ne casse aucun client) dans `.timonier/perf/perf.sqlite`, avec son `.gitignore` (ADR
    0043, chemin précisé) : snapshot de tête, commit, branche, empreinte de la machine.
  - Tests : `crates/timonier-cli/tests/test.rs` (projet fixture en git, vrai démon : mesures,
    tests et durées du JUnit, fichier et ligne, commit et branche, repli rusage, runners de
    `tests.json`, rapport périmé, commande introuvable), unités du relais, du JUnit, du store
    et de l'empreinte. Essayé à la main : 70 Mo pour tout l'arbre dans le scope, 68 Mo pour le
    plus gros processus sans.
  - Le snapshot est pris au début du run depuis 8.3. Un Ctrl-C arrête aussi le relais : le
    run reste commencé, sans mesures. Sur Timonier, les durées par test demandent cargo-nextest (`doctor` le dit).
- 8.3 Injection de `OTEL_EXPORTER_OTLP_ENDPOINT` et `OTEL_RESOURCE_ATTRIBUTES`
  (`timonier.run_id`, `timonier.snapshot`) ; chaque test devient un span racine — critère :
  test.

  Fait (2026-10-21), ADR 0047 :
  - le run commence chez le démon (`run/start` : numéro, snapshot de tête au début, commit,
    branche, adresse du récepteur) et finit par `run/finish` ; `run/record` de 8.2 disparaît ;
    un run jamais fini reste, sans mesures ;
  - la commande reçoit `OTEL_EXPORTER_OTLP_ENDPOINT` (pour l'instant
    `http://127.0.0.1:4318`, le port HTTP par défaut ; 8.4 donnera l'adresse réelle) et
    `OTEL_RESOURCE_ATTRIBUTES` avec `timonier.run_id` et `timonier.snapshot` après les
    attributs déjà posés ;
  - chaque cas du JUnit XML devient un span racine synthétisé du run (`timonier.test.name`,
    `.classname`, `.status`, `code.file.path` et `code.line.number` quand le runner les
    donne), daté par le `timestamp` du cas (lu en RFC 3339) ; les spans du code s'y
    rattacheront par `timonier.test.name` au récepteur (8.4), posé par les conventions (8.6 ;
    nextest donne `NEXTEST_TEST_NAME` et `NEXTEST_BINARY_ID`, vérifié) ;
  - critère : `the_tests_are_told_their_run_and_each_test_becomes_a_root_span` (vrai démon),
    plus les tests du store (début, fin unique, racines) et du CLI (attributs, dates).
- 8.4 `timonier-otlp` : récepteur gRPC 4317 et HTTP 4318 (tonic 0.14, opentelemetry-proto
  0.33), ports occupés gérés, rattachement au graphe par les attributs `code.*` — critère :
  test d'intégration avec un vrai exporteur OTLP.

  Fait (2026-10-21), ADR 0048 :
  - `timonier-otlp` : `decode` (OTLP → spans et points des runs : `code.*`, test, statut,
    attributs bornés à 4 Kio ; `code.function.name` déduit de `code.module.name`, ADR 0044) et
    `Receiver` (127.0.0.1 ; 4317 et 4318 sinon un port du système ; chaque port sert gRPC et
    OTLP/HTTP en protobuf et en JSON ; 415 pour la compression et les autres types, 400 pour un
    message qui ne se lit pas ; 16 Mio au plus) ;
  - le démon le démarre au premier `run/start` et donne son adresse à la commande ; il garde ce
    qui nomme un run ouvert (ou fini depuis moins de 60 s), au plus 200 000 spans par run (le
    reste compté), efface les spans et métriques des runs plus anciens que les 50 derniers ;
  - rattachement : au test par `timonier.test.name` et `.classname` (à la fin du run, ou à
    l'arrivée s'il est fini) ; au nœud par ID, puis nom dans le fichier, puis ligne, puis
    fichier, la règle gardée ; `timonier test` attend la première indexation avant d'ouvrir le
    run ;
  - critère : `crates/timonier-otlp/tests/receiver.rs` (un vrai exporteur `opentelemetry-otlp`
    en gRPC, vers chacun des deux ports ; OTLP/HTTP protobuf et JSON écrit à la main ; port pris)
    et `the_spans_of_a_real_exporter_come_back_attached_to_their_test_and_node` (`timonier test`
    lance `examples/emit_spans.rs`, instrumenté avec le SDK : ses trois spans reviennent avec
    leur test et leur nœud, par ID, par nom et par ligne).
- 8.5 Vérification de l'instrumentation : fonctions publiques jamais vues, spans sans
  `code.*`, surcoût estimé contre le niveau 1 ; outils MCP `get_instrumentation_gaps` et
  `get_instrumentation_conventions(lang)` — critère : une fixture aux trous connus les fait
  tous remonter.

  Fait (2026-10-21), ADR 0049 :
  - `timonier-core::gaps` : fonctions publiques (Rust : signature `pub` ; TypeScript : toutes
    sauf privées, faute d'`export` dans le graphe, dit dans `approximations`), hors code de
    test ; jamais vues = aucun span rattaché par ID, nom ou ligne ; spans sans `code.*` groupés
    par nom avec ce qui manque ; spans sans nœud ; spans au-delà du plafond ; surcoût =
    médianes des runs avec et sans traces de même commit, commande et empreinte ;
  - `timonier test --without-traces` (`OTEL_SDK_DISABLED=true` ; le récepteur ne garde rien
    d'un tel run ; le SDK Rust 0.33.1 ne lit pas la variable, vérifié : les conventions la
    liront, 8.6) ;
  - outils MCP `get_instrumentation_gaps { run?, cursor?, limit? }` (le dernier run fini avec
    traces ; pages de 100, 500 au plus ; arguments vérifiés par le démon) et
    `get_instrumentation_conventions { language }` (`.timonier/conventions/<langage>/`) ;
    catalogue et bloc géré d'`AGENTS.md` à jour ;
  - critère : `a_fixture_s_known_gaps_all_show` (fixture : `h` publique jamais vue, `p` privée
    ignorée, un span sans rien, un span avec son seul fichier, un span sans nœud, le surcoût
    contre un run sans traces) et `the_conventions_of_a_language_are_those_init_copied`.
  - Écart : le surcoût n'a qu'une mesure de chaque côté tant que la méthode des runs répétés
    (8.9) n'existe pas.
- 8.6 Instrumentation — critère : `timonier doctor` produit une trace rattachée au graphe.
  Contenu :
  - Timonier s'instrumente lui-même, derrière la feature ;
  - volet instrumentation de `timonier init` pour Rust et TS (SDK OpenTelemetry, module
    d'initialisation) ;
  - test de fumée de `timonier doctor` (H6).

  Fait (2026-10-21), ADR 0050 :
  - Timonier s'instrumente (ADR 0044) : `timonier-cli/src/telemetry.rs` (logs sur stderr par
    une seule installation, et la couche OpenTelemetry derrière la feature `otel`, active par
    défaut, quand `OTEL_EXPORTER_OTLP_ENDPOINT` est posée sans `OTEL_SDK_DISABLED=true`) ; des
    spans `#[instrument(skip_all)]` aux frontières : écritures du store et des mesures, diff et
    agrégation, étapes du pipeline, requêtes JSON-RPC (`method`), outils MCP (`tool`), runs,
    décodage OTLP, `doctor` ; essayé : un `tim doctor` lancé sous `tim test` envoie
    `run_doctor` avec `code.function.name` déduit, `code.file.path` et `code.line.number` ;
  - volet instrumentation d'`init` (Q72) : section `[instrumentation]` des packs et
    `[tests.runners.smoke]` ; `Cargo.toml` (deux blocs), `package.json` (devDependencies),
    `src/timonier_otel.rs` et `tests/timonier_smoke.rs`, `timonier.otel.ts` et
    `timonier.smoke.test.ts` (vitest) ; `smoke` dans `tests.json` ; `uninit` reprend tout ;
    conventions des deux packs réécrites ;
  - `doctor` lance le test de fumée (`--no-smoke` pour le sauter) ;
  - critère : `doctor_s_smoke_test_produces_a_trace_attached_to_the_graph` (fixture Rust :
    `init`, vrai SDK compilé, `doctor --check` dit « the smoke test (rust) produced a trace
    attached to the graph », puis `uninit` rend `Cargo.toml` tel quel) ; tests des éditions de
    manifestes (aller et retour, `[features]` du développeur, workspace, feature prise).
  - Écarts : le test de fumée TypeScript n'est pas joué dans les tests (il faut `npm install`
    du SDK) ; les conventions de `.timonier/` de Timonier restent à l'ancienne version jusqu'à
    un `timonier doctor --fix`, que je ne lance pas : il poserait aussi le tableau blanc vide,
    qui est à toi.
- 8.7 Vues — critère : test des cinq secondes. Contenu :
  - timeline par test (spans emboîtés, ressources du processus dessous, uPlot) ;
  - carte annotée (temps cumulé, nombre d'appels, arêtes épaissies selon le trafic) ;
  - évolution par commit ;
  - à ta demande (2026-10-18), s'inspirer des panneaux « Performances » des outils de
    développement de Chrome et de Firefox : un arbre des appels qu'on déplie, avec le temps
    passé en ms et les ressources utilisées à chaque niveau.

  Fait (2026-10-21), ADR 0051 :
  - démon : `perf/runs`, `perf/run` (20 000 spans au plus, temps depuis le début du run),
    `perf/map` (temps propre et appels par nœud, trafic entre nœuds, lignées) ;
    `timonier-core::perf::map_of`, pur et testé ;
  - calque « Performances » de la carte (bouton, commande `timonier.map.showPerformance`, état
    gardé) : temps passé dans chaque boîte, liens épaissis selon les appels mesurés (échelle
    log), bandeau du run et de ses trois nœuds les plus lents, légende ; `PerfMapFeed` dans
    l'extension (`perf/runs` toutes les 3 s tant qu'il est allumé) ;
  - panneau Performances (`webview/src/performance/`, page `performance.html`, commande
    `timonier.openPerformance`, bouton du pont et du calque) : choix du run et ses chiffres,
    onglets Timeline (tests, spans par profondeur, mémoire et CPU du processus pendant le
    test), Arbre des appels (`treegrid` déplié au clavier, total, propre, appels, mémoire et CPU
    du processus par niveau), Évolution (durée et CPU par run, nommés par commit) ; uPlot
    1.6.32 ; un tableau caché à la vue pour chaque courbe ;
  - tests : modèle (vitest), feeds (node:test), `the_views_read_a_run_its_spans_and_the_map_s_layer`
    (vrai démon, vrai exporteur), `performance.a11y.ts` (clavier, tableaux, AA dans les six
    thèmes, calque compris), intégration (le panneau s'ouvre sous la CSP, le calque interroge le
    démon) ;
  - reste : le test des cinq secondes, à toi (critère de 8.7).
  - Écarts : les ressources d'un niveau de l'arbre sont celles de tout le processus pendant qu'il
    tournait, pas du niveau (rien ne les mesure) ; sans scope systemd (macOS), pas de courbe ni
    de ressources par niveau.
- 8.8 Budgets — critère : un budget dépassé volontairement est visible sur la carte et
  lisible via MCP. Contenu :
  - chaque note budget de l'intention est comparée à la médiane de plusieurs runs ;
  - un dépassement apparaît en rouge, avec une icône, sur la carte et dans le journal ;
  - outils MCP `get_budgets`, `get_budget_violations` et `get_perf(component)`.

  Fait (2026-10-21), ADR 0052 :
  - `timonier-core::budgets` (pur) : cible de chaque budget (ancre, couche, flèche, tests),
    appels entrants d'un run, p50 et p95 au rang le plus proche, médiane des 5 derniers runs avec
    traces qui ont atteint la cible, hors tests de fumée (relectures n° 1 et 2), états `ok`,
    `exceeded`, `unmeasured`, `unanchored` ; les allocations restent non mesurées ;
  - démon : `perf/budgets` ; outils MCP `get_budgets`, `get_budget_violations`, `get_perf
    { component, run? }` ; `session_end` garde les budgets dépassés dans la session (champ
    `budgets`, schéma régénéré) ;
  - carte : badge « Budget dépassé » dans la barre de titre de la boîte, toujours, chiffres au
    survol et dans le nom, légende ; `BudgetFeed` dans l'extension ; journal : la partie « Tests »
    d'une session montre ses budgets dépassés ; bloc géré d'`AGENTS.md` ;
  - critère : `a_budget_exceeded_on_purpose_shows_on_the_map_through_mcp_and_in_the_journal`
    (fixture : un p95 d'une nanoseconde sur `demo`, vrai exporteur, vrai démon : MCP, données de
    la carte avec la boîte, `get_perf`, journal) et `performance.a11y.ts` (la marque sur la
    carte, son nom, AA dans les six thèmes).
- 8.9 Détection de régressions selon la méthode décidée dans la spec — critère : tests
  statistiques sur des jeux de données connus ; une fonction ralentie exprès passe de
  « probable » à « confirmée ». Méthode :
  - instructions CPU par compteurs matériels quand c'est possible, temps réel affiché ;
  - 5 exécutions, jusqu'à 15 si la variance est forte ; médiane et MAD ;
  - référence = médiane des 5 derniers snapshots de la branche principale ;
  - régression signalée si Mann-Whitney p < 0,01 et écart ≥ max(5 %, 3 × MAD) ;
  - « probable », puis « confirmée » après un nouveau run ;
  - référence réinitialisée si l'empreinte de la machine change.

  Fait (2026-10-21), ADR 0053 :
  - `timonier-core::regression` : médiane, MAD, Mann-Whitney unilatéral (exact sans ex æquo
    jusqu'à 40 valeurs, sinon normal corrigé), jugement selon la spec ; vérifié contre SciPy
    1.18.1 sur des jeux connus ;
  - `timonier test --runs N|auto` (5, jusqu'à 15 si la MAD dépasse 5 % de la médiane), lots
    `B-…` ; référence : les 5 derniers commits de la branche principale (`origin/HEAD`, sinon
    `main`), même commande et même empreinte ; mesures : coût du run, durée de chaque test,
    temps propre de chaque nœud ; « probable » puis « confirmée » par le lot précédent du même
    commit ;
  - `perf/regressions`, outil MCP `get_regressions` (nouveau), lignes à la fin de
    `timonier test`, liste dans l'onglet Évolution, bloc géré d'`AGENTS.md` ;
  - critère : `the_mann_whitney_test_gives_scipy_s_values_on_known_datasets` et
    `a_function_slowed_on_purpose_goes_from_probable_to_confirmed` (fixture : un lot de
    référence sur `main`, puis `f` ralentie de 40 ms : « probable », puis « confirmée » au lot
    suivant ; la durée du test, inchangée, n'en est pas une ; stable sur 10 passages).
- T-110 (ajoutée le 2026-10-21) : le serveur MCP se reconnecte à un démon relancé. Le relais
  sait sa connexion morte ; l'appel suivant se reconnecte, relance le démon par le CLI s'il ne
  répond pas, et refait la poignée de main ; un appel coupé en cours n'est pas rejoué (une
  écriture a pu se faire), l'erreur le dit. Test : `the_proxy_reaches_a_daemon_that_went_again`.
- T-111 (ajoutée le 2026-10-21) : le hook d'arrêt ne compte plus comme du code changé ce que le
  commit regroupe. Cause, lue dans `graph.sqlite` au commit de la Phase 7 (snapshot 29) : le
  regroupement en composants fait à chaque commit (ADR 0016) change le parent de 24 fichiers,
  sans rien changer de leur contenu ni de leurs arêtes ; `changed_files` les comptait. Un fichier
  dont seul le parent change n'y compte plus ; la dette de relecture (ADR 0030), qui lit la même
  fonction, cesse aussi de les marquer « untracked ». Tests :
  `files_only_regrouped_into_another_component_are_not_changed` (échoue sans le correctif) et
  `work_committed_after_its_session_ended_holds_no_agent` (démon réel, commit après la session,
  puis un changement tardif toujours retenu).
- T-114 (ajoutée le 2026-10-21, à la demande de l'humain) : la carte dit ce qu'est un symbole.
  Couleur de sa sorte (celle de la vue « Structure » de l'IDE), icône et mot à la place du rôle,
  signature en police de code, et l'infobulle « Champ : pub size: u32 » ; `ViewNode.signature`
  porte la signature du pack (ADR 0054). Tests : `a_symbol_of_the_map_says_its_kind_and_signature`
  (démon), « a symbol's box » (vitest), `test:a11y` (une boîte de chaque sorte dans la galerie,
  six thèmes).
- T-112 (ajoutée le 2026-10-21, à la demande de l'humain) : README lisible sur GitHub, avec une
  capture en anglais de chaque fonctionnalité. `README.md` (anglais, la référence) : but en une
  phrase, le problème, une visite par fonctionnalité avec sa capture, le fonctionnement, l'état,
  essayer, développer ; `README.fr.md` le traduit, mêmes sections et mêmes images, liens de
  langue en tête ; la liste détaillée d'avant passe dans `docs/features.md`. Les 12 captures de
  `docs/images/` sont prises par Playwright (`npm run screenshots -w webview`,
  `webview/test/screenshots/readme.shots.ts`, projet à part sous `SHOTS=1`) : la page de
  développement avec ses données d'exemple pour la carte, le suivi et le panneau Performances,
  le build de production pour le tableau blanc, thème Dark Modern. Skill `readme` pour les
  réécritures suivantes.
- 8.10 Relecture par `phase-reviewer` — critère : verdict « prête ».

Reporté de la Phase 4 (Écarts) : le coût de Timonier lui-même sur un grand dépôt, mesuré par
les relectures n° 15 et 16 sur des dépôts synthétiques (build release, tmpfs) :
- consolidation d'un commit : 39 à 50 s à 40 000 fichiers, environ 2 s à 10 000, surtout pour
  le recalcul des composants ; le fil du pipeline est bloqué pendant ce temps ;
- fichier créé ou supprimé, ou fichier d'exclusion qui fait entrer ou sortir des fichiers :
  environ 2 s à 10 000 fichiers, coût quadratique (1,8 s dans `Indexer::refresh`) ;
- hypothèse non mesurée, à vérifier sous macOS (H8) : surveiller le haut d'un monorepo y fait
  filtrer dans le processus les événements de tout le dépôt (notify 8.2, FSEvents ; ADR 0023,
  relecture n° 17, m17-7).

Critère de sortie (validé le 2026-10-21, Q71) : `timonier test` sur Timonier produit une timeline par
test, avec des spans rattachés aux nœuds du graphe ; une régression provoquée est détectée ;
un budget dépassé apparaît en rouge.

Vérification du critère de sortie sur Timonier (2026-10-21, T-113) :
- timeline par test : `tim test` (cargo-nextest 0.9.149, Apache-2.0 OR MIT, installé ce jour)
  passe les 540 tests en 13,0 s (run R-2, 2,4 Go au plus pour tous les processus) ; 585 spans
  reçus, tous rattachés à leur test et à un nœud du graphe (571 par le nom, 14 par l'id), sur
  les 19 tests qui lancent le binaire `timonier` (les autres n'exécutent pas de code instrumenté
  dans un processus qui exporte) ; `get_instrumentation_gaps` : 0 span sans `code.*`, 0 sans
  nœud ;
- régression provoquée, dans un clone jetable (scratchpad, commits locaux non signés, jamais
  poussés) : un lot de référence de 5 runs sur `main`, puis un commit qui ajoute 40 ms au
  début d'`index_all`, sur `cargo nextest run --profile timonier -p timonier-cli --test mcp` :
  « probable » au premier lot (`index_all` +797,7 %, p = 0,0003), « confirmée » au second
  (+800,9 %) ; `request` et `call`, qui attendent l'indexation côté client, et la durée du test
  qui lance le plus de démons (+21 %) le sont aussi ;
- budget dépassé en rouge : à l'humain, qui seul dessine les budgets sur le tableau blanc (la
  vue est vérifiée par `test:a11y` et l'intégration, ADR 0052).

Décisions : ADR 0043 à 0054.
Écarts :
- Trouvé par le critère de sortie : la télémétrie de Timonier lui-même ne posait pas le test
  de nextest sur ses spans (ADR 0047), au contraire du module que le pack Rust écrit ; au
  premier run, aucun span réel n'était sous un test. `telemetry::test_attributes` copie
  désormais `NEXTEST_TEST_NAME` et `NEXTEST_BINARY_ID` dans la ressource, que les démons lancés
  par un test héritent (test `the_test_nextest_runs_names_the_spans`).
- T-111 : le regroupement en composants à chaque commit comptait comme du code changé sans
  session, pour le hook d'arrêt et la dette de relecture (détail ci-dessus).
- ADR 0047 : pas de fichier de mise en place pour vitest et `node --test` ; un span TypeScript se
  rattache à son test seulement si le code de test pose `timonier.test.name` (relecture n° 1, I4).
- Un run sur un arbre de travail modifié reste hors de la référence des régressions (Q77) : les
  sorties des tests doivent être ignorées par git.

## Phase 9 — Fin du MVP

Statut : à faire

- 9.1 Inventaire des technologies, exposé par l'outil MCP `get_tech_inventory` — critère :
  sur Timonier, un SBOM valide ; les doublons d'une fixture sont détectés. Contenu :
  - lecture des manifestes (`Cargo.toml`, `package.json` ; `pyproject.toml` et `go.mod` si le
    temps le permet) ;
  - SBOM CycloneDX (cyclonedx-bom 0.8) : langages, frameworks, dépendances, versions,
    licences ;
  - CVE connues (source fixée par ADR, accès réseau) ;
  - doublons fonctionnels, et dépendances ajoutées depuis la dernière revue.

  Fait (2026-10-21), ADR 0055 et Q79 :
  - `[inventory]` des packs : frameworks, et fonctions précises (`http-client`, `async-runtime`,
    `cli-parser`, `date-time`…) qui font les doublons ;
  - démon (`inventory.rs`) : `cargo metadata --offline`, sinon `Cargo.lock` seul (licences
    inconnues), `package-lock.json` v2 et v3 avec les dépendances de chaque workspace ; langages
    du graphe ; ajoutées depuis la revue par les arêtes `DependsOn` du diff ;
  - CVE (`osv.rs`) : OSV.dev à la demande, 24 h de cache dans `.timonier/cache/` (hors de git),
    « non vérifiées » sans réponse, `cve: false` et `TIMONIER_OFFLINE=1` ;
  - SBOM (`sbom.rs`) : CycloneDX 1.5 par `cyclonedx-bom`, validé avant d'être rendu ;
    `inventory/get`, `inventory/sbom`, outil `get_tech_inventory`, `timonier inventory [--sbom
    FICHIER] [--no-cve]`, ligne du bloc géré ;
  - critère : sur Timonier, 840 dépendances (318 cargo, 522 npm), le SBOM (828 composants, 3
    vulnérabilités d'outils de développement npm : `diff` 7.0.0, `serialize-javascript` 6.0.2)
    passe le schéma officiel CycloneDX 1.5 sans erreur (jsonschema, hors du projet) ; doublon de
    la fixture : `the_inventory_finds_two_http_clients_and_gives_a_cyclonedx_sbom`.
- T-115 (ajoutée le 2026-10-21, à la demande de l'humain) : une barre de chargement au
  démarrage. L'indexeur compte les fichiers lus d'une indexation complète ; `status` les donne
  (`progress`) jusqu'à `indexed` ; l'extension les demande toutes les 300 ms après la connexion
  et les passe aux pages dans le message `daemon` ; la carte et les panneaux montrent « Fichiers
  lus : 120 sur 247 » dans une barre native, sans animation, puis « écriture du graphe ». Tests :
  vitest (carte), test de l'indexeur, galerie du système de conception (`test:a11y`).
- T-116 (ajoutée le 2026-10-21) : l'audit de sécurité que l'humain a fourni (rapport hors du
  dépôt, non versionné ; aucun constat critique ni élevé), ADR 0057 :
  - constat 1 (Q84) : les commandes de `tests.json` ne partent que si ce poste les a approuvées
    (`approval.rs`, `--approve`, sortie 2) ; `init` approuve ce qu'il écrit ;
  - constat 2 : un jeton par run, donné aux tests dans `OTEL_EXPORTER_OTLP_HEADERS`, exigé par le
    récepteur ;
  - constat 3 : 200 000 points d'encre au plus, et `intent.json` de plus de 16 Mio refusé sans être
    lu ;
  - constat 5 : le relais LSP refuse un chemin hors de la racine ; constat 6 : 10 000 boîtes au plus
    par vue ;
  - constats 4, 7, 8, 10 : gardés et dits dans l'ADR ; constat 9 (permissions locales de Claude
    Code de l'humain) : à l'humain.
  Tests : `a_command_is_approved_for_its_project_only`, le refus dans
  `without_a_command_the_recorded_runners_run_and_rusage_measures_alone`,
  `the_receiver_takes_a_run_s_spans_with_its_token_only`, `too_much_ink_is_refused`,
  `a_view_that_opens_too_many_boxes_is_refused`.
- T-117 (ajoutée le 2026-10-21, à la demande de l'humain) : un bouton « Tableau blanc » dans le
  pont, et l'icône du tableau blanc dans la barre de titre de la vue « Overview ».
- T-118 (ajoutée le 2026-10-21, à la demande de l'humain, révise Q53) : configurer depuis le pont
  sans console. `timonier init --detect` dit en JSON les quatre agents et ceux trouvés ; le pont
  les montre en cases à cocher (cibles de 24 px, WCAG 2.5.8), et « Configurer » lance `init --yes
  --agents …` en arrière-plan (`setup.ts`), dit l'issue dans le pont et une notification. Tests :
  `init --detect` (CLI), `setup.test.ts` (faux `timonier`), vitest du pont, `test:a11y` de l'offre
  au clavier.
- 9.2 Processus de développement — critère : le scénario « Solo discipliné » fonctionne de
  bout en bout sur un dépôt de test. Contenu :
  - préréglages Solo rapide, Solo discipliné et Équipe ; `.timonier/workflow.json` ;
    panneau de réglages ; bloc géré dérivé du processus ;
  - GitHub via `gh` : un ticket par tâche, branche `tim/<ticket>-<slug>`, PR en fin de
    session avec une description générée ;
  - génération du workflow de CI ;
  - outils MCP `get_workflow`, `task_start`, `open_pull_request` et `link_ticket` ;
  - jetons dans le trousseau de l'IDE.

  Fait (2026-10-21), ADR 0056, Q80 à Q83 :
  - `timonier-core::workflow` : `.timonier/workflow.json` (schéma dans `docs/schemas/`), les trois
    préréglages, `custom` dès qu'un réglage s'en écarte, ce qui ne tient pas ensemble, les
    consignes de l'agent, le nom de branche `tim/{ticket}-{slug}`, la description de la PR ;
  - démon : `forge.rs` (`gh` et `git`, le programme réglable : `Options::gh`), `process.rs`
    (`workflow/get`, `workflow/set`), le champ `ticket` d'une tâche, la PR à la fin de session,
    l'issue fermée avec sa tâche ; `.github/workflows/timonier.yml` écrit quand la CI est GitHub
    Actions (jamais par-dessus un fichier changé à la main) ;
  - outils `get_workflow`, `task_start`, `open_pull_request`, `link_ticket` ; une ligne fixe du
    bloc géré renvoie à `get_workflow` ;
  - `timonier check` : échoue sur un budget dépassé ou une règle d'architecture enfreinte ;
  - VSCode : « Processus de développement » (préréglages et réglages, page `workflow` du build du
    suivi), `test:a11y` dans six thèmes et au clavier, test d'intégration ;
  - jetons : la connexion de `gh` (Q81), aucun jeton gardé ;
  - critère : « Solo discipliné » de bout en bout sur un démon réel, un faux `gh` et un dépôt
    distant local (`solo_disciplined_opens_the_issue_the_branch_and_the_pull_request`) ; le
    scénario sur un vrai dépôt GitHub attend ton feu vert (Q80).
- 9.3 Coût de l'IA, selon Q7 : la télémétrie OpenTelemetry native de Claude Code est
  pointée par init vers le récepteur, puis les tokens sont comptés par session et par tâche.
  Sans source, la valeur est « non mesuré », jamais une estimation inventée — critère : une
  session réelle donne des tokens attribués.
- 9.4 Assistant « Nouveau projet piloté », selon Q8 (en Phase 9) — critère : depuis VSCode, un
  nouveau projet Rust ou TypeScript est créé par le générateur officiel, configuré par `init`,
  et `doctor` voit sa première trace rattachée au graphe.
- 9.5 Relecture complète du MVP par `phase-reviewer` contre la section « Découpage » —
  critère : verdict « prête ».
- 9.6 Critère de sortie du MVP selon la spec : un projet réel, nouveau (Q9), créé par
  l'assistant de 9.4 et piloté de bout en bout
  sans lire le code ligne à ligne — critère : constaté avec toi.
- 9.7 Publication de l'extension, selon Q58 et Q59, avant la relecture 9.5 : un `.vsix` par
  plateforme qui embarque `timonier`, construit par la CI ; une icône PNG et un `CHANGELOG.md` ;
  le dépôt public (Q2) ; l'éditeur `NicolasMichel` (Q12) — critère : l'extension installée depuis
  la Marketplace configure un projet et montre sa carte, sans rien installer à part.

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
| rmcp | 3.5.1 | Apache-2.0 (parties en MIT) | SDK Rust officiel de MCP ; revérifié le 2026-10-20 (Rust 1.88 au moins ; la 3.6.0 du jour attend), verrouillé dans `Cargo.lock` (ADR 0027) |
| schemars | 1.2.3 | MIT | Vérifié le 2026-10-20 ; schémas JSON des entrées des outils MCP (ADR 0027) |
| tonic / prost / axum | 0.14.6 / 0.14.4 / 0.8.9 | MIT / Apache-2.0 / MIT | |
| clap | 4.6.7 | MIT OR Apache-2.0 | |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 | |
| graphrs | 0.12.0 | MIT | **Écarté** (ADR 0016) : son Leiden ne part pas d'une partition initiale |
| quick-junit | 0.8.0 | Apache-2.0 OR MIT | Revérifiée le 2026-10-21 : sa lecture est « expérimentale » et refuse une racine `<testsuite>` seule ; écrire seulement |
| junit-parser | 1.7.0 | BSD-2-Clause | 2026-10-21 : lit `testsuites`, `testsuite` seul et `testrun` ; `file` et `line` par cas (8.2) |
| perf-event | 0.4.9 | MIT OR Apache-2.0 | 2026-10-21 : compteurs avec `inherit` et `enable_on_exec` ; refusé si `perf_event_paranoid` > 2 (Q70) |
| nix / wait4 | 0.31.3 / 0.2.0 | MIT / MIT | 2026-10-21 : `getrusage` sûr (nix) ; `wait4` par enfant (crate wait4, MSRV 1.95) ; le `maxrss` de rusage est suspect |
| libproc | 0.14.11 | MIT | 2026-10-21 : macOS, `pidrusage::<RUsageInfoV4>` (instructions et cycles d'un processus, sans ses enfants) ; non essayé sur un Mac |
| tonic / opentelemetry-proto / axum | 0.14.6 / 0.33.1 / 0.8.9 | MIT / Apache-2.0 / MIT | 2026-10-21 : récepteur (ADR 0048) ; un `Routes` de tonic dans un routeur axum sert gRPC et HTTP sur un port |
| opentelemetry, _sdk, -otlp | 0.33.1 | Apache-2.0 | 2026-10-21 : `-otlp` sans ses features par défaut, avec `grpc-tonic` ; en test seulement pour l'instant (8.6 : la feature `otel`) |
| cyclonedx-bom | 0.8.1 | Apache-2.0 | 2026-10-21 : SBOM CycloneDX 1.5 de l'inventaire (ADR 0055) ; `Bom`, `Component::new`, `Purl::new`, `SpdxExpression::parse_lax`, `Validate`, `output_as_json` lus dans ses sources |
| ureq | 3.4.2 | MIT OR Apache-2.0 | 2026-10-21 : client HTTPS bloquant pour OSV.dev, à la demande (Q79, ADR 0055) ; rustls par défaut |
| OSV.dev (API v1) | — | données CC-BY 4.0 | 2026-10-21 : `POST /v1/querybatch`, `GET /v1/vulns/{id}` essayés (crates.io et npm) |
| @opentelemetry/sdk-node / api | 0.223.0 / 1.9.1 | Apache-2.0 | 2026-10-21 : ajoutés par `init` (ADR 0050) ; sdk-node apporte les exporteurs OTLP et lit `OTEL_SDK_DISABLED` ; la ligne 0.300 (SDK 3.0, 2026-10-20) attend |
| tracing-opentelemetry | 0.34.0 | MIT | 2026-10-21 : feature `otel` du CLI (ADR 0044), sans features par défaut |
| statrs | 0.19.1 | MIT | 2026-10-21 : son Mann-Whitney exact est faux (queues inversées, essayé) ; à écrire nous-mêmes |
| cyclonedx-bom | 0.8.1 | Apache-2.0 | |
| scip | 0.10.0 | Apache-2.0 | v2 ; dépend de rust-protobuf, pas de prost |
| procfs / cgroups-rs | 0.18.0 / 0.5.1 | MIT OR Apache-2.0 | |
| insta / proptest / tempfile / assert_cmd | 1.49.0 / 1.12.0 / 3.27.0 / 2.2.2 | Apache-2.0 / MIT OR Apache-2.0 | |
| similar | 2.7.0 | Apache-2.0 | Vérifié le 2026-10-20 (la 3.2.0 existe) ; déjà dans le dépôt par insta, utilisé pour le diff de `timonier init --dry-run` (ADR 0033) |
| @xyflow/react | 12.12.0 | MIT | Pas `reactflow` (figé). Revérifié et utilisé le 2026-10-16 (ADR 0021) ; zustand, classcat, @xyflow/system, use-sync-external-store en MIT |
| d3-drag, -zoom, -selection, -transition, -interpolate, -color, -dispatch, -timer / d3-ease | 3.x | ISC / BSD-3-Clause | Vérifiés le 2026-10-16 ; tirés par @xyflow/system, empaquetés dans la webview avec leur notice |
| scheduler | 0.28.0 | MIT | Vérifié le 2026-10-16 ; tiré par react-dom |
| elkjs | 0.12.0 | EPL-2.0 OR GPL-3.0-or-later | Utilisé sous EPL-2.0, sans worker (ADR 0021) ; la notice lie le paquet de cette version |
| tldraw | 5.5.2 | Propriétaire, avec clé | **Écarté** (Q3) |
| tiny-skia | 0.12.0 | BSD-3-Clause | Vérifié le 2026-10-21 ; dans le démon, image du croquis pour l'agent (ADR 0040) ; apporte png 0.18.1 (MIT OR Apache-2.0), flate2, miniz_oxide, zlib-rs (Zlib), arrayref (BSD-2-Clause), bytemuck, strict-num |
| perfect-freehand | 1.2.3 | MIT | Vérifié le 2026-10-20 ; sans dépendance, 112 ko ; dessin libre du tableau blanc (ADR 0036) |
| @excalidraw/excalidraw | 0.18.1 | MIT | Vérifié le 2026-10-20 ; **écarté** (ADR 0036, Q60) |
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
- 2026-10-18 — Phase 5 : « Marquer comme revu », diff de revue sur la carte (badges, résumé,
  filtre), snapshots de commit avec les paquets du commit (Q23). Tes réponses : Q26 à Q29.
  Relecture n° 1 « pas prête » (1 bloquant, 4 importants, 12 mineurs) ; tes réponses Q30 à Q32 ;
  tout est corrigé, densité réglable comprise. Le test des cinq secondes attend ta réponse.
- 2026-10-19 — Phase 5, relecture n° 2 « pas prête » (1 important, 8 mineurs) ; tes réponses
  Q33 (une boîte n'est nouvelle ou supprimée que si du code arrive ou part) et Q34 (une crate
  sans manifeste est modifiée) ; tout est corrigé.
- 2026-10-19 — Phase 5, relecture n° 3 « pas prête » (2 importants, 3 mineurs) ; ta réponse
  Q35 (une crate vient et part avec son manifeste) ; tout est corrigé.
- 2026-10-19 — Phase 5, relecture n° 4 « pas prête » (2 importants, 2 mineurs) ; tes réponses
  Q36 (comparer les arêtes) et Q37 (la crate qui cède ou reçoit les fichiers est modifiée) ;
  tout est corrigé.
- 2026-10-19 — Phase 5, relecture n° 5 « pas prête » (1 important, 3 mineurs) ; tes réponses
  Q38 (les boîtes que le fichier quitte et rejoint) et Q39 (cas limite noté comme écart) ;
  tout est corrigé.
- 2026-10-19 — Phase 5, relecture n° 6 « pas prête » (1 important, 4 mineurs) : une crate
  renommée dans un grand espace de travail faisait lister de fausses dépendances ; corrigé,
  avec les mineurs.
- 2026-10-19 — Phase 5, relecture n° 7 « pas prête » (1 important, 2 mineurs) ; tes réponses
  Q40 (un fichier renommé dans son dossier ne bouge pas), Q41 (les deux choix de l'ADR 0024
  gardés) et Q42 (clore avec réserves si la relecture n° 8 ne trouve plus que des mineurs) ;
  tout est corrigé.
- 2026-10-19 — Phase 5, relecture n° 8 : ni bloquant ni important, 4 mineurs (3 corrigés, 1
  noté en écart). « Prête avec réserves » une fois le test des cinq secondes passé (Q42) ;
  5.5 cochée. Reste 5.3 : ton test des cinq secondes.
- 2026-10-20 — Test des cinq secondes n° 1 échoué (marques lues comme des actions, résumé pas
  vu) ; corrigé : marques pleines, « 2 modifiés », bandeau en haut, bouton principal, légende
  des marques. Phase 5 validée par toi (« commit et push et passe à la suite ») : ADR 0024 à
  0026 acceptées ; 5.5 close avec réserves (Q42) ; Phase 6 commencée.
- 2026-10-20 — Phase 6 : MCP (proxy et outils de suivi), non suivi et dette de revue, vue « Plan,
  todo et sessions », pont, `timonier init` / `uninit` / `doctor`, volet init des packs, détection
  dans VSCode ; bascule de Timonier sur lui-même (plan dans `plan.json`, sessions par MCP).
  Quatre relectures, la dernière « prête » ; ADR 0027 à 0035 acceptées par toi ; commit
  `ae83bb7` poussé, CI verte. Phase terminée, en attente de ton feu vert pour la Phase 7.
- 2026-10-20 — Phase 7 commencée (ton « en avant ») : 7.0 faite, ADR 0036 acceptée (Q60 : React
  Flow et perfect-freehand), critère de sortie validé (Q61) ; 7.1 faite, ADR 0037 proposée :
  tableau blanc dans sa propre page, testé sur le build de production sous la CSP.
- 2026-10-20 — 7.2 faite (ton « passe à la suite ») : Q62 à Q65 répondues, ADR 0038 proposée ;
  formes typées au pointeur et au clavier, sérialisation testée.
- 2026-10-20 — 7.3 faite : ta demande « le dessin doit forcément être persistant » (Q66) ;
  `intent.json` écrit après chaque geste par le démon, relu à l'ouverture, ADR 0039 proposée.
- 2026-10-21 — 7.4 faite : Q67 répondue (une image et des emplacements), ADR 0040 proposée.
- 2026-10-21 — 7.5 faite : Q68 répondue (ancrage par nom souple, puis emplacement), ADR 0041
  proposée ; `get_intended_architecture` et `get_drift` par MCP.
- 2026-10-21 — 7.6 faite : la carte comparée à l'intention, ADR 0042 proposée.
- 2026-10-21 — Phase 7, relecture n° 1 « prête avec réserves » (5 importants, 8 mineurs) ; ta
  réponse Q69 (IDs nommés acceptés en lecture) ; corrigé, sauf ce qui demande tes mains (dessiner
  dans ce dépôt, tests des cinq secondes). Critère de sortie joué sur une copie, captures envoyées.
- 2026-10-21 — Phase 7, relecture n° 2 « prête avec réserves » (2 importants, 6 mineurs :
  l'image écrivait mal les IDs nommés, une carte de rôle coupait son dernier membre) ; corrigé.
- 2026-10-21 — Phase 7, relecture n° 3 « prête avec réserves » (1 important : la marque d'une
  boîte fermée disparaissait avec la dette ; 8 mineurs) ; la marque devient une icône de barre de
  titre ; corrigé.
- 2026-10-21 — Phase 7, relecture n° 4 « prête avec réserves » (1 important : le lien interdit
  épaissi contredisait l'ADR 0021 ; 7 mineurs) ; corrigé, deux écarts notés.
- 2026-10-21 — Phase 7, relecture n° 5 « prête avec réserves » (1 important : `daemon/shutdown`
  sans poignée de main non noté en ADR ; 4 mineurs) ; corrigé.
- 2026-10-21 — Phase 7, relecture n° 6 « prête avec réserves » (1 important : le mot
  « interdite » restait caché sous les boîtes ou l'étiquette du lien inverse ; 2 mineurs) ;
  corrigé.
- 2026-10-21 — Phase 7, relecture n° 7 « prête avec réserves » (1 important : le mot
  « interdite » posé sur une partie cachée de son lien, contre une barre de titre ; 3 mineurs) ;
  corrigé.
- 2026-10-21 — Phase 7 validée par toi (« commit et push et passe à la suite ») : ADR 0037 à 0042
  acceptées ; Phase 8 commencée.
- 2026-10-21 — CI macOS rouge sur c7f59f0 (`shutdown_closes_the_other_clients`) : macOS refuse
  le délai de lecture (EINVAL) sur un socket que le démon a déjà fermé, sa réponse encore à
  lire ; le client bloquant lit quand même (un socket fermé ne bloque pas). Corrigé sans session
  Timonier (le serveur MCP de la session ne se reconnecte pas au démon relancé).
- 2026-10-21 — Phase 8, relecture n° 1 « pas prête » (7 importants, 11 mineurs, aucun bloquant).
  Trois questions posées et répondues (Q76 temps cumulé sur la carte, Q77 runs sur un arbre
  modifié, Q78 `@opentelemetry/api` en `dependencies`). Corrigé :
  - I1 : l'évolution est la médiane des runs de chaque commit, pour la commande du run affiché ;
  - I2 : test en échec et span en erreur en icône et en mot ;
  - I3 : une seule origine du temps pour les spans et les échantillons (`rebase_samples`) ;
  - I4 : table `aggregate` (migration `perf/0002`), lue au-delà de la rétention ; l'ADR 0047
    corrigée pour TypeScript (pas de fichier de mise en place, Q72) ;
  - I5 : test d'intégration VSCode sur un vrai run et un budget dépassé (émetteur OTLP/HTTP JSON) ;
  - I6 (Q77) : colonne `worktree`, hors référence, confirmation à empreinte égale ;
  - I7 (Q76) : `enteredNs` par nœud et `ns` par lien, temps cumulé sur la boîte ;
  - m1 et m2 (note des régressions traduite, lot d'un run trop petit), m3 (README), m4 (titres en
    français), m5 (budgets sur la commande du dernier run), m6 (Q78), m8 (CPU du scope par
    `cpu.stat`), m9 (ligne du test de fumée TypeScript), m10 (budgets dépassés et trous dits à
    la fin de `timonier test`), m11 (jetons) ; m7 noté dans l'ADR 0053.
- 2026-10-21 — Phase 8, relecture n° 2 « prête avec réserves » (1 important, 4 mineurs). Corrigé :
  - I-1 : le correctif de m5 faisait juger les budgets sur la commande du dernier run, test de
    fumée compris ; les runs de fumée sont marqués (`smoke`, migration `perf/0003`) et laissés
    hors des budgets et des vues par défaut, et chaque budget prend les 5 derniers runs qui ont
    atteint ce qu'il vise (test : un run qui n'atteint pas la crate laisse le budget dépassé) ;
  - m-1 (ADR 0043 : l'évolution lit les 50 derniers résumés), m-2 (test de la confirmation à
    empreinte d'arbre égale), m-3 (titres des commandes « (activer ou non) », « panneau »), m-4
    (commentaire d'`evolution`).
- 2026-10-21 — Phase 8, relecture n° 3 « prête avec réserves » (2 importants, 3 mineurs). Corrigé :
  - I-1 : le calque de la carte et les budgets de l'extension ne prennent plus un test de fumée
    pour le dernier run (le calque se redemandait toutes les 3 s) ; test node:test ;
  - I-2 : le panneau ouvre sur le dernier run hors fumée, et la liste marque la fumée en mot ;
  - m-1 (tests : le choix des runs par budget, le run de `doctor` marqué `smoke`), m-2 (coût de
    `perf/budgets` noté dans l'ADR 0052, non mesuré), m-3 (ADR 0043, 0049, 0051, PLAN.md 8.8,
    descriptions des outils MCP régénérées).
- 2026-10-21 — Phase 8, relecture n° 4 « prête avec réserves » (7 mineurs, aucun important).
  Corrigé : tests du run par défaut du panneau et du mot « fumée » (m-1), d'un run de fumée pour
  les budgets de la carte (m-2) ; ADR 0049 et description de `get_perf` (m-3, m-4) ; le calque
  ne se redemande plus à chaque tick quand aucun run listé n'a de traces (m-5, test) ; mise en
  forme de 8.8 (m-7). m-6, un échec isolé de `test:a11y` : non reproduit en 3 passages complets
  répétés (537 sur 537), test non identifié.
- 2026-10-21 — Phase 8, relecture n° 5 « prête avec réserves » (1 mineur) : après un seul échec
  de `perf/map` ou de `perf/budgets`, la carte ne les redemandait plus avant le run suivant ;
  corrigé (l'échec laisse la prochaine relève redemander), tests node:test pour les deux.
- 2026-10-21 — Phase 8, relecture n° 6 : aucun constat (ni bloquant, ni important, ni mineur) ;
  « prête avec réserves » pour les seules vérifications laissées à l'humain : le test des cinq
  secondes (8.7) et le budget en rouge sur Timonier (T-113). Le correctif de la relecture n° 5
  tient : un mutant sans lui fait échouer ses deux tests, et les relèves restent bornées.
- 2026-10-21 — Trouvé par l'humain au test de 8.7 : le `.vsix` n'embarquait pas la page
  Performances (`.vscodeignore` liste les fichiers à garder page par page ; les tests
  d'intégration tournent sur `dist/`, pas sur le paquet). Ajoutée ; le test du manifeste déduit
  désormais les pages des fichiers HTML de la webview au lieu de les nommer.
- 2026-10-21 — Phase 8 validée par toi (« oui je valide ») : ADR 0043 à 0054 acceptées ; 8.7 et
  T-113 closes sur ta validation, après tes propres essais des vues (les réponses du test des
  cinq secondes ne sont pas consignées). Pas encore de commit.
- 2026-10-21 — Trouvé par l'humain, sur un dossier sans Rust ni TypeScript (un jeu, un seul
  fichier Lua) : la carte disait « aucun fichier source qu'il sait lire pour l'instant », comme
  s'il fallait attendre. Elle dit désormais quels langages Timonier lit, et que la carte
  apparaît dès qu'il en trouve.
