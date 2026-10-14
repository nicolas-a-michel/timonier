Timonier — spec du plugin de pilotage du développement par IA
14 oct. 2026 · @Nicolas
Vision et problème
Le plugin rend le développeur capable de piloter un agent IA sans lire tout le code qu'il produit. Quand l'IA écrit le code, le goulot n'est plus la production mais la compréhension : le code évolue plus vite que le modèle mental du développeur, qui accumule une dette de compréhension.
Le développeur devient architecte et relecteur. Il lui faut en permanence quatre réponses :
• Ce que je veux : l'architecture voulue, dessinée librement.
• Ce qui existe : l'architecture réelle, les technologies, la santé du code.
• Comment ça se comporte : temps d'exécution et ressources, mesurés pendant les tests.
• Ce qui se passe : le plan, la todo en direct, ce que l'agent a fait et où regarder.
Cible : n'importe quel langage, usage solo d'abord, équipe ensuite.
Nom : Timonier. Le timonier tient la barre et garde le cap pendant que l'équipage, ici les agents, fait avancer le navire. Commande CLI timonier, alias court tim.
Disponibilité vérifiée le 14 octobre 2026 :
• crate timonier sur crates.io et paquet timonier sur npm : libres, à réserver ;
• marketplace VSCode : aucune extension trouvée, à confirmer ;
• organisation GitHub timonier : prise (images Docker inactives depuis plus de cinq ans), prévoir timonier-dev ou équivalent ;
• homonyme mineur : un paquet Emacs de gestion Kubernetes ;
• domaines (.dev, .io, .fr) : à vérifier chez un registrar.
Architecture technique
Un démon Rust local, un par projet, porte tout le modèle ; les front-ends restent fins et interchangeables.
Le graphe unifié est la seule source de vérité : l'IDE et l'agent lisent le même modèle, l'un par JSON-RPC, l'autre par MCP.
• VSCode : front-end principal du MVP, grâce à l'API webview (carte, tableau blanc tldraw, plan, timelines).
• Zed : hors périmètre pour l'instant, à reconsidérer plus tard.
• Agent IA : n'importe quel agent compatible MCP, configuré par timonier init.
Indexation et modèle de graphe
Aucun analyseur n'est écrit par langage : trois sources génériques sont fusionnées dans un graphe unifié, avec une provenance et une confiance par arête.
Source
Apport
Limite
Tree-sitter
Symboles, imports, définitions, ~200 langages
Résolution des appels approximative
LSP (déjà lancé par l'IDE)
Définitions, références, call hierarchy précises
Interrogation au fil de l'eau, coûteuse en masse
SCIP/LSIF si disponible
Graphe complet et précis
Batch, pas pour tous les langages
Schéma.
• Nœuds : Repo, Component, Package, File, Symbol (module, type, trait, fonction, méthode, champ, constante), avec nom, langage, span et parent.
• Arêtes : Imports, Calls, References, Implements, Inherits, DependsOn, avec provenance (TreeSitter, Lsp, Scip, Manifest, Inferred, Intended), confidence et weight.
• La hiérarchie (parent) est un arbre strict ; les relations forment un graphe libre. Les arêtes entre composants sont agrégées depuis les arêtes de symboles, jamais stockées à la main.
Identité stable. Format de symbole SCIP quand il existe, sinon lang:qualified_name. Les déplacements sont détectés au diff par similarité (signature + hash du corps normalisé), pour qu'un refactoring ne devienne pas « tout supprimé, tout recréé ».
Fusion. Une arête vue par plusieurs sources garde la provenance la plus fiable ; une arête tree-sitter contredite par le LSP est supprimée.
Composants. Agrégation par structure (crates, packages, dossiers), puis clustering du graphe (Louvain/Leiden). Le LLM nomme et résume les clusters, il ne les calcule pas. Chaque composant reçoit un rôle architectural (serveur ou point d'entrée, modèle de données, persistance, interface, tâche de fond, groupe de bibliothèques externes, utilitaire), déduit des frameworks et API utilisés (un TcpListener, un routeur HTTP, un ORM, des types sérialisés) puis confirmé par le LLM. Les dépendances externes sont regroupées par usage (réseau, sérialisation, base de données, async). La carte s'ouvre ainsi sur une vue très haut niveau, à la manière du modèle C4 : quelques boîtes nommées par leur rôle et reliées entre elles. Cliquer ou zoomer sur une boîte ouvre son contenu, lui-même fait de boîtes, et ainsi de suite jusqu'aux fichiers et symboles.
Stockage. SQLite mis à jour en continu. Chaque nœud et arête porte un intervalle valid_from/valid_to de snapshots ; une table snapshot(seq, commit_sha, timestamp, kind) avec kind ∈ {commit, review_mark, working_tree}. Le diff entre deux snapshots est une requête. .timonier/actual.json est régénéré au commit par un hook.
Tableau blanc
Le tableau blanc est à sens unique : le développeur dessine, l'IA lit. Rien ne revient dessus, ni fantômes, ni propositions, ni statuts calculés. Le travail de l'IA se voit dans la visualisation d'architecture.
Mode mixte. Dessin libre partout, interprété par un modèle vision et par le texte. Une forme peut être typée quand elle doit compter comme contrainte :
• boîte → composant voulu (nom, rôle) ;
• flèche → arête avec une politique allowed, forbidden ou required ;
• zone → couche ou frontière (domaine, infra, UI) ; une arête entre couches s'applique à tout leur contenu ;
• note → contrainte en texte, attachée à un élément.
Budgets de performance. Une note typée budget fixe une limite mesurable sur un composant ou un chemin : latence (p50, p95), mémoire max, allocations, nombre d'appels. Exemple : « Parser : p95 < 5 ms, < 50 Mo ». L'agent la lit comme une contrainte, le démon la vérifie à chaque run de tests.
Fichier .timonier/intent.json. Un seul auteur, le développeur. Sémantique et géométrie séparées (shape ne sert qu'au rendu), IDs locaux stables (C-…, E-…, L-…, N-…), versionné dans le repo.
{
  "version": 1,
  "layers": [{ "id": "L-domain", "name": "Domaine" }],
  "components": [{ "id": "C-parser", "name": "Parser", "layer": "L-domain",
                    "role": "Transforme les tokens en AST" }],
  "edges": [{ "id": "E-2", "src": "L-domain", "dst": "L-infra",
               "kind": "depends_on", "policy": "forbidden" }],
  "notes": [{ "id": "N-1", "text": "Aucune allocation dans la boucle chaude",
               "attached_to": "C-parser" }],
  "sketch": { }
}
Implémentation. tldraw dans la webview (formes custom, export JSON propre), plutôt qu'Excalidraw, moins adapté aux formes typées.
Visualisation d'architecture
La visualisation est générée uniquement depuis le code réel : c'est là que se voit le travail de l'IA.
• Carte des composants avec zoom composant → fichiers → symboles, et clic vers le code. Les arêtes sont épaissies selon le trafic mesuré.
• Diff depuis ma dernière revue : composants, dépendances et cycles ajoutés, composants dont le fan-in/fan-out a bondi, badge « modifié » sur les boîtes touchées par un refactoring interne. Un bouton « marquer comme revu » crée un snapshot review_mark.
• Comparer à l'intention (option d'affichage) : composants dessinés mis en évidence, composants manquants listés, arêtes interdites franchies signalées. L'ancrage boîte dessinée ↔ composant réel est calculé ici, par nom et emplacement, sans écrire dans le fichier d'intention.
• Voyage dans le temps : curseur sur les snapshots pour rejouer l'évolution.
• Calques activables sur la carte : performances, couverture de tests, dette de revue, santé, coût de l'IA.
Précision affichée : une arête issue de tree-sitter seul est rendue différemment d'une arête confirmée par LSP ou SCIP.
Animation des flux de données. Des particules circulent le long des arêtes de la carte pour montrer comment les données traversent le logiciel.
• Source réelle : les traces OpenTelemetry des tests. Le flux animé est ce qui s'est vraiment passé, pas une hypothèse.
• Encodage : densité et vitesse des particules selon le débit d'appels, taille selon le volume transporté, teinte selon le type de donnée (issu des signatures LSP). Les attributs timonier.payload.bytes et timonier.payload.type s'ajoutent aux conventions d'instrumentation pour les appels internes.
• Deux modes : vue agrégée (le trafic d'ensemble d'un run de tests) et rejeu d'une trace unique (le chemin d'une requête ou d'un test, étape par étape).
• Contrôles : lecture, pause, vitesse, curseur temporel synchronisé avec la timeline des performances ; clic sur une particule pour ouvrir le span et le code correspondants.
• Repères : goulots d'étranglement visibles comme des accumulations de particules, erreurs signalées par un éclat bref sur le nœud concerné.
Fidèle aux principes de design : mouvement lent et doux, couleurs sourdes, animation désactivée par défaut pour les utilisateurs qui ont activé la réduction des mouvements, avec une vue statique équivalente (arêtes épaissies, chiffres de débit).
Performances
Les graphes de performances se remplissent automatiquement pendant les tests, avec une instrumentation OpenTelemetry explicite (niveau 3) dès le MVP. Pas de lib propriétaire : si le plugin disparaît, l'instrumentation reste valable.
Niveau
Mécanisme
Coopération de l'IA
1
timonier test enveloppe le runner : durée, CPU, mémoire max, I/O via cgroups v2 / procfs ; durée par test via JUnit XML
Aucune
2
Profilage par échantillonnage (perf/eBPF, py-spy, async-profiler), symboles rattachés au graphe
Aucune
3
Récepteur OTLP local (gRPC 4317 / HTTP 4318), spans et métriques via le SDK OTel du langage
Obligatoire
Le niveau 1 tourne toujours en fond comme référence ; le niveau 2 sert de repli pour un langage sans SDK OTel mature.
Branchement. timonier test injecte OTEL_EXPORTER_OTLP_ENDPOINT et OTEL_RESOURCE_ATTRIBUTES=timonier.run_id=…,timonier.snapshot=…. Chaque test devient un span racine.
Conventions imposées à l'IA.
• Un span par frontière de composant (fonction publique d'un module).
• Attributs obligatoires code.function.name, code.file.path, code.line.number, pour le rattachement au graphe.
• Pas de span dans les boucles chaudes : histogrammes et compteurs à la place.
• Instrumentation désactivable à la compilation ou par configuration (en Rust : tracing + tracing-opentelemetry derrière une feature).
Vérification après chaque run. Fonctions publiques jamais vues dans les traces, spans sans attributs code.*, surcoût estimé contre le niveau 1. Les écarts sont signalés et lisibles par l'agent via MCP.
Budgets. Chaque note budget du tableau blanc est comparée aux mesures du run (médiane sur plusieurs runs). Un dépassement apparaît en rouge sur la carte et dans le journal de la session, et l'agent le récupère via get_budget_violations.
Vues. Timeline par test (spans emboités, ressources du processus dessous), carte annotée (temps cumulé, nombre d'appels), évolution par commit (médiane sur plusieurs runs, régression au-delà d'un seuil).
Pilotage
Le plan et la todo sont le fil conducteur : visibles en permanence, ils relient chaque tâche au code, aux tests et aux performances qu'elle a produits.
• Plan : jalons → tâches, chaque tâche rattachée à des composants du tableau blanc. Panneau latéral permanent, avancement calculé.
• Todo : tâches de la session en cours, qui se remplissent quand l'agent découpe le travail et se vident au fil de l'exécution, en direct.
• Journal des sessions : pour chaque session d'agent, objectif, tâches, composants touchés, diff structurel, tests et effet sur les performances. Répond à « pourquoi ce module existe ? » des semaines plus tard.
• Dette de revue : chaleur par composant selon la quantité de code écrit par l'IA et jamais relu. Les changements faits hors session sont marqués « non suivis ».
• Couverture de tests par composant, en calque sur la carte.
• Analyse d'impact : sélection d'un composant → tout ce qui en dépend, pour cadrer un prompt avant de lancer l'agent.
• Coût de l'IA : tokens et argent par session et par composant, pour repérer où l'agent tourne en rond.
L'agent ne modifie jamais plan.json, todo.json ou le journal à la main : il passe par les outils MCP, qui valident chaque écriture.
Multi-agents. Plusieurs agents peuvent travailler en parallèle sur le même projet.
• Une voie par agent dans le plan et la todo, chaque session identifiée par son agent.
• Réservation de composants : un agent réserve ceux qu'il va modifier (claim_components) ; un recouvrement est refusé ou signalé avant qu'il code.
• Isolation par worktree git par agent, fusion contrôlée en fin de session.
• Détection des conflits structurels : deux agents qui modifient la même surface publique ou créent des arêtes contradictoires.
• Vue d'ensemble en temps réel sur la carte : quel agent travaille sur quel composant.
Collaboration entre modèles
Plusieurs modèles, éventuellement de fournisseurs différents, travaillent en parallèle selon un protocole fixé d'avance par le plugin : ils se répartissent les tâches, s'entraident et se contrôlent. Le développeur voit en permanence qui fait quoi et ce qu'ils se disent.
Topologies prédéfinies.
Topologie
Rôles
Usage
Binôme
Un implémenteur, un relecteur qui valide chaque tâche
Contrôle qualité continu
Orchestrateur
Un planificateur découpe et assigne, des exécutants livrent
Gros jalons parallélisables
Essaim
Agents égaux qui prennent les tâches libres avec réservation de composants
Débit maximal sur tâches indépendantes
Contre-expertise
Deux modèles implémentent la même tâche, un troisième arbitre
Décisions critiques, choix d'algorithme
Les rôles (planificateur, implémenteur, relecteur, testeur, arbitre) sont attribués par modèle dans .timonier/team.json, en quelques clics depuis un préréglage.
Protocole. Les échanges sont des messages typés, jamais du texte libre seul : task_assign, question, answer, review_request, review_result, handoff, blocker, status, escalate. Chaque tâche suit une machine à états (assignée, en cours, en revue, acceptée, rejetée, escaladée) que le démon fait respecter : un implémenteur ne peut pas accepter sa propre tâche, une revue rejetée renvoie la tâche avec ses motifs.
Canaux. Un canal par tâche, un par composant, un canal général et des messages directs. Ils sont stockés par le démon en journal append-only, liés aux sessions, aux commits et aux PR.
Notifications sans veille active. Un modèle ne peut pas être interrompu en pleine génération ; le plugin garantit en revanche qu'il apprend l'arrivée d'un message au plus tôt, sans dépenser de tokens à surveiller :
1. En-tête de boîte aux lettres : chaque réponse d'un outil MCP Timonier porte le nombre de messages non lus et leur priorité. Tout agent compatible MCP le voit dès son prochain appel d'outil.
2. Hooks quand l'agent en propose : injection des messages après chaque appel d'outil, et refus de terminer une session tant qu'un message bloquant est en attente.
3. Réveil sur message : un agent inactif est relancé par le démon en mode non interactif, avec sa session reprise et le message en entrée.
Supervision.
• Vue des canaux, façon messagerie d'équipe, avec filtres par modèle, tâche et type de message.
• Lignes de nage par modèle dans le temps : tâche en cours, attente, revue, blocage.
• Tableau de répartition : quel modèle tient quelles tâches et quels composants, avec la charge et le coût de chacun.
• Intervention humaine : poster dans n'importe quel canal, mettre un modèle en pause, réassigner une tâche, trancher une escalade.
Garde-fous. Détection des boucles (ping-pong de revues), budget de messages et de tokens par tâche, escalade automatique vers l'humain après un nombre fixé de désaccords.
Processus de développement
Le développeur choisit en quelques clics comment l'agent travaille, comme on règle le processus d'une équipe : branches, pull requests, tickets, revue, intégration continue.
Préréglage
Fonctionnement
Pour qui
Solo rapide
Commits directs sur la branche principale, une session = une série de commits
Prototype, exploration
Solo discipliné
Une branche et une PR par tâche, fusion automatique si tests, budgets et règles passent
Développeur seul sur un projet durable
Équipe
Ticket → branche → PR → revue humaine obligatoire → fusion, CI bloquante
Projet partagé, code en production
Chaque préréglage est un point de départ : tout réglage reste modifiable individuellement dans un panneau de paramètres.
Réglages disponibles.
• Tickets : création automatique d'un ticket par tâche du plan, synchronisation de l'état (ouvert, en cours, en revue, fermé) ; GitHub Issues, GitLab, Jira, Linear.
• Branches : convention de nommage (tim/<ticket>-<slug>), un worktree par agent en multi-agents.
• Commits : convention (Conventional Commits ou libre), référence au ticket, granularité.
• Pull requests : ouverture automatique en fin de session, description générée avec la tâche, le diff structurel, les tests, l'effet sur les performances et les violations éventuelles ; brouillon ou prête à relire.
• Revue et fusion : approbation humaine requise ou non, fusion automatique sous conditions, stratégie (squash, merge, rebase).
• CI : génération du workflow (GitHub Actions, GitLab CI) qui rejoue tests, budgets et règles d'architecture.
Fichier .timonier/workflow.json. Versionné dans le repo, il décrit le processus choisi. Le bloc géré des instructions de l'agent en est dérivé, et les outils MCP l'appliquent : task_start crée le ticket et la branche, session_end ouvre la PR. L'agent n'a pas à connaître le processus, il le suit par construction.
L'accès aux forges passe par leurs CLI (gh, glab) ou leurs API, avec les jetons stockés dans le trousseau de l'IDE.
Inventaire des technologies et santé des composants
Inventaire. Construit depuis les manifestes (Cargo.toml, package.json, pyproject.toml, go.mod…) en SBOM CycloneDX : langages, frameworks, dépendances avec versions, licences, CVE connues. Il signale en priorité les doublons fonctionnels (deux libs pour la même chose) et les dépendances ajoutées par l'agent depuis la dernière revue.
Santé par composant.
• churn (fréquence de modification) ;
• complexité ;
• duplication, travers fréquent du code généré ;
• code mort et symboles orphelins.
Ces indicateurs alimentent un calque de la carte et sont exposés à l'agent.
Surface publique et surface de sécurité
Deux vues dérivées du graphe rendent visibles des changements que l'IA introduit souvent sans le dire.
Surface publique. Pour chaque composant, les symboles exportés (fonctions, types, endpoints, schémas, commandes CLI) et leurs signatures, via LSP/SCIP et les fichiers OpenAPI, protobuf ou GraphQL quand ils existent. Le diff depuis la dernière revue classe chaque changement : ajout, modification compatible, rupture. Les ruptures sont signalées sur la carte et dans le journal de la session.
Surface de sécurité. Carte des points où le code touche l'extérieur :
• points d'entrée (endpoints HTTP, CLI, handlers de messages) ;
• entrées non fiables et leur propagation jusqu'aux puits sensibles (SQL, shell, système de fichiers, désérialisation) ;
• accès réseau sortants, fichiers, processus lancés ;
• usage de secrets et de variables d'environnement ;
• dépendances avec CVE connues, depuis l'inventaire.
Toute nouvelle entrée de surface ajoutée par l'agent est signalée, même sans violation. L'analyse de flux commence par des heuristiques par langage (requêtes tree-sitter), puis s'appuie sur Semgrep ou CodeQL quand ils sont disponibles.
Initialisation et configuration de l'IA
timonier init formalise les fichiers de suivi dès la création du projet (ou à l'adoption d'un projet existant) et configure l'agent.
.timonier/
  intent.json           # tableau blanc (auteur : le développeur)
  plan.json             # jalons et tâches
  todo.json             # tâches actives
  sessions/             # journal, un fichier par session d'agent
  conventions/          # instrumentation et règles, par langage
  actual.json           # état réel, régénéré au commit
AGENTS.md               # bloc géré par le plugin
.mcp.json, .vscode/mcp.json   # serveur MCP déclaré
Configuration automatique. Deux points d'entrée : l'assistant « Nouveau projet piloté » dans VSCode, et la détection à l'ouverture d'un repo non configuré, qui propose timonier init. Le développeur ne répond qu'aux questions que la détection ne tranche pas.
1. Détection : langages, gestionnaire de paquets, runner de tests, agents installés (Claude Code, Copilot, Cursor, Codex…), état git.
2. Squelette (nouveau projet) : le plugin appelle le générateur officiel de la stack choisie (cargo new, npm create, uv init…) plutôt que des templates maison.
3. Instrumentation : ajout du SDK OpenTelemetry du langage, module d'initialisation derrière une feature ou une variable de configuration, runner de tests configuré pour produire du JUnit XML.
4. Agents : bloc géré dans le fichier d'instructions de chaque agent détecté, déclaration du serveur MCP dans leur format, hooks quand ils existent.
5. Git : hook de commit qui régénère actual.json, base SQLite ignorée, fichiers .timonier/ versionnés.
6. Suivi : tableau blanc vide, plan avec un jalon « Initialisation », première session enregistrée.
7. Preuve de bout en bout : timonier doctor puis un test de fumée instrumenté qui doit produire une première trace rattachée au graphe. La chaîne est validée avant la première tâche.
Packs de langage. Tout ce qui dépend du langage (conventions d'instrumentation, ajout du SDK, configuration du runner, requêtes de surface de sécurité) vit dans un pack versionné et extensible. Ajouter un langage revient à écrire un pack, pas à modifier le démon.
Sûreté. timonier init --dry-run affiche le diff de tous les fichiers touchés ; l'opération est idempotente, et timonier uninit retire tout ce que le plugin a ajouté.
Le bloc géré d'AGENTS.md (et ses équivalents CLAUDE.md, règles Cursor, instructions Copilot selon les agents détectés) est délimité par des marqueurs : le plugin le réécrit, le reste du fichier appartient au développeur.
Faire respecter le protocole. Un agent ne peut pas être contraint à 100 % ; on rend l'écart visible et coûteux :
1. Instructions : ouvrir une session, découper en todo, instrumenter selon les conventions.
2. Hooks quand l'agent en propose : refus de clore une session si la todo n'est pas à jour ou si le journal est vide.
3. Détection par le démon : tout changement de code hors session est marqué « non suivi » et compte dans la dette de revue.
4. timonier doctor : vérifie et répare la configuration (MCP déclaré, bloc géré à jour, conventions présentes).
Expérience et design
Timonier doit être reposant à regarder pendant des heures et compris en un coup d'œil. Ces principes s'appliquent dès le MVP, à toutes les vues.
Calme par défaut.
• Palette désaturée, fonds neutres ; la couleur vive est réservée au sens (rouge = violation, orange = attention, vert = conforme) et n'apparaît que lorsqu'il y a quelque chose à dire.
• Intégration au thème de l'IDE via ses variables CSS, clair comme sombre, pour que Timonier semble natif.
• Animations courtes et douces, uniquement pour expliquer un changement (un composant qui apparaît, une arête qui s'épaissit), jamais décoratives.
• Notifications discrètes : badges et résumés plutôt qu'une pluie de pop-ups ; seules les alertes bloquantes interrompent.
Clair avant d'être complet.
• Une vue d'accueil, le pont, qui répond en quelques secondes à trois questions : où en est le plan, que s'est-il passé depuis ma dernière revue, où dois-je regarder.
• Divulgation progressive : vue d'ensemble d'abord, détail au survol, profondeur au clic.
• Zoom sémantique sur la carte : plus on zoome, plus le niveau de détail change (composants → fichiers → symboles) au lieu d'afficher tout en petit.
• Mise en page stable : la carte garde la position des éléments d'une mise à jour à l'autre (layout incrémental contraint), pour que l'œil retrouve ses repères.
• Un seul langage visuel partagé par toutes les vues : mêmes formes, couleurs et icônes pour un composant, un agent, une violation.
Accessible et fluide.
• Contrastes WCAG AA au minimum, palette sûre pour les daltoniens, et jamais la couleur seule pour porter une information (icône ou motif en plus).
• Navigation complète au clavier, palette de commandes pour chaque action.
• Fluidité sur de gros graphes : rendu par niveaux de détail, agrégation automatique au-delà d'un seuil de nœuds.
• Densité réglable (confortable ou compacte), états vides pédagogiques qui disent quoi faire ensuite.
Méthode. Un petit système de design défini avant la première vue (tokens de couleur, typographie, espacements, composants), des maquettes testées auprès de quelques développeurs, et un test simple pour chaque écran : un utilisateur qui le découvre comprend-il en cinq secondes ce qu'il montre ?
Interface MCP
Le même modèle du projet sert l'humain et l'agent : l'agent consulte l'intention et les règles avant de coder, et lit ses propres résultats après.
Domaine
Outils
Intention
get_intended_architecture, get_drift, get_budgets
Graphe
query_graph, get_component, get_impact
Surfaces
get_public_api_diff, get_security_surface
Performances
get_instrumentation_conventions(lang), get_perf(component), get_instrumentation_gaps, get_budget_violations
Suivi
session_start, session_end, plan_add_task, plan_update_task, todo_add, todo_complete
Processus
get_workflow, task_start, open_pull_request, link_ticket
Multi-agents
claim_components, release_components, list_active_agents
Communication
post_message, read_inbox, ack_message, request_review, submit_review, handoff_task, escalate
Qualité
get_tech_inventory, get_component_health, get_coverage
Les noms sont provisoires ; les outils d'écriture n'acceptent que des opérations structurées et validées.
Découpage MVP / v2 / v3
Le MVP vise la boucle complète pour un développeur seul sous VSCode : je dessine, l'agent produit en suivant le protocole, je vois le résultat, ses performances et où regarder.
Phase
Contenu
Critère de sortie
MVP (solo, VSCode)
Démon Rust, indexation tree-sitter + LSP, composants, carte et diff de revue, tableau blanc mixte et comparaison à l'intention, performances niveaux 1 et 3, budgets de performance, timonier init / timonier doctor, plan, todo et journal via MCP, processus de développement (préréglages, GitHub), dette de revue, inventaire des technologies
Un projet réel piloté de bout en bout sans lire le code ligne à ligne
v2 (profondeur)
SCIP, profilage niveau 2, surface publique, surface de sécurité, multi-agents (voies, réservation, worktrees), collaboration entre modèles (topologies, protocole, canaux, notifications, supervision), couverture par composant, santé, analyse d'impact, voyage dans le temps, animation des flux de données, coût de l'IA, GitLab, Jira et Linear
Toutes les vues de pilotage disponibles sur deux langages au moins, deux agents en parallèle sans conflit
v3 (équipe)
Snapshot et diff structurel calculés en CI par PR, commentaire automatique de PR, règles et intention relues en PR, journal et dette de revue partagés
Une équipe de 3 à 5 personnes l'utilise sur un même repo
Le MVP reste ambitieux ; si un découpage plus serré est nécessaire, l'inventaire des technologies et la comparaison à l'intention sont les premiers candidats au report.
Décisions techniques
[ ] Granularité par défaut des arêtes Calls sur un gros repo : fonction → fonction, ou tronquée au fichier pour les langages sans LSP fiable ? → Décidé : arêtes fonction → fonction stockées seulement si elles viennent du LSP ou de SCIP, sinon fichier → fichier pondérées ; affichage toujours agrégé au niveau du zoom, en partant de la vue haut niveau par rôles ; affinage LSP paresseux quand on ouvre un fichier ; compaction au-delà d'environ 50 000 arêtes de symboles.
[ ] Stratégie d'indexation incrémentale : invalidation, ordre de fusion des sources, coût des requêtes LSP en masse. → Décidé : trois vitesses. Immédiat à la sauvegarde (anti-rebond ~300 ms, hash du fichier, reparse tree-sitter incrémental, mise à jour du graphe par différence) ; arrière-plan (fichiers dépendants marqués si la signature publique change, affinage LSP en file basse priorité en commençant par ce qui est affiché) ; au commit (SCIP si disponible, re-clustering complet initialisé par la partition précédente, snapshot figé). La source la plus fiable gagne, chaque fichier mémorise « vérifié par LSP au hash X ». Un seul écrivain SQLite en WAL, indexation initiale parallélisée, cache persistant.
[ ] Bibliothèque de graphe pour la webview (Cytoscape + ELK, ou autre) et tenue à l'échelle sur plusieurs milliers de nœuds. → Décidé : React dans la webview (cohérent avec tldraw), React Flow (xyflow) pour la carte, elkjs pour la mise en page hiérarchique avec les positions précédentes en contraintes, calque canvas/WebGL (PixiJS) pour l'animation des flux, uPlot pour les timelines et courbes de performances. Le zoom sémantique limite l'affichage à quelques centaines de nœuds.
[ ] Protocole entre le démon et la webview (JSON-RPC sur socket local, ou réutilisation du serveur MCP). → Décidé : JSON-RPC 2.0 sur socket local (Unix ou named pipe) entre démon et extension, relais postMessage vers la webview ; abonnements par vue avec deltas versionnés et snapshot complet en cas de désynchronisation ; MCP séparé, mince proxy vers le même démon ; JSON d'abord, MessagePack en réserve pour les traces ; même protocole sur WebSocket local avec jeton pour Zed ou une app standalone.
[ ] Seuil de régression de performances et nombre de runs nécessaires pour qu'il soit fiable. → Décidé : détection d'abord sur le nombre d'instructions CPU (compteurs matériels) quand disponible, temps réel affiché ; 5 exécutions, jusqu'à 15 si la variance est forte, médiane et MAD ; référence = médiane des 5 derniers snapshots de la branche principale ; régression signalée si Mann-Whitney p < 0,01 et écart ≥ max(5 %, 3 × MAD) ; « probable » puis « confirmée » après re-run ; référence réinitialisée si l'empreinte machine change.
[ ] Capacités réelles des extensions Zed (panneaux UI) au moment du développement. → Décidé : Zed hors périmètre pour l'instant, à reconsidérer plus tard.
[ ] Mesure du coût de l'IA : quels agents exposent leur consommation de tokens, et comment la récupérer. → Décidé : 1) télémétrie OpenTelemetry native quand l'agent l'exporte (Claude Code : tokens par type et coût estimé), pointée vers le récepteur OTLP du démon par timonier init ; 2) proxy local de comptage pour les agents utilisant une clé d'API ; 3) sinon « non mesuré », jamais d'estimation inventée. Attribution exacte par session et par tâche, estimée par composant au prorata des modifications. Les tokens sont la mesure de référence, le coût en dollars une indication.