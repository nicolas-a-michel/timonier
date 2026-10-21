/**
 * Icons of the design system (ADR 0020): 16 px, drawn in the current colour, always next to a
 * text that says the same. They are decorative for screen readers.
 */
import type { ReactNode } from "react";

import type { Role } from "../protocol";

function Icon({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <svg
      className={className ? `tim-icon ${className}` : "tim-icon"}
      viewBox="0 0 16 16"
      width="16"
      height="16"
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {children}
    </svg>
  );
}

/** One shape per architectural role (ADR 0017). */
const ROLE_SHAPES: Record<Role, ReactNode> = {
  // An arrow going in.
  entry_point: (
    <>
      <path d="M2 8h8" />
      <path d="M7 5l3 3-3 3" />
      <path d="M11 2.5h2.5v11H11" />
    </>
  ),
  // A record with fields.
  data_model: (
    <>
      <rect x="2.5" y="2.5" width="11" height="11" rx="1" />
      <path d="M2.5 6h11M2.5 9.5h11M6.5 6v7.5" />
    </>
  ),
  // A stack of disks.
  persistence: (
    <>
      <ellipse cx="8" cy="4" rx="5" ry="1.8" />
      <path d="M3 4v8c0 1 2.2 1.8 5 1.8s5-.8 5-1.8V4" />
      <path d="M3 8c0 1 2.2 1.8 5 1.8s5-.8 5-1.8" />
    </>
  ),
  // A window.
  interface: (
    <>
      <rect x="2" y="3" width="12" height="10" rx="1" />
      <path d="M2 6h12" />
    </>
  ),
  // A loop.
  background_task: (
    <>
      <path d="M13 8a5 5 0 1 1-1.5-3.5" />
      <path d="M11.8 2v2.8H9" />
    </>
  ),
  // A parcel.
  external_libraries: (
    <>
      <path d="M8 2l5.5 3v6L8 14l-5.5-3V5z" />
      <path d="M2.5 5L8 8l5.5-3M8 8v6" />
    </>
  ),
  // A tool.
  utility: (
    <>
      <circle cx="5" cy="5" r="2.5" />
      <path d="M6.8 6.8L13 13" />
    </>
  ),
};

export function RoleIcon({ role }: { role: Role | null }) {
  if (!role) {
    return (
      <Icon className="tim-icon--role">
        <circle cx="8" cy="8" r="2" />
      </Icon>
    );
  }
  return <Icon className="tim-icon--role">{ROLE_SHAPES[role]}</Icon>;
}

export type Tone = "neutral" | "violation" | "attention" | "conformity";

/** A distinct shape per tone: a dot, a cross, a triangle, a tick. */
export function ToneIcon({ tone }: { tone: Tone }) {
  switch (tone) {
    case "neutral":
      return (
        <Icon className="tim-icon--neutral">
          <circle cx="8" cy="8" r="3" fill="currentColor" />
        </Icon>
      );
    case "violation":
      return (
        <Icon className="tim-icon--violation">
          <circle cx="8" cy="8" r="6" />
          <path d="M5.8 5.8l4.4 4.4M10.2 5.8l-4.4 4.4" />
        </Icon>
      );
    case "attention":
      return (
        <Icon className="tim-icon--attention">
          <path d="M8 2.2l6 10.8H2z" />
          <path d="M8 6.5v3M8 11.3v.1" />
        </Icon>
      );
    case "conformity":
      return (
        <Icon className="tim-icon--conformity">
          <circle cx="8" cy="8" r="6" />
          <path d="M5.2 8.2l1.9 1.9 3.7-4" />
        </Icon>
      );
  }
}

/**
 * What a box is to the whiteboard (ADR 0042): the box a drawn component stands for, a ringed
 * tick; a box that holds one deeper, the same tick in a dashed ring. Two shapes, not two colours.
 */
export function IntentIcon({ kind }: { kind: "drawn" | "within" }) {
  return (
    <Icon className={`tim-icon--intent-${kind}`}>
      <circle cx="8" cy="8" r="6" strokeDasharray={kind === "within" ? "3.6 2.6" : undefined} />
      <path d="M5.2 8.2l1.9 1.9 3.7-4" />
    </Icon>
  );
}

/** What changed since the last review (ADR 0024). */
export type ChangeKind = "added" | "modified" | "cycle" | "fanIn" | "fanOut";

/**
 * A shape per change: a star, a dot, a loop, an arrow coming in, an arrow going out. Filled
 * marks for « new » and « modified », that no one takes for a button (« + » read « add », a pen
 * « edit »: five-second test n° 1, PLAN.md 5.3).
 */
const CHANGE_SHAPES: Record<ChangeKind, ReactNode> = {
  added: <path d="M8 2l1.8 3.9 4.2.5-3.1 2.9.8 4.2L8 11.4l-3.7 2.1.8-4.2L2 6.4l4.2-.5z" fill="currentColor" />,
  modified: <circle cx="8" cy="8" r="3.5" fill="currentColor" />,
  cycle: (
    <>
      <path d="M12.5 6.5A5 5 0 0 0 4 5" />
      <path d="M3.5 9.5A5 5 0 0 0 12 11" />
      <path d="M4 2.5V5h2.5M12 13.5V11H9.5" />
    </>
  ),
  fanIn: (
    <>
      <path d="M2 8h7M6.5 5L9 8l-2.5 3" />
      <path d="M12.5 3v10" />
    </>
  ),
  fanOut: (
    <>
      <path d="M3.5 3v10" />
      <path d="M7 8h7M11.5 5L14 8l-2.5 3" />
    </>
  ),
};

export function ChangeIcon({ kind }: { kind: ChangeKind }) {
  return <Icon className="tim-icon--change">{CHANGE_SHAPES[kind]}</Icon>;
}

/** A tick: a toggle that is on. */
export function CheckIcon() {
  return <Icon>{<path d="M3.5 8.5l3 3 6-7" />}</Icon>;
}

/** An arrow, for the buttons that move a box (ADR 0021): their name says the direction. */
const ARROWS: Record<"left" | "right" | "up" | "down", ReactNode> = {
  left: <path d="M13 8H3M7 4L3 8l4 4" />,
  right: <path d="M3 8h10M9 4l4 4-4 4" />,
  up: <path d="M8 13V3M4 7l4-4 4 4" />,
  down: <path d="M8 3v10M4 9l4 4 4-4" />,
};

export function ArrowIcon({ direction }: { direction: "left" | "right" | "up" | "down" }) {
  return <Icon>{ARROWS[direction]}</Icon>;
}
