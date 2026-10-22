# 0069 — Le pack C# : projets `.csproj`, espaces de noms, rapports JUnit par projet de test

- Statut : Accepté (2026-10-22, Phase 10 validée)
- Date : 2026-10-22
- Phase : 10
- Liens : Q86, Q88, Q90 ; ADR 0012, 0013, 0034, 0050, 0064 (dossier de rapports), 0066
  (fichiers sous un projet) ; tâche PLAN.md 10.9

## Contexte

Un projet .NET est un fichier `<Nom>.csproj` (XML), réuni à d'autres par une solution (`.sln`,
`.slnx`). Le code se range par espaces de noms, déclarés dans le fichier et indépendants des
dossiers ; un `using` amène un espace de noms entier. `dotnet test` n'écrit pas de JUnit sans
un enregistreur tiers ; le SDK OpenTelemetry de .NET écoute les `ActivitySource` du runtime.

## Décision

- **Pack `csharp`** : grammaire `tree-sitter-c-sharp` 0.23.5 (MIT, vérifiée le 2026-10-22),
  `.cs`. Définitions : espaces de noms (`module`, en bloc ou pour le fichier), classes,
  structures, records et leurs paramètres, énumérations, interfaces (`trait`), délégués,
  méthodes, constructeurs, propriétés, champs, événements. `[walk] skip` : `bin`, `obj`,
  `TestResults`, `packages`.
- **Manifestes à motif** : `MANIFESTS` accepte `*.csproj`, une fin de nom de fichier.
- **Écosystème `nuget`** : un paquet par `.csproj`, nommé par `<AssemblyName>` ou le nom du
  fichier, son espace racine pour nom de code ; ses `<PackageReference>` ; un projet de test
  (qui référence Microsoft.NET.Test.Sdk) n'a pas de dépendance à l'exécution. Le BOM qu'écrit
  `dotnet new` est retiré avant la lecture.
- **Noms** : `csharp:<projet>/<chemin>#<définition>`, comme npm.
- **Résolution** : un `using` mène au premier fichier qui déclare cet espace de noms, dans
  n'importe quel projet du dépôt ; sinon au paquet NuGet que l'espace commence ; `System` reste
  non résolu. Un nom seul se cherche dans son projet, puis dans tout le dépôt.
- **Tests** : `dotnet test --logger "junit;LogFilePath=TestResults/timonier/{assembly}.xml"`
  avec JunitXml.TestLogger 8.0.0 (MIT). L'enregistreur écrit à côté de chaque projet de test
  (constaté avec le SDK 10.0.402) : `junit = "**/TestResults/timonier/"`. La recherche des
  dossiers de rapports ne saute plus `src/`, où beaucoup de solutions gardent leurs tests.
- **SDK** (`init`) : le projet de test reçoit `OpenTelemetry` et
  `OpenTelemetry.Exporter.OpenTelemetryProtocol` 1.20.1 (Apache-2.0, vérifiés sur NuGet le
  2026-10-22) et l'enregistreur, dans un `<ItemGroup>` entre marqueurs ; les fichiers
  `Timonier/TimonierOtel.cs` et `Timonier/TimonierSmokeTest.cs` (xUnit) vont dans son dossier.
  Le démarreur écoute toutes les `ActivitySource` et force OTLP/HTTP (l'exportateur part en gRPC
  par défaut) ; il lit le point d'arrivée et les en-têtes de la passe.
- **Inventaire** : les `<PackageReference>` des `.csproj` jusqu'à trois dossiers sous la racine,
  versions écrites ou centrales (`Directory.Packages.props`) ; OSV sous « NuGet ».
- **Trous d'instrumentation** : publique, une méthode `public`.

## Alternatives écartées

- **Lire les rapports TRX** de `dotnet test` : un second format de résultats à côté du JUnit.
- **Nommer par l'espace de noms** : un fichier peut en déclarer plusieurs, et une classe
  partielle se répartit entre fichiers.

## Conséquences

- Le test de fumée suppose xUnit ; un projet NUnit ou MSTest l'adapte.
- Les fichiers de `TestResults/` doivent rester hors de git (le modèle `.gitignore` de .NET
  les ignore).
