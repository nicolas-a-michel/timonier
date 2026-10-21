/**
 * The development process's settings (spec « Development process », ADR 0056): the three
 * presets, then each setting by itself; a change goes to the daemon, which checks it and writes
 * `.timonier/workflow.json`. A preset changed by one setting becomes « custom ».
 */
import { EmptyState } from "../design/components";
import type {
  CiProvider,
  CommitConvention,
  MergeStrategy,
  Preset,
  TicketProvider,
  Workflow,
  WorkflowView,
} from "../protocol";
import type { TrackingMessages } from "./messages";

export interface WorkflowSettingsProps {
  t: TrackingMessages;
  view: WorkflowView | undefined;
  /** What the daemon said when it refused a setting. */
  problem: string | null;
  onSet?: (workflow: Workflow) => void;
}

const PRESETS = ["solo_fast", "solo_disciplined", "team"] as const;

/** A preset's settings, as the daemon makes them (`Workflow::preset`). */
export function preset(which: Preset, pattern = "tim/{ticket}-{slug}"): Workflow {
  const disciplined = which === "solo_disciplined" || which === "team";
  return {
    version: 1,
    preset: which,
    tickets: { provider: disciplined ? "github" : "none" },
    branches: { per_task: disciplined, pattern },
    commits: { convention: "conventional", reference_ticket: disciplined },
    pull_requests: { open_at_session_end: disciplined, draft: false },
    merge: { human_approval: which === "team", auto: which === "solo_disciplined", strategy: "squash" },
    ci: { provider: disciplined ? "github_actions" : "none" },
  };
}

export function WorkflowSettings({ t, view, problem, onSet }: WorkflowSettingsProps) {
  if (!view) {
    return <EmptyState title={t.workflowTitle} text={problem ?? t.workflowLoading} live />;
  }
  const w = view.workflow;
  const set = (change: (w: Workflow) => Workflow) => onSet?.({ ...change(structuredClone(w)), preset: "custom" });
  const check = (label: string, on: boolean, change: (w: Workflow, on: boolean) => void) => (
    <label className="tim-workflow__setting">
      <input type="checkbox" checked={on} onChange={(e) => set((w) => (change(w, e.target.checked), w))} />
      <span>{label}</span>
    </label>
  );
  const choice = <T extends string>(
    label: string,
    value: T,
    options: [T, string][],
    change: (w: Workflow, value: T) => void,
  ) => (
    <label className="tim-workflow__setting">
      <span>{label}</span>
      <select value={value} onChange={(e) => set((w) => (change(w, e.target.value as T), w))}>
        {options.map(([v, words]) => (
          <option key={v} value={v}>
            {words}
          </option>
        ))}
      </select>
    </label>
  );
  return (
    <section className="tim-workflow" aria-label={t.workflowTitle}>
      <h1 className="tim-tracking__heading">{t.workflowTitle}</h1>
      <fieldset className="tim-workflow__presets">
        <legend>{t.workflowPreset}</legend>
        {PRESETS.map((p) => (
          <label key={p} className="tim-workflow__preset">
            <input
              type="radio"
              name="preset"
              checked={w.preset === p}
              onChange={() => onSet?.(preset(p, w.branches.pattern))}
            />
            <span>
              <strong>{t.presets[p].name}</strong>
              <span className="tim-tracking__quiet"> · {t.presets[p].text}</span>
            </span>
          </label>
        ))}
        {w.preset === "custom" && <p className="tim-tracking__quiet">{t.presets.custom.text}</p>}
      </fieldset>
      <fieldset className="tim-workflow__group">
        <legend>{t.workflowSettings}</legend>
        {choice<TicketProvider>(
          t.ticketsLabel,
          w.tickets.provider,
          [
            ["none", t.none],
            ["github", "GitHub Issues"],
          ],
          (w, v) => (w.tickets.provider = v),
        )}
        {check(t.branchPerTask, w.branches.per_task, (w, on) => (w.branches.per_task = on))}
        <label className="tim-workflow__setting">
          <span>{t.branchPattern}</span>
          {/* Sent once typed: a half-written pattern would be refused at each letter. */}
          <input
            key={w.branches.pattern}
            type="text"
            defaultValue={w.branches.pattern}
            onBlur={(e) => {
              const pattern = e.target.value;
              if (pattern !== w.branches.pattern) set((w) => ((w.branches.pattern = pattern), w));
            }}
          />
        </label>
        {choice<CommitConvention>(
          t.commitConvention,
          w.commits.convention,
          [
            ["conventional", "Conventional Commits"],
            ["free", t.free],
          ],
          (w, v) => (w.commits.convention = v),
        )}
        {check(t.referenceTicket, w.commits.reference_ticket, (w, on) => (w.commits.reference_ticket = on))}
        {check(t.prAtEnd, w.pull_requests.open_at_session_end, (w, on) => (w.pull_requests.open_at_session_end = on))}
        {check(t.prDraft, w.pull_requests.draft, (w, on) => (w.pull_requests.draft = on))}
        {check(t.humanApproval, w.merge.human_approval, (w, on) => (w.merge.human_approval = on))}
        {check(t.autoMerge, w.merge.auto, (w, on) => (w.merge.auto = on))}
        {choice<MergeStrategy>(
          t.strategy,
          w.merge.strategy,
          [
            ["squash", "squash"],
            ["merge", "merge"],
            ["rebase", "rebase"],
          ],
          (w, v) => (w.merge.strategy = v),
        )}
        {choice<CiProvider>(
          t.ciLabel,
          w.ci.provider,
          [
            ["none", t.none],
            ["github_actions", "GitHub Actions"],
          ],
          (w, v) => (w.ci.provider = v),
        )}
      </fieldset>
      <div role="status" className="tim-workflow__status">
        {problem && <p className="tim-workflow__problem">{t.workflowRefused(problem)}</p>}
        {view.problems.map((p) => (
          <p key={p} className="tim-workflow__problem">
            {p}
          </p>
        ))}
      </div>
      <h2 className="tim-tracking__heading">{t.workflowAgent}</h2>
      <ul className="tim-plain">
        {view.instructions.split("\n").map((line) => (
          <li key={line} className="tim-tracking__quiet">
            {line.replace(/^- /, "")}
          </li>
        ))}
      </ul>
    </section>
  );
}
