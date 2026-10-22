import { rmSync } from "node:fs";
import path from "node:path";

// After every suite, the daemon stopped: the project of `.vscode-test.mjs` and the daemon's
// runtime directory go with the run, however VSCode was launched. `npm run test:integration`
// would remove them on exit; « Extension Tests » (F5) only lists that file, and nothing else
// would (review n° 10, I10-1).
after(() => {
  const project = process.env.TIMONIER_FIXTURE;
  if (!project) return;
  const temp = path.dirname(project);
  // Only the directory `.vscode-test.mjs` made.
  if (!path.basename(temp).startsWith("timonier-it-")) return;
  try {
    rmSync(temp, { recursive: true, force: true });
  } catch {
    // Windows keeps the files a process holds open (EPERM): the next run removes what is a day
    // old (T-125).
  }
});
