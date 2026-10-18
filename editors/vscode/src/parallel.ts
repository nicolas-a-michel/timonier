/**
 * `task` on each item, at most `limit` at a time; the results in the order of the items. The
 * relay asks the language servers so for a whole file at once (ADR 0022).
 */
export async function inParallel<T, R>(
  items: readonly T[],
  limit: number,
  task: (item: T) => Promise<R>,
): Promise<R[]> {
  const results: R[] = new Array<R>(items.length);
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const index = next++;
      results[index] = await task(items[index] as T);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

/**
 * Asks again, after each pause, the items `doubtful` still names in `answer`, until none is,
 * the pauses run out, or `cancelled` says the asker no longer waits (ADR 0022). `answer` is
 * updated in place.
 */
export async function askAgain<R>(
  answer: R[],
  doubtful: (answer: readonly R[]) => number[],
  pauses: readonly number[],
  cancelled: () => boolean,
  ask: (indices: number[]) => Promise<R[]>,
  sleep: (ms: number) => Promise<unknown> = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
): Promise<void> {
  for (const pause of pauses) {
    const again = doubtful(answer);
    if (again.length === 0 || cancelled()) return;
    await sleep(pause);
    if (cancelled()) return;
    const found = await ask(again);
    again.forEach((i, k) => {
      const next = found[k];
      if (next !== undefined) answer[i] = next;
    });
  }
}
