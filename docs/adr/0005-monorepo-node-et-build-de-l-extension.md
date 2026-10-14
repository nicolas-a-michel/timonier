# 0005 — Monorepo Node, build de l'extension et chargement de la webview

- Statut : Proposé
- Date : 2026-10-14
- Phase : 1
- Liens : skill `vscode-webview-ui` ; PLAN.md H1, H11, H12 ; tâches 1.7 à 1.10

## Contexte

L'extension VSCode (`editors/vscode/`) et la webview React (`webview/`, à la racine pour être
réutilisable, H1) doivent :

- se construire, se tester, se linter et s'empaqueter ensemble ;
- tourner sur Linux et macOS ;
- respecter la sécurité des webviews VSCode.

Versions vérifiées le 2026-10-14 (voir PLAN.md, « Dépendances vérifiées »).

## Décision

**Gestionnaire de paquets** : npm workspaces (`webview`, `editors/vscode`), avec un
`package-lock.json` versionné. La racine porte les outils communs.

**Outils communs**

- TypeScript `~6.0.3` en mode strict. typescript-eslint 8.71 exige TypeScript < 6.1.
- ESLint 10.12 (MIT) en configuration plate :
  - `@eslint/js` 10.0 (MIT) ;
  - `typescript-eslint` 8.71 (MIT) avec `recommendedTypeChecked` et `projectService` ;
  - `eslint-plugin-react-hooks` 7.1 (MIT) pour la webview.
- Prettier 3 (largeur 120). Le Markdown est exclu : il est écrit à la main, `PLAN.md` est lu
  par les hooks et la spec appartient à l'humain.

**Webview**

- Vite 8.3 avec `@vitejs/plugin-react` 6.1 (MIT tous deux), en sortie `webview/dist`.
- `build.manifest` est activé : les front-ends retrouvent les fichiers hachés via
  `.vite/manifest.json`, sans dépendre des noms produits.
- `build.license` écrit `THIRD-PARTY-NOTICES.md`, les notices MIT de React, react-dom et
  scheduler, que la minification retire du bundle. Le `.vsix` le contient.
- Tests vitest 5, en environnement Node, rendus avec `react-dom/server`, sans jsdom.

**Extension**

- esbuild 0.28.2 (MIT) assemble `src/extension.ts` en un seul fichier CommonJS (`dist/extension.js`,
  `vscode` en externe). Le script de build copie ensuite `webview/dist` et la licence dans le
  paquet.
- Tests unitaires : `node:test` sur la sortie de `tsc`. Ils tournent sur le code CommonJS réel,
  sans dépendance de plus. Vitest marcherait aussi : vérifié, `tsc` accepte son import. Le
  premier argument avancé, « ESM seul contre CommonJS », était faux.
- Tests d'intégration : `@vscode/test-cli` avec mocha, dans un vrai VSCode, sous xvfb sur
  Linux.
- Paquet : `vsce package --no-dependencies`. Le bundle n'a pas besoin de `node_modules` ;
  `.vscodeignore` n'inclut que `dist/`, les traductions, le README et la licence.

**Sécurité de la webview**

- HTML généré par l'extension.
- CSP `default-src 'none'`.
- Scripts autorisés seulement avec un nonce frais : `script-src 'nonce-…'`, sans origine,
  sans `unsafe-inline` ni `unsafe-eval`.
- Un test d'intégration prouve que le bundle s'exécute sous cette CSP : la webview envoie
  `{ type: "ready" }` après son premier rendu.
- `localResourceRoots` restreint à `dist/webview`.
- Une seule instance du panneau, révélée si elle est déjà ouverte.

**Node 24** pour les outils (LTS active) ; `@types/node` 24, qui correspond au Node 24.21
embarqué par VSCode 1.141.

## Alternatives écartées

- **pnpm** : `vsce` gère mal ses liens symboliques, et pnpm n'est pas installé sur la machine.
- **TypeScript 7** : incompatible avec typescript-eslint aujourd'hui.
- **webpack** : plus lent, et non nécessaire pour un seul point d'entrée.
- **Noms de fichiers fixes (sans hachage)** : demanderait des options de nommage propres à
  Rolldown, plus fragiles que le manifeste.
- **`@vscode/webview-ui-toolkit`** : déprécié.

## Conséquences

- npm 11 avertit que les scripts d'installation d'esbuild et de vsce-sign ne sont pas couverts
  par `allowScripts`. Dans cette version, ils s'exécutent quand même : le binaire natif
  d'esbuild est en place. L'avertissement est sans effet.
- `npm audit` signale 4 vulnérabilités, toutes dans mocha, une dépendance de
  `@vscode/test-cli` : 1 haute (serialize-javascript), 1 modérée (mocha), 2 basses. Elles ne concernent que les outils de test, jamais le paquet livré. À
  revoir à la prochaine version de test-cli.
- Les tests d'intégration téléchargent VSCode stable dans `.vscode-test/`, ignoré par git.
