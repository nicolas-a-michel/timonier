# 0065 — Le pack C++ : chemins de fichiers, CMake avec vcpkg et Conan, CTest

- Statut : Proposé
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q89 ; ADR 0012, 0013 (packs, noms qualifiés), 0034 (lanceurs), 0050 (SDK
  par `init`), 0055 (inventaire), 0063 (conteneur sur le nœud d'une définition), 0064 ; tâche
  PLAN.md 10.5

## Contexte

C++ n'a ni modules nommés ni gestionnaire de paquets standard : un fichier en inclut un autre
par son chemin, une classe se déclare dans un en-tête et se définit dans un source, et les
dépendances viennent de CMake (`find_package`, `FetchContent`), de vcpkg (`vcpkg.json`) ou de
Conan (`conanfile.txt`, `conanfile.py`). L'humain a retenu les trois (Q89). Le SDK OpenTelemetry
de C++ (opentelemetry-cpp) se compile, avec curl et protobuf pour son exportateur OTLP/HTTP.

## Décision

- **Pack `cpp`** : grammaire `tree-sitter-cpp` 0.23.4 (MIT, vérifiée le 2026-10-22, chargée par
  tree-sitter 0.27) ; `.cpp`, `.cc`, `.cxx`, `.c++`, `.hpp`, `.hh`, `.hxx`, `.h++`, `.ipp`,
  `.tpp`, et `.h` jusqu'au pack C. Définitions : espaces de noms (`module`), classes, structures,
  unions, énumérations, alias, concepts (`trait`), méthodes déclarées ou définies dans la classe,
  champs, fonctions, prototypes et variables de fichier ou d'espace de noms. `[walk] skip` des
  dossiers de construction (`build`, `out`, `cmake-build-*`, `vcpkg_installed`, `_deps`,
  `third_party`).
- **Noms** : `cpp:<chemin depuis la racine, extension comprise>#<espace::Classe::membre>`
  (`cpp:src/cart.cpp#shop::Cart::total`) : un en-tête et son source partagent leur nom de base.
  Une méthode définie hors de sa classe (`int Cart::total() const`) porte un conteneur sur son
  propre nœud, comme le récepteur Go : elle a le même chemin que dans sa classe, et la version
  avec un corps est gardée.
- **Écosystème `cmake`** : un paquet par `CMakeLists.txt` qui déclare un `project()` ; ses
  `add_subdirectory` sont des membres. Ses dépendances : `vcpkg.json`, puis Conan, puis
  `find_package` et `FetchContent_Declare`, une par nom de code, le premier dossier de leurs
  en-têtes (en minuscules, coupé au premier `-` ou `_`, sans `unofficial-` : `nlohmann-json` →
  `nlohmann`, `boost-asio` → `boost`). Les cadres de test (gtest, catch2, doctest, benchmark)
  ne sont pas à l'exécution.
- **Résolution** : un include ne lie aucun nom. `"…"` se cherche depuis le dossier du fichier ;
  puis depuis le dossier du paquet, son `include/` et son `src/` ; puis le seul fichier du dépôt
  dont le chemin finit ainsi (un index par nom de fichier) ; sinon la dépendance que son premier
  dossier nomme ; les en-têtes du système restent non résolus. Un nom défini dans plusieurs
  fichiers (déclaré dans l'en-tête, défini dans le source) mène à celui que le fichier inclut.
  `Cart::make()` et `shop::price()` se résolvent par leur dernier nom, sinon par le premier.
  Les chevrons d'un include sont retirés à l'extraction.
- **Tests** : CTest, dans `.timonier/runs/cmake` (que git ignore déjà), à part du dossier du
  développeur : `cmake -S . -B … && cmake --build … && ctest --test-dir … --output-junit
  ../junit.xml` (chemin relatif au dossier de construction, vérifié avec CMake 4.2.3). Avec
  vcpkg, CMake prend sa chaîne d'outils dans `CMAKE_TOOLCHAIN_FILE` (variable d'environnement
  depuis CMake 3.21) ou dans un preset. Avec Conan (`conanfile.txt` ou `conanfile.py`), un
  lanceur à part passe d'abord par `conan install . --output-folder=.timonier/runs/cmake
  --build=missing -s build_type=Debug`, puis donne à CMake `conan_toolchain.cmake`, que CMake lit
  dans le dossier de construction, et `-c tools.cmake.cmaketoolchain:user_presets=` l'empêche
  d'écrire un `CMakeUserPresets.json` dans le projet (vérifié avec Conan 2.33.0).
- **SDK** (`init`) : opentelemetry-cpp 1.29.0 avec la fonction `otlp-http` (Apache-2.0, vérifié
  sur le registre de vcpkg et bâti par vcpkg 2026.10.16 le 2026-10-22), ajouté à `vcpkg.json`
  quand il existe ; un bloc balisé du `CMakeLists.txt` racine (`enable_testing()`,
  `add_subdirectory(timonier)`) ; le dossier `timonier/` : `timonier_otel.hpp`
  (`timonier::start_telemetry()`), le test de fumée et leur `CMakeLists.txt`, qui ne fait rien
  quand `find_package(opentelemetry-cpp CONFIG QUIET)` ne le trouve pas. Avec Conan,
  `opentelemetry-cpp/1.26.0` (la plus récente de ConanCenter, son exportateur OTLP/HTTP par
  défaut) en tête des `[requires]` de `conanfile.txt`, entre marqueurs ; une dépendance de pack
  dit pour quel gestionnaire elle est (`manager`). Le CMake de `timonier/` prend la cible de
  l'exportateur sous son nom amont et vcpkg (`otlp_http_exporter`) ou Conan
  (`exporter_otlp_http`), et lie `opentelemetry-cpp::common` après lui (les bibliothèques
  statiques de Conan ne déclarent pas ce lien). Avec un `conanfile.py`, ou sans gestionnaire,
  le SDK s'ajoute à la main ; `init` le dit.
- **Inventaire** : le projet du `CMakeLists.txt` racine ; les ports de `vcpkg.json` (version des
  `overrides` ou de `version>=`), les références Conan (versions de `conan.lock`), puis
  `find_package` et `FetchContent` (`GIT_TAG`). Trois écosystèmes : Conan (OSV « ConanCenter »,
  purl `conan`), vcpkg et CMake (inconnus d'OSV, purl `generic`). Quand OSV ne connaît aucun des
  écosystèmes de l'inventaire, il dit « non vérifié », pas « aucune ».
- **Trous d'instrumentation** : publique, une fonction qui n'est pas `static` ;
  `*_test.cpp` et `*_unittest.cc` sont du code de test.

## Alternatives écartées

- **Nommer par le chemin sans extension** : l'en-tête et le source d'une même classe auraient
  les mêmes identifiants.
- **Interroger compile_commands.json ou clangd pour les includes** : plus juste, mais il faut une
  construction ; le raffinement LSP (ADR 0022) la remplace quand l'IDE a clangd.
- **Écrire un exportateur OTLP à la main** : pas un SDK OpenTelemetry, ce que la spec demande.

## Conséquences

- Un include que deux fichiers terminent de la même façon (`util.h` dans deux dossiers) reste
  non résolu, faute de savoir lequel le compilateur prendrait.
- La première passe d'un projet vcpkg compile ses ports (quelques minutes pour
  opentelemetry-cpp), puis le cache binaire de vcpkg les reprend.
