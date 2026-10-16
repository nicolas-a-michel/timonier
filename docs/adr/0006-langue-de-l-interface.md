# 0006 — Langue de l'interface : anglais de base, traduction française

- Statut : Accepté (2026-10-16, réponse de l'humain à Q13 : bilingue pour l'instant, d'autres langues plus tard)
- Date : 2026-10-14
- Phase : 1

## Contexte

La spec est en français et nomme ses éléments d'interface en français (« marquer comme revu »,
« le pont »). Elle ne dit pas dans quelle langue l'interface s'affiche. Le projet est public et
vise « n'importe quel langage » de programmation, donc des utilisateurs non francophones.

## Décision proposée

- **Base en anglais**, avec une traduction française, dès la Phase 1, tant que les chaînes
  sont peu nombreuses.
- **Extension** : `package.nls.json` et `package.nls.fr.json` pour le manifeste (commandes,
  description) ; `vscode.l10n.t` avec `l10n/bundle.l10n.fr.json` pour les messages.
- **Webview** : un dictionnaire `en`/`fr` (`webview/src/i18n.ts`). La langue est choisie
  d'après `<html lang>`, que l'extension renseigne avec `vscode.env.language`.
- **Tests** : toute chaîne du manifeste ou de `l10n.t` doit avoir sa traduction française.
- **CLI et messages des agents** : en anglais.

## Alternatives écartées

- **Français seul** : cohérent avec la spec, mais ferme le projet public aux non-francophones.
  Il serait coûteux de traduire après coup.
- **Anglais seul** : moins de travail, mais l'auteur et premier utilisateur est francophone.

## Conséquences

Chaque nouvelle chaîne s'écrit en anglais et en français.

D'autres langues viendront plus tard (réponse à Q13). En ajouter une demande :
- `package.nls.<code>.json` et `l10n/bundle.l10n.<code>.json` dans l'extension ;
- une entrée dans `webview/src/i18n.ts`, et sa reconnaissance dans `resolveLocale` ;
- d'étendre à cette langue les tests de complétude, qui ne visent aujourd'hui que le français.
