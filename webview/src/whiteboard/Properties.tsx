/**
 * The properties of the selected shape (ADR 0038): what it is called and does, its policy, its
 * size, what it is attached to, its limits. Everything a typed shape holds is set here from the
 * keyboard too; a field is kept when it loses the focus or on Enter, as one step of the history.
 */
import { createContext, useContext, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

import {
  LIMITS,
  POLICIES,
  type BoxShape,
  type BudgetShape,
  type Limits,
  type NoteShape,
  type Shape,
  type ZoneShape,
} from "./board";
import type { WhiteboardMessages } from "./messages";

export interface PropertiesProps {
  shape: Shape;
  t: WhiteboardMessages;
  /** The name each shape goes by, for the lists. */
  names: ReadonlyMap<string, string>;
  /** The boxes and zones, and the arrows: what an arrow or an attachment can point to. */
  ends: readonly { id: string; kind: Shape["kind"] }[];
  onChange: (shape: Shape) => void;
  onConnect: (src: string, dst: string) => void;
  onRemove: (id: string) => void;
  /** Put the focus on the first field: the shape was just added. */
  focus: boolean;
  /** A field was kept with Enter: the focus goes back to the shape, and the board's keys work again. */
  onDone: () => void;
}

/** What Enter does once a field is kept. */
const Done = createContext<() => void>(() => undefined);

/** A field that keeps its draft until it loses the focus or Enter is pressed. */
function Field({
  label,
  value,
  multiline = false,
  type = "text",
  autoFocus = false,
  onCommit,
}: {
  label: string;
  value: string;
  multiline?: boolean;
  type?: "text" | "number";
  autoFocus?: boolean;
  onCommit: (value: string) => void;
}) {
  const id = useId();
  const done = useContext(Done);
  const [draft, setDraft] = useState(value);
  // Undone or changed elsewhere: the field shows the shape as it is.
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setDraft(value);
  }
  const ref = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  useEffect(() => {
    if (autoFocus) ref.current?.focus();
  }, [autoFocus]);
  const commit = () => {
    if (draft !== value) onCommit(draft);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    // Enter keeps a line; in a text of several, Ctrl+Enter does, and Enter goes to the next line.
    if (event.key === "Enter" && (!multiline || event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      commit();
      done();
    } else if (event.key === "Escape") {
      setDraft(value);
    }
  };
  const common = {
    id,
    ref,
    className: "tim-wb-props__input",
    value: draft,
    onChange: (event: { target: { value: string } }) => setDraft(event.target.value),
    onBlur: commit,
    onKeyDown,
  };
  return (
    <div className="tim-wb-props__field">
      <label htmlFor={id}>{label}</label>
      {multiline ? (
        <textarea {...common} rows={3} />
      ) : (
        <input {...common} type={type} min={type === "number" ? 0 : undefined} step="any" />
      )}
    </div>
  );
}

function Choice({
  label,
  value,
  options,
  onChoose,
}: {
  label: string;
  value: string;
  options: readonly { value: string; label: string }[];
  onChoose: (value: string) => void;
}) {
  const id = useId();
  return (
    <div className="tim-wb-props__field">
      <label htmlFor={id}>{label}</label>
      <select id={id} className="tim-wb-props__input" value={value} onChange={(e) => onChoose(e.target.value)}>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  );
}

const number = (text: string): number | undefined => {
  const value = Number(text.replace(",", "."));
  return text.trim() === "" || !Number.isFinite(value) || value < 0 ? undefined : value;
};

function Size({
  shape,
  t,
  onChange,
}: {
  shape: BoxShape | ZoneShape;
  t: WhiteboardMessages;
  onChange: (s: Shape) => void;
}) {
  return (
    <div className="tim-wb-props__row">
      {(["width", "height"] as const).map((key) => (
        <Field
          key={key}
          label={t[key]}
          type="number"
          value={String(Math.round(shape[key]))}
          onCommit={(text) => {
            const value = number(text);
            if (value !== undefined) onChange({ ...shape, [key]: value });
          }}
        />
      ))}
    </div>
  );
}

/** A new arrow from this box or zone: the keyboard's way of drawing one. */
function NewArrow({ shape, t, names, ends, onConnect }: PropertiesProps) {
  const targets = ends.filter((end) => end.id !== shape.id && (end.kind === "box" || end.kind === "zone"));
  const [to, setTo] = useState(targets[0]?.id ?? "");
  if (targets.length === 0) return null;
  const chosen = targets.some((target) => target.id === to) ? to : (targets[0]?.id ?? "");
  return (
    <div className="tim-wb-props__row">
      <Choice
        label={t.arrowToField}
        value={chosen}
        options={targets.map((target) => ({ value: target.id, label: names.get(target.id) ?? target.id }))}
        onChoose={setTo}
      />
      <button type="button" className="tim-button" onClick={() => onConnect(shape.id, chosen)}>
        {t.addArrow}
      </button>
    </div>
  );
}

function Attachment({ shape, t, names, ends, onChange }: PropertiesProps & { shape: NoteShape | BudgetShape }) {
  return (
    <Choice
      label={t.attachedTo}
      value={shape.attachedTo ?? ""}
      options={[
        { value: "", label: t.nothing },
        ...ends.map((end) => ({ value: end.id, label: names.get(end.id) ?? end.id })),
      ]}
      onChoose={(value) => onChange({ ...shape, attachedTo: value === "" ? null : value })}
    />
  );
}

export function Properties(props: PropertiesProps) {
  const { shape, t, names, onChange, onRemove, focus, onDone } = props;
  const heading = useId();
  return (
    <Done.Provider value={onDone}>
      <section className="tim-wb-props" aria-labelledby={heading}>
        <h2 id={heading} className="tim-wb-props__title">
          {t.kind[shape.kind]}
          <span className="tim-wb-props__id">{shape.id}</span>
        </h2>
        {shape.kind === "box" && (
          <>
            <Field
              label={t.name}
              value={shape.name}
              autoFocus={focus}
              onCommit={(name) => onChange({ ...shape, name })}
            />
            <Field label={t.role} value={shape.role} multiline onCommit={(role) => onChange({ ...shape, role })} />
            <Size shape={shape} t={t} onChange={onChange} />
            <NewArrow {...props} />
          </>
        )}
        {shape.kind === "zone" && (
          <>
            <Field
              label={t.name}
              value={shape.name}
              autoFocus={focus}
              onCommit={(name) => onChange({ ...shape, name })}
            />
            <Size shape={shape} t={t} onChange={onChange} />
            <NewArrow {...props} />
          </>
        )}
        {shape.kind === "arrow" && (
          <>
            <p className="tim-wb-props__ends">
              {t.from} <strong>{names.get(shape.src)}</strong> {t.to.toLowerCase()}{" "}
              <strong>{names.get(shape.dst)}</strong>
            </p>
            <fieldset className="tim-wb-props__policy">
              <legend>{t.kind.arrow}</legend>
              {POLICIES.map((policy) => (
                <label key={policy}>
                  <input
                    type="radio"
                    name={`policy-${shape.id}`}
                    checked={shape.policy === policy}
                    autoFocus={focus && shape.policy === policy}
                    onChange={() => onChange({ ...shape, policy })}
                  />
                  {t.policy[policy]}
                </label>
              ))}
            </fieldset>
          </>
        )}
        {shape.kind === "note" && (
          <>
            <Field
              label={t.text}
              value={shape.text}
              multiline
              autoFocus={focus}
              onCommit={(text) => onChange({ ...shape, text })}
            />
            <Attachment {...props} shape={shape} />
          </>
        )}
        {shape.kind === "budget" && (
          <>
            <Attachment {...props} shape={shape} />
            {LIMITS.map((key: keyof Limits, index) => (
              <Field
                key={key}
                label={t.limit[key]}
                type="number"
                autoFocus={focus && index === 1}
                value={shape[key] === undefined ? "" : String(shape[key])}
                onCommit={(text) => {
                  const next: BudgetShape = { ...shape };
                  const value = number(text);
                  if (value === undefined) delete next[key];
                  else next[key] = value;
                  onChange(next);
                }}
              />
            ))}
          </>
        )}
        <button type="button" className="tim-button" onClick={() => onRemove(shape.id)}>
          {t.remove}
        </button>
      </section>
    </Done.Provider>
  );
}
