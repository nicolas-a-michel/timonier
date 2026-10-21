import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { EmptyState } from "../design/components";
import { connectHost } from "../host";
import { resolveLocale } from "../i18n";
import { isWhiteboardMessage, readyMessage } from "../protocol";
import { EMPTY_BOARD, type Board } from "./board";
import { boardOf, intentOf } from "./intent";
import { whiteboardMessages } from "./messages";
import { Whiteboard, type Saved, type WhiteboardCommands } from "./Whiteboard";
import "../tokens.css";
import "../design/design.css";
import "./whiteboard.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Timonier webview: the #root element is missing.");
}

const locale = resolveLocale(document.documentElement.lang || navigator.language);
const t = whiteboardMessages[locale];
const host = connectHost();
const root = createRoot(container);
const commands: WhiteboardCommands = { current: null, pending: [] };
// The board is `.timonier/intent.json` (ADR 0039): the host sends it when the page opens, and
// again when the file changes outside the page. Without a host (the development page), an
// empty one at once.
let board: Board | undefined = host ? undefined : EMPTY_BOARD;
let problem: string | null = null;
/** A new board from the host: the whiteboard starts again from it, its history too. */
let revision = 0;
let saved: Saved | null = null;
// After each gesture, the board goes to the host, which writes it (Q66).
const onChange = (next: Board) => host?.postMessage({ type: "intent", intent: intentOf(next) });

function render() {
  root.render(
    <StrictMode>
      {problem !== null ? (
        <EmptyState
          title={t.unreadTitle}
          text={t.unreadText}
          detail={problem}
          action={{
            label: t.startOver,
            onSelect: () => {
              problem = null;
              board = EMPTY_BOARD;
              revision++;
              render();
            },
          }}
        />
      ) : board ? (
        <Whiteboard
          key={revision}
          locale={locale}
          board={board}
          onChange={onChange}
          commandsRef={commands}
          saved={saved}
        />
      ) : null}
    </StrictMode>,
  );
}

window.addEventListener("message", (event: MessageEvent<unknown>) => {
  const message = event.data;
  if (!isWhiteboardMessage(message)) return;
  if (message.type === "intent") {
    const read = boardOf(message.intent, locale);
    problem = message.problem ?? read.problem;
    board = read.board;
    revision++;
    render();
  } else if (message.type === "intentSaved") {
    saved = { saved: message.saved, detail: message.detail };
    render();
  } else if (commands.current) commands.current(message.command);
  else commands.pending.push(message.command);
});

render();
// The script runs under the CSP, and waits for its board: the host answers with it.
host?.postMessage(readyMessage());
