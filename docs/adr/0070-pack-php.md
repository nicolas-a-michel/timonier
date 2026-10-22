# 0070 — Le pack PHP : noms PSR-4, Composer, PHPUnit

- Statut : Proposé
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q90 ; ADR 0012, 0013, 0034, 0050, 0055 ; tâche PLAN.md 10.10

## Contexte

Un projet PHP moderne passe par Composer : `composer.json` déclare ses paquets et sa carte
d'autochargement PSR-4 (`"Shop\\": "src/"`), qui fait d'un chemin de fichier un nom de classe ;
`composer.lock` épingle les versions, et pour chaque paquet son espace de noms et sa licence.
PHPUnit écrit lui-même du JUnit.

## Décision

- **Pack `php`** : grammaire `tree-sitter-php` 0.25.1 (MIT, vérifiée le 2026-10-22), sa
  variante qui lit le HTML autour de `<?php` ; `.php`. Définitions : classes, énumérations et
  leurs cas, interfaces et traits (`trait`), méthodes, propriétés (et celles que promeut un
  constructeur), constantes, fonctions. `[walk] skip` : `vendor`, `var`.
- **Noms** : `php:<nom PSR-4>#<définition>` (`php:Shop\Model\Item#Item.price`), le chemin hors
  du dossier PSR-4 (`php:tests\CartTest#…`).
- **Écosystème `composer`** : `vendor/paquet`, sa première racine PSR-4 (l'espace pour nom de
  code, le dossier pour `lib_path`) ; `require` à l'exécution, `require-dev` à part, `php` et
  les extensions laissés. Le nom de code d'une dépendance est la liste des espaces de noms que
  `composer.lock` lui donne ; sans verrou, le nom du fournisseur.
- **Résolution** : un `use` lie son dernier segment ou son alias ; il mène au fichier dont le
  nom PSR-4 est ce chemin, sinon au paquet qui autocharge cet espace ; les classes de PHP
  (`PDO`) restent non résolues. Un nom seul se cherche dans le paquet.
- **Tests** : `php vendor/bin/phpunit --log-junit .timonier/runs/junit.xml`, par PHP pour que
  la commande marche sous Windows aussi ; avec `phpunit.xml.dist` ou `phpunit.xml`, leurs
  suites, sinon le dossier `tests` (vérifié avec PHPUnit 13.4 et PHP 8.5).
- **SDK** (`init`) : `open-telemetry/api` ^1.10 dans `require`, `open-telemetry/sdk` ^1.15,
  `open-telemetry/exporter-otlp` ^1.4 et `guzzlehttp/guzzle` (son client HTTP) dans
  `require-dev` (Apache-2.0 et MIT, vérifiés sur Packagist le 2026-10-22) ; `uninit` retire ce
  qui n'y était pas. `Timonier\TimonierOtel` (`tests/Timonier/TimonierOtel.php`) construit
  l'exportateur OTLP/HTTP, lit le point d'arrivée, les en-têtes et la ressource de la passe.
- **Inventaire** : `composer.lock` donne chaque paquet, sa version et sa licence ; OSV sous
  « Packagist », purl `pkg:composer/<fournisseur>/<paquet>`.
- **Trous d'instrumentation** : publique, une méthode sans `private` ni `protected`.

## Alternatives écartées

- **Nommer par la déclaration `namespace`** : PSR-4 la rend égale au chemin, connu avant
  l'extraction ; un fichier hors de la carte garde son chemin.
- **`vendor/bin/phpunit` directement** : `cmd` ne l'exécute pas tel quel sous Windows.

## Conséquences

- Un projet sans Composer n'a ni paquets ni noms PSR-4 : ses fichiers sont nommés par leur
  chemin.
- `composer update` doit passer après `init` pour installer le SDK.
