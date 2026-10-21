---
name: readme
description: How to write and update Timonier's public pages — README.md (English), README.fr.md (French), docs/features.md, and their screenshots in docs/images/. Use whenever a feature lands or changes in a way a GitHub visitor would see, when the status of the project changes (a phase ends), when a screenshot is out of date, or when asked to rewrite, translate or shorten the README.
---

# Timonier's README

The README is the page a GitHub visitor reads for thirty seconds before deciding whether
Timonier concerns them. It answers, in this order: what problem it solves, what it looks like,
whether it works yet, how to try it. Everything else lives one link away.

Write it with the user's `scientific-prose` skill: plain words, every claim checkable, no
slogans, nothing that is not true today.

## The files

| File               | Reader                                    | Content                                                  |
| ------------------ | ----------------------------------------- | -------------------------------------------------------- |
| `README.md`        | Any developer, in English (the reference) | Purpose, tour with screenshots, status, try it, develop  |
| `README.fr.md`     | French-speaking developers                | The same page, translated by hand, same order and images |
| `docs/features.md` | Someone evaluating in depth, in English   | Every feature that works today, with its figures         |
| `docs/images/*.png`| Both READMEs                              | One screenshot per feature, in English                   |

- Both READMEs start with a language line: `**English** · [Français](README.fr.md)`.
- A change to one README is made to the other in the same change. English is the reference;
  the French is written, not machine-translated (French typography: « », espace insécable
  before « : ; ! ? », decimal comma).
- Another language gets `README.<code>.md`, is listed in the language line of every README,
  and says at its top which English version it translates (the date).

## README.md structure

1. Title, language line, one sentence of purpose, then the hero screenshot (the map).
2. **The problem** in a short paragraph: agents write code faster than the developer can read
   it. Then the four questions Timonier answers (want, exists, behaves, happening).
3. **A tour**: one section per feature, each with two to four sentences on what the developer
   sees and does, then its screenshot. Order: map, review diff, review debt, whiteboard and
   comparison with the intent, plan/todo/sessions and bridge, agents through MCP, performances.
4. **How it works**: daemon, graph, extension, MCP, in four lines; a link to the spec.
5. **Status**: the true, dated state (phases done, what is under way, platforms, nothing
   published). Update it at the end of each phase.
6. **Try it**: requirements and the fewest commands.
7. **Develop**, **Documentation** (spec, plan, ADRs are in French), acknowledgements, licence.

Keep the README under about 250 lines. Detail (figures, edge cases, every flag) goes in
`docs/features.md`; the README links to it.

## Screenshots

- Taken by Playwright, never by hand: `npm run screenshots -w webview` (after `npm run build`
  only for the whiteboard, which uses the production build) writes every file of
  `docs/images/` from `webview/test/screenshots/readme.shots.ts`. It uses the development page
  (`demo.html`, sample data, English) and VSCode's Dark Modern theme.
- A new feature with a visible surface gets a test in that file and an image in both READMEs.
- Look at every image after taking it (Read the PNG): the state shown must match the caption
  (for example the debt layer needs `review=1&debt=1`). Crop the viewport to the content.
- Each image has alt text that says what it shows, not "screenshot".
- Images are sample data: say so once, near the tour.

## Checks before handing over

- Every command in « Try it » and « Develop » exists (`package.json` scripts, `cargo` targets).
- Every feature described exists in the code today; planned work is under « Status ».
- Links resolve (relative paths from the repository root).
- Both READMEs have the same sections and the same images.
