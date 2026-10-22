# 0062 — Le pack Python : modules, `pyproject.toml`, pytest

- Statut : Proposé
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88 ; ADR 0012, 0013 (packs, noms qualifiés), 0034 (lanceurs), 0050 (SDK par
  `init`), 0055 (inventaire), 0061 (pack JavaScript, `[walk] skip`) ; tâche PLAN.md 10.2

## Contexte

Python est le premier langage d'un écosystème que Timonier ne lisait pas. Son code ne se nomme
pas par sa distribution (`shop-api`) mais par son arborescence sous une racine de sources :
`src/shop/cart.py` s'importe `shop.cart`, et `__init__.py` est le paquet lui-même. Ses projets
déclarent leurs dépendances dans `pyproject.toml` (PEP 621, groupes PEP 735, Poetry encore), et
leurs tests passent par pytest, souvent dans un environnement virtuel que uv gère.

## Décision

- **Pack `python`** : grammaire `tree-sitter-python` 0.25.0 (MIT, vérifiée le 2026-10-22,
  chargée par tree-sitter 0.27), `.py` et `.pyi`. Définitions : classes, fonctions, méthodes
  (décorées ou non), champs de classe, valeurs de module. Imports : `import a.b` lie `a`,
  `import a.b as c` lie `c`, `from m import x` et les imports relatifs (`from . import x`,
  `from ..m import y`). `[walk] skip` : `__pycache__`, `venv`, `site-packages` (`.venv` et `.tox`
  sont cachés).
- **Écosystème `pypi`** : un paquet par `pyproject.toml` (`[project]`, sinon `[tool.poetry]`),
  les membres d'un espace de travail uv ; les dépendances de `[project]` à l'exécution, les
  optionnelles et les groupes en développement ; noms normalisés selon PEP 503. Racine des
  sources : `src/` quand le paquet en a un, sinon son dossier.
- **Noms** : `python:<module pointé>#<définition>` (`python:shop.models#Item.name`). Un import se
  résout vers le fichier du plus long module, puis vers un paquet du dépôt ou une dépendance par
  son premier segment ; la bibliothèque standard reste non résolue. `from shop import prices`
  relie au sous-module `prices` quand il existe.
- **Tests** : `pytest --junit-xml=.timonier/runs/junit.xml`, ou `uv run --group timonier pytest
  …` quand le projet a un `uv.lock` (l'environnement du projet, que uv fait) ; nouveau champ
  `when` des règles de lanceur : un fichier qui doit exister à la racine.
- **SDK** (`init`) : `opentelemetry-api` 1.45.1 dans les `dependencies` de `[project]` (le code
  qui part l'importe, comme Q78 pour npm) ; `opentelemetry-sdk`,
  `opentelemetry-exporter-otlp-proto-http` 1.45.1 et pytest 9.1.1 dans un groupe PEP 735
  `timonier` (versions vérifiées sur PyPI le 2026-10-22, Apache-2.0 et MIT). Chaque ligne ajoutée
  finit par `# timonier`, la table `[dependency-groups]` créée est un bloc entre marqueurs ;
  `uninit` rend le fichier à l'octet près. Un paquet déjà déclaré n'est pas ajouté ; sans
  `[project]` (Poetry seul), l'API s'ajoute à la main. Module `timonier_otel.py`, test de fumée
  `timonier_smoke_test.py`.
- **Inventaire** : les dépendances déclarées, et leurs versions dans `uv.lock` ; uv ne garde pas
  les licences, l'inventaire le dit. OSV les interroge sous l'écosystème « PyPI ».
- **Trous d'instrumentation** : publique, une fonction dont le nom ne commence pas par `_` ;
  `test_*.py` et `*_test.py` sont du code de test.

## Alternatives écartées

- **Nommer par la distribution** (`python:shop-api/…`) : le code ne s'en sert jamais ; un import
  ne se résoudrait pas.
- **Éditer `pyproject.toml` par une bibliothèque TOML qui réécrit le fichier** : les
  commentaires et la mise en forme du développeur partiraient.
- **`requirements.txt` et `poetry.lock` pour l'inventaire** : plus tard, à la demande ; uv est
  le gestionnaire que les nouveaux projets prennent.

## Conséquences

- Un projet sans `uv.lock` doit avoir pytest dans son `PATH` (environnement activé) ; `doctor` le
  dit.
- Les noms d'import qui diffèrent de leur distribution (`pyyaml` / `yaml`, `beautifulsoup4` /
  `bs4`) ne relient pas l'import à la dépendance : il reste vers rien.
