# 0075 — Les noms et les rôles donnés à la main : `names.json`, posés sur le graphe

- Statut : Proposé
- Date : 2026-10-23
- Phase : 11
- Liens : Q6, Q96 ; ADR 0016, 0017, 0072 (composants, rôles) ; 0027 à 0029 (outils et écriture
  de `.timonier/`) ; 0039 (`intent.json`) ; tâche PLAN.md 11.6

## Contexte

Les noms et les rôles des boîtes viennent d'heuristiques : un regroupement prend le nom de son
fichier central, un rôle le vote de ses paquets. Ils se trompent parfois (11.1, Q91). Q6 voulait
que l'agent les affine par un outil structuré ; Q96 a décidé que le développeur les donne aussi,
dans un fichier versionné, rôle compris, et que son choix l'emporte.

## Décision

- **Le fichier** `.timonier/names.json` (format 1) : par identifiant de boîte (paquet ou
  composant), un nom (une ligne, 60 caractères au plus), un rôle, qui l'a donné (`developer`,
  `agent`) et quand. Versionné et partagé par git, comme `intent.json` ; lu dans une borne d'un
  mégaoctet ; un fichier qui ne se lit pas n'est jamais réécrit par-dessus.
- **Posé sur le graphe, jamais dans le stockage** : `view::load` applique les noms à la tête du
  graphe que lisent la carte, les outils MCP, le journal et la comparaison à l'intention ; le
  graphe de la dernière relecture les reçoit aussi, pour qu'un nom donné ne soit pas une
  modification de la boîte. Le stockage garde ce que l'indexation calcule.
- **Le développeur** : F2 ou double clic sur un paquet ou un composant (un fichier ou un symbole
  garde le double clic pour ouvrir son code), ou la commande « Renommer la boîte et changer son
  rôle » ; l'extension demande le nom dans sa saisie et le rôle dans sa liste (« Comme Timonier
  le calcule » en tête), puis appelle `names/set`. Un nom vide et le rôle calculé rendent la
  boîte à Timonier ; ce que l'heuristique donne déjà n'est pas gardé.
- **L'agent** : l'outil `propose_component_name { component, name?, role? }`, nouveau dans le
  catalogue ; refusé (`developer_chose`) là où le développeur a donné un nom ou un rôle ; le bloc
  géré d'`AGENTS.md` l'annonce. L'écriture est datée et marquée `agent`, sans session exigée.
- **Le démon est le seul à écrire** le fichier, sous le verrou des outils (ADR 0029), par un
  fichier temporaire renommé ; il le regarde chaque seconde (un nom venu d'un commit d'un
  collègue) et redessine les vues.

## Alternatives écartées

- **Les noms dans SQLite** : la base se reconstruit ; un nom donné à la main est une donnée du
  développeur, qui vit dans `.timonier/` (skill `rust-daemon`).
- **Un renommage dans la page, sur la boîte** : la saisie de VSCode est accessible au clavier,
  traduite et connue ; une saisie dans la carte demanderait de refaire tout cela.
- **Des noms propres à la machine** : écarté par Q96.

## Conséquences

Une boîte dont l'identifiant change (un regroupement recalculé, `cluster/…`) perd son nom, qui
reste dans le fichier sans effet. Les résumés que l'agent pourrait écrire viendront avec 11.10.
