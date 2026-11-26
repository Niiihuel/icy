// JSNIR owns the caret, selection, native IME and undo stack while the field is focused.
// Only dirty drafts outlive a pane, so preview navigation and closing a pane don't lose work.
interface EditorDraft {
  spec: TileSpec;
  path: string;
  original: string;
  baseline: string;
  text: string;
  dirty: Signal<boolean>;
  pending: Signal<boolean>;
  status: Signal<string>;
}

const editorDrafts = new Map<string, EditorDraft>();
const editorSaves = new Map<number, { draft: EditorDraft; submitted: string }>();
let editorSaveToken = 0;

function editorText(source: string): string {
  return source.replace(/^\uFEFF/, "").replace(/\r\n/g, "\n");
}

function updateEditorDraft(draft: EditorDraft, text: string): void {
  draft.text = text;
  draft.dirty.value = text !== draft.baseline;
  const key = docKey(draft.spec);
  if (draft.dirty.value || draft.pending.value) editorDrafts.set(key, draft);
  else if (editorDrafts.get(key) === draft) editorDrafts.delete(key);
}

function saveEditorDraft(draft: EditorDraft | undefined): void {
  if (!draft || !draft.dirty.value || draft.pending.value) return;
  const token = ++editorSaveToken;
  const text = draft.text;
  draft.pending.value = true;
  draft.status.value = "Saving…";
  editorSaves.set(token, { draft, submitted: text });
  if (!requestHost({ type: "saveEditor", token, path: draft.path, expected: draft.original, text })) {
    editorSaves.delete(token);
    draft.pending.value = false;
    draft.status.value = "Could not start the save.";
  }
}

function receiveEditorSaved(result: { token: number; saved?: string; error?: string }): void {
  const operation = editorSaves.get(result.token);
  if (!operation) return;
  editorSaves.delete(result.token);
  const { draft, submitted } = operation;
  draft.pending.value = false;
  if (result.saved === undefined) {
    draft.status.value = result.error ?? "Could not save the file.";
    updateEditorDraft(draft, draft.text);
    return;
  }
  draft.original = result.saved;
  draft.baseline = editorText(draft.original);
  updateEditorDraft(draft, draft.text);
  draft.status.value = draft.text === submitted ? "Saved" : "Saved · newer edits are not saved";
  loadTile(draft.spec);
  requestChanges(draft.spec.cwd);
}

function editorContext(path: string, start: number, end: number, text: string): string {
  return `${path.replace(/[\r\n]/g, " ")}:${start}${end > start ? `-${end}` : ""}\n\n${text}`;
}

function editorSelection(path: string, text: string, [anchor, head]: [number, number]): string {
  const start = Math.min(anchor, head);
  const end = Math.max(anchor, head);
  if (start === end) return "";
  // JSNIR Field.selection() is in Unicode scalar offsets, not JavaScript UTF-16 units.
  const chars = Array.from(text);
  const first = chars.slice(0, start).join("").split("\n").length;
  const selected = chars.slice(start, end).join("");
  const last = first + selected.replace(/\n$/, "").split("\n").length - 1;
  return editorContext(path, first, last, selected);
}

// Most recently focused CLI sessions first; "Add to Chat" pastes into the first one still running.
const recentSessions: number[] = [];
effect(() => {
  const id = tilingState.value.focusedSession;
  if (!isTerminalId(id)) return;
  const at = recentSessions.indexOf(id);
  if (at >= 0) recentSessions.splice(at, 1);
  recentSessions.unshift(id);
});

interface BlameView { author: string; time: number; summary: string; uncommitted: boolean }
interface Caret { line: number; col: number; selected: number }

// Keyed by document; the page's SourceEditor registers its "Add to Chat" action here.
const editorChats = new Map<string, () => void>();
const blameCache = new Map<string, Map<number, BlameView | null>>();
const branchCache = new Map<string, string>();
const blameVersion = signal(0);

function cliSessions(cwd: string) {
  return sessionsState.value.items.filter((s) => s.alive && s.cli !== "terminal" && s.cwd === cwd);
}

function chatTarget<T extends { id: number }>(sessions: T[]): T | undefined {
  if (sessions.length <= 1) return sessions[0];
  for (const id of recentSessions) {
    const match = sessions.find((s) => s.id === id);
    if (match) return match;
  }
  return undefined;
}

function pasteContext(session: { id: number; cli: string }, cwd: string, text: string): boolean {
  return requestHost({ type: "pasteEditorContext", id: session.id, cli: session.cli, cwd, text });
}

function orderPoints(a: TextPoint, b: TextPoint): [TextPoint, TextPoint] {
  return a[0] < b[0] || (a[0] === b[0] && a[1] <= b[1]) ? [a, b] : [b, a];
}

// A selection ending at column 0 of a later line stops at the end of the line before it.
function lastLine(start: TextPoint, end: TextPoint): number {
  return end[1] === 0 && end[0] > start[0] ? end[0] - 1 : end[0];
}

function previewText(lines: ViewerLine[], start: TextPoint, end: TextPoint): string {
  const last = lastLine(start, end);
  const out: string[] = [];
  for (let i = start[0]; i <= last && i < lines.length; i++) {
    const text = lines[i].text;
    out.push(text.slice(i === start[0] ? start[1] : 0, i === end[0] ? end[1] : text.length));
  }
  return out.join("\n");
}

function previewContext(path: string, lines: ViewerLine[], start: TextPoint, end: TextPoint): string {
  return editorContext(path, start[0] + 1, lastLine(start, end) + 1, previewText(lines, start, end));
}

// The selected columns on one 1-based line, or null when the line is outside it. A selection over
// several lines has square fills, so its lines join into one block like VS Code's.
function lineSelection(lines: ViewerLine[], start: TextPoint, end: TextPoint, num: number): LineSelection | null {
  const index = num - 1;
  if (index < start[0] || index > end[0]) return null;
  const radius = start[0] === end[0] ? editorLayout.selectionRadius : 0;
  const from = index === start[0] ? start[1] : 0;
  // A line the selection continues past also takes its line break: one cell after the text, as VS Code.
  const to = index === end[0] ? end[1] : (lines[index]?.text.length ?? 0) + 1;
  return from !== to ? [from, to, radius] : null;
}

// Caret position of a text field from JSNIR's Unicode-scalar selection offsets.
function fieldCaret(text: string, anchor: number, head: number): Caret {
  const before = Array.from(text).slice(0, head);
  const lineStart = before.lastIndexOf("\n") + 1;
  return {
    line: before.filter((ch) => ch === "\n").length + 1,
    col: before.length - lineStart + 1,
    selected: Math.abs(head - anchor),
  };
}

function blameText(blame: BlameView): string {
  if (blame.uncommitted) return "Not committed yet";
  const ago = shortAgo(blame.time);
  return `${blame.author}, ${ago === "now" ? "just now" : `${ago} ago`}`;
}

function receiveGitBlame(result: { path: string; line: number; blame: BlameView | null; branch: string | null }): void {
  let lines = blameCache.get(result.path);
  if (!lines) {
    lines = new Map();
    blameCache.set(result.path, lines);
  }
  lines.set(result.line, result.blame);
  if (result.branch) branchCache.set(result.path, result.branch);
  blameVersion.value++;
}

function addFocusedSelectionToChat(): void {
  const spec = focusedTile();
  if (spec?.kind === "file") editorChats.get(docKey(spec))?.();
}

// Which CLI gets the context when several could: anchored to the editor body, since the chip
// is gone once the menu opens from the keyboard.
function SessionMenu(props: { cwd: string; open: Signal<boolean>; captured: Signal<string>; anchor: () => number | undefined }): Element {
  const { open, captured } = props;
  const sessions = () => cliSessions(props.cwd);
  return (
    <MenuPanel open={open} anchor={props.anchor} align="end" width={spacing.menuWidth}>
      <MenuLabel label="Paste context without submitting" />
      <MenuItem label="Copy context" icon="copy" onPress={() => {
        copyText(captured.value);
        open.value = false;
      }} />
      <MenuSeparator />
      <Column style={{ maxHeight: editorLayout.sessionListHeight, scroll: true }}>
        <For each={sessions} key={(s) => s.id}>
          {(item) => <MenuItem label={() => cleanTitle(item().title) || cliDisplayName(item().cli)}
            description={() => `${cliDisplayName(item().cli)} · #${item().id}`}
            image={() => `cli:${item().cli}`} onPress={() => {
              if (pasteContext(item(), props.cwd, captured.value)) open.value = false;
            }} />}
        </For>
      </Column>
    </MenuPanel>
  );
}

function AddToChatChip(props: { onPress: () => void }): Element {
  return (
    <Row
      style={{ height: editorLayout.chipHeight, padding: editorLayout.chipPadding, gap: spacing.unit * 1.5, align: "center" }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: palette().appBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        hover: { edge: hoverBorder() },
        cursor: "pointer",
        a11y: { role: "button", label: "Add to Chat" },
      })}
    >
      <Text font={{ family: fontFamilies.sans, size: typeScale.caption }} paint={() => ({ color: palette().textSecondary })}>Add to Chat</Text>
      <For each={() => keysFor("editor.addToChat")} key={(key) => key}
        style={{ direction: "row", gap: spacing.unit, align: "center" }}>
        {(key) => <KbdKey label={() => key()} />}
      </For>
    </Row>
  );
}

// Right-click menu of the source editor, in the house menu style the Add to Chat chip shares:
// app background, hairline border and each action's key chips. Edits run as their keys would.
function EditorMenu(props: { open: Signal<boolean>; anchor: () => number | undefined; selected: () => boolean; onAddToChat: () => void }): Element {
  const run = (command: string) => () => {
    props.open.value = false;
    requestHost({ type: "editorCommand", command });
  };
  return (
    <MenuPanel open={props.open} anchor={props.anchor} width={spacing.menuWidth}>
      <Show when={props.selected}>
        {() => (
          <Column style={{ gap: 1 }}>
            <MenuItem icon="message-square-plus" label="Add to Chat" keys={() => keysFor("editor.addToChat")} onPress={() => {
              props.open.value = false;
              props.onAddToChat();
            }} />
            <MenuSeparator />
          </Column>
        )}
      </Show>
      <MenuItem icon="scissors" label="Cut" keys={chordParts("Ctrl+X")} onPress={run("cut")} />
      <MenuItem icon="copy" label="Copy" keys={chordParts("Ctrl+C")} onPress={run("copy")} />
      <MenuItem icon="clipboard" label="Paste" keys={chordParts("Ctrl+V")} onPress={run("paste")} />
      <MenuSeparator />
      <MenuItem icon="select-all" label="Select All" keys={chordParts("Ctrl+A")} onPress={run("selectAll")} />
    </MenuPanel>
  );
}

// shortcut: JSNIR's create-input command takes an initial selection but `TextInput` has no prop
// for it, so this mirrors sdk.ts `TextInputElement.mount` with one. Use `TextInput` once it has it.
class CodeInputElement extends TextInputElement {
  selection: [number, number];
  constructor(props: TextInputProps, selection: [number, number]) {
    super(props);
    this.selection = selection;
  }
  mount(parent: number | undefined, before: number | undefined, created: number[] | null): number {
    const props = this.props;
    const id = newId();
    const field: Field = {
      text: () => jsnir.readText(id) ?? "",
      selection: () => [jsnir.columns.selectionStart[id], jsnir.columns.selectionEnd[id]],
      rect: () => [jsnir.columns.x[id], jsnir.columns.y[id], jsnir.columns.width[id], jsnir.columns.height[id]],
      replace: (text: string, expected?: string) => emitSetField(id, text, expected ?? (jsnir.readText(id) ?? "")),
    };
    const change = props.onChange !== undefined ? register(props.onChange) : 0;
    const paint = props.paint === undefined ? undefined : bindId(props.paint, id, emitSetPaint);
    emitCreateInput(id, parent, before, props.value, this.selection, 0, change, props.placeholder,
      props.placeholderColor, props.multiline, props.submitOnEnter, props.retainOnSubmit, props.purpose,
      props.style, props.font, paint);
    if (created !== null) created.push(id);
    if (props.autoFocus) emitFocus(id);
    if (props.ref !== undefined) props.ref(field);
    return id;
  }
}

function CodeInput(props: TextInputProps & { selection: [number, number] }): Element {
  return new CodeInputElement(props, props.selection);
}

// A key on a file's read view (`window.rs` `editor_preview_key`), or Escape from its editor.
interface EditorKey {
  insert?: string;
  // true: Backspace, false: Delete.
  delete?: boolean;
  command?: "selectAll" | "copy" | "cut" | "escape";
}

const editorKeys = new Map<string, (key: EditorKey) => void>();
// Per document: true while its editor is open. The read view of the focused file takes the keys.
const editorModes = signal<Record<string, boolean>>({});
let editorPreviewOn = false;
effect(() => {
  const spec = focusedTile();
  const on = activeNav.value === "agents" && !overviewOpen2.value && spec?.kind === "file"
    && editorModes.value[docKey(spec)] === false;
  if (on !== editorPreviewOn && requestHost({ type: "editorPreview", on })) editorPreviewOn = on;
});

function receiveEditorKey(key: EditorKey): void {
  const spec = focusedTile();
  if (spec?.kind === "file") editorKeys.get(docKey(spec))?.(key);
}

// Character offset of a [line, UTF-16 column] point in `text` (JSNIR fields count characters).
function charOffset(text: string, [line, col]: TextPoint): number {
  const lines = text.split("\n");
  const row = Math.min(line, lines.length - 1);
  const before = lines.slice(0, row).join("\n") + (row > 0 ? "\n" : "");
  return Array.from(before + lines[row].slice(0, col)).length;
}

// `text` with the [from, to) characters replaced the way a key edits them, and the caret after it.
function applyEditorKey(text: string, [from, to]: [number, number], key: EditorKey): { text: string; caret: number } {
  const chars = Array.from(text);
  if (key.insert !== undefined) {
    chars.splice(from, to - from, ...Array.from(key.insert));
    return { text: chars.join(""), caret: from + Array.from(key.insert).length };
  }
  if (key.delete === undefined) return { text, caret: to };
  if (from !== to) {
    chars.splice(from, to - from);
    return { text: chars.join(""), caret: from };
  }
  if (key.delete && from > 0) {
    chars.splice(from - 1, 1);
    return { text: chars.join(""), caret: from - 1 };
  }
  if (!key.delete && from < chars.length) chars.splice(from, 1);
  return { text: chars.join(""), caret: from };
}

// VS Code-like: the file reads with highlighting and line numbers; select with the pointer or
// Ctrl+A, copy with Ctrl+C, and typing, pasting or a double click edits in place. Esc returns a
// clean editor to the read view; Ctrl+S saves. A selection offers "Add to Chat".
function SourceEditor(props: { spec: TileSpec; doc: () => ViewerView; paintOffset?: () => [number, number] }): Element {
  const key = docKey(props.spec);
  const draft = signal<EditorDraft | undefined>(editorDrafts.get(key));
  const editing = signal(draft.value !== undefined);
  const sel = signal<{ anchor: TextPoint; head: TextPoint } | null>(null);
  // Where the last press landed on the read view: the caret when editing starts.
  let point: TextPoint = [0, 0];
  let pressedAt = 0;
  // The editor field's selection when it opens, in characters.
  let openSelection: [number, number] = [0, 0];
  const chipAt = signal<RowPoint | null>(null);
  const caret = signal<Caret>({ line: 0, col: 0, selected: 0 });
  const menuOpen = signal(false);
  const menuText = signal("");
  const fieldBox = signal<[number, number]>([0, 0]);
  const contextOpen = signal(false);
  const contextAt = signal<[number, number]>([0, 0]);
  let contextAnchor: number | undefined;
  let bodyId: number | undefined;
  const confirmingDiscard = signal(false);
  let field: Field | undefined;
  let lastLines: ViewerLine[] | undefined;
  effect(() => {
    const doc = props.doc();
    if (lastLines !== doc.lines) {
      sel.value = null;
      chipAt.value = null;
      caret.value = { line: 0, col: 0, selected: 0 };
      blameCache.delete(doc.path);
    }
    lastLines = doc.lines;
  });
  effect(() => {
    const on = editing.value;
    editorModes.update((modes) => ({ ...modes, [key]: on }));
  });
  onCleanup(() => editorModes.update((modes) => {
    const { [key]: _, ...rest } = modes;
    return rest;
  }));
  // A clean editor closes when its window loses focus.
  effect(() => {
    const spec = focusedTile();
    if (editing.value && (!spec || docKey(spec) !== key) && !draft.value?.dirty.value && !draft.value?.pending.value) {
      editing.value = false;
    }
  });

  // Blame for the caret line, requested once the caret rests and the draft is saved.
  let blameToken = 0;
  effect(() => {
    const c = caret.value;
    const doc = props.doc();
    const token = ++blameToken;
    if (c.line === 0 || draft.value?.dirty.value || blameCache.get(doc.path)?.has(c.line)) return;
    afterView(editorLayout.blameDelay, () => {
      if (token === blameToken && !blameCache.get(doc.path)?.has(c.line)) {
        requestHost({ type: "gitBlame", path: doc.path, line: c.line });
      }
    });
  });

  // JSNIR's field has no selection-change event or caret geometry, so the caret is polled.
  let pollToken = 0;
  const pollField = () => {
    const token = ++pollToken;
    let last = "";
    const tick = () => {
      if (!field || token !== pollToken) return;
      const [a, b] = field.selection();
      const text = field.text();
      const stamp = `${a}:${b}:${text.length}`;
      if (stamp !== last) {
        last = stamp;
        caret.value = fieldCaret(text, a, b);
      }
      // Field.rect() is in window space; the chip is positioned inside the body column.
      const [x, y, w, h] = field.rect();
      const ox = bodyId === undefined ? 0 : jsnir.columns.x[bodyId] ?? 0;
      const oy = bodyId === undefined ? 0 : jsnir.columns.y[bodyId] ?? 0;
      const right = x + w - ox;
      const bottom = y + h - oy;
      if (fieldBox.value[0] !== right || fieldBox.value[1] !== bottom) fieldBox.value = [right, bottom];
      afterView(editorLayout.caretPollMs, tick);
    };
    tick();
  };

  const ordered = (): [TextPoint, TextPoint] => {
    const s = sel.value;
    return s ? orderPoints(s.anchor, s.head) : [point, point];
  };
  // Opens the editor at the read view's caret or selection, applying the key that opened it.
  const edit = (editKey?: EditorKey) => {
    const doc = props.doc();
    let d = draft.value;
    if (!d?.dirty.value && !d?.pending.value) {
      if (doc.editableText === undefined) {
        if (doc.editUnavailable) noticeState.value = { kind: "info", text: doc.editUnavailable };
        return;
      }
      const text = editorText(doc.editableText);
      d = {
        spec: props.spec, path: doc.path, original: doc.editableText, text, baseline: text,
        dirty: signal(false), pending: signal(false), status: signal(""),
      };
      draft.value = d;
    }
    const [start, end] = ordered();
    const range: [number, number] = [charOffset(d.text, start), charOffset(d.text, end)];
    const next = editKey ? applyEditorKey(d.text, range, editKey) : { text: d.text, caret: range[1] };
    if (next.text !== d.text) updateEditorDraft(d, next.text);
    openSelection = editKey ? [next.caret, next.caret] : range;
    sel.value = null;
    chipAt.value = null;
    editing.value = true;
  };
  const context = () => {
    const doc = props.doc();
    if (editing.value && field) return editorSelection(doc.displayPath, field.text(), field.selection());
    const s = sel.value;
    if (!s || caret.value.selected === 0) return "";
    const [start, end] = orderPoints(s.anchor, s.head);
    return previewContext(doc.displayPath, doc.lines, start, end);
  };
  const addToChat = () => {
    const text = context();
    if (!text) return;
    const cwd = props.spec.cwd ?? workspacePath.value;
    const sessions = cliSessions(cwd);
    if (sessions.length === 0) {
      noticeState.value = { kind: "info", text: "No running CLI in this project" };
      return;
    }
    const target = chatTarget(sessions);
    if (target) {
      pasteContext(target, cwd, text);
      return;
    }
    menuText.value = text;
    menuOpen.value = true;
  };
  editorChats.set(key, addToChat);
  onCleanup(() => { if (editorChats.get(key) === addToChat) editorChats.delete(key); });

  const selectedText = () => {
    const [start, end] = ordered();
    return previewText(props.doc().lines, start, end);
  };
  const onKey = (k: EditorKey) => {
    if (editing.value) {
      if (k.command === "escape" && !draft.value?.dirty.value && !draft.value?.pending.value) editing.value = false;
      return;
    }
    const lines = props.doc().lines;
    switch (k.command) {
      case "selectAll": {
        if (!lines.length) return;
        const last = lines.length - 1;
        sel.value = { anchor: [0, 0], head: [last, lines[last].text.length] };
        chipAt.value = null;
        caret.value = { line: last + 1, col: lines[last].text.length + 1, selected: Array.from(selectedText()).length };
        return;
      }
      case "copy":
      case "cut": {
        // No selection: the caret's whole line, as VS Code copies it.
        const text = sel.value ? selectedText() : `${lines[point[0]]?.text ?? ""}\n`;
        copyText(text);
        if (k.command === "cut" && sel.value) edit({ insert: "" });
        return;
      }
      case "escape":
        sel.value = null;
        chipAt.value = null;
        return;
    }
    edit(k);
  };
  editorKeys.set(key, onKey);
  onCleanup(() => { if (editorKeys.get(key) === onKey) editorKeys.delete(key); });

  const status = () => {
    const d = draft.value;
    if (editing.value) return d?.status.value || (d?.dirty.value ? "Unsaved changes" : "");
    return props.doc().editUnavailable ?? "";
  };
  return (
    <Column style={{ grow: 1, minHeight: 0, minWidth: 0 }}>
      <Column ref={(id) => { bodyId = id; }} style={{ grow: 1, minHeight: 0, minWidth: 0 }}
        onContextMenu={({ x, y }) => {
          // The pointer is in window space; the anchor sits in the body column.
          const ox = bodyId === undefined ? 0 : jsnir.columns.x[bodyId] ?? 0;
          const oy = bodyId === undefined ? 0 : jsnir.columns.y[bodyId] ?? 0;
          contextAt.value = [x - ox, y - oy];
          contextOpen.value = true;
        }}>
        <Dynamic value={editing} style={{ grow: 1, minHeight: 0, minWidth: 0 }}>
          {(on) => {
            if (!on) return <SourceLines doc={props.doc} paintOffset={props.paintOffset}
              select={(anchor, head, phase, at) => {
                if (phase === "start") {
                  // A second press on the same line soon after the first edits there.
                  const now = Date.now();
                  const again = now - pressedAt < editorLayout.doubleClickMs && point[0] === anchor[0];
                  pressedAt = again ? 0 : now;
                  point = anchor;
                  if (again) {
                    sel.value = null;
                    edit();
                    return;
                  }
                }
                const [start, end] = orderPoints(anchor, head);
                const empty = start[0] === end[0] && start[1] === end[1];
                point = head;
                sel.value = empty ? null : { anchor, head };
                chipAt.value = phase === "end" && !empty ? at : null;
                caret.value = {
                  line: head[0] + 1,
                  col: head[1] + 1,
                  selected: empty ? 0 : Array.from(previewText(props.doc().lines, start, end)).length,
                };
              }}
              selection={(num) => {
                const s = sel.value;
                if (!s) return null;
                const [start, end] = orderPoints(s.anchor, s.head);
                return lineSelection(props.doc().lines, start, end, num);
              }}
              overlay={() => (
                <Show when={() => !!chipAt.value && caret.value.selected > 0}
                  style={() => {
                    const p = chipAt.value ?? { x: 0, y: 0, top: 0, width: 0 };
                    return {
                      position: "absolute",
                      layer: editorLayout.chipLayer,
                      left: Math.max(CODE_GUTTER, Math.min(p.x, p.width - editorLayout.chipWidth)),
                      top: p.top + (Math.floor(p.y / CODE_LINE) + 1) * CODE_LINE + editorLayout.chipGap,
                    };
                  }}>
                  {() => <AddToChatChip onPress={addToChat} />}
                </Show>
              )} />;
            const d = draft.value!;
            onCleanup(() => { field = undefined; });
            return <CodeInput multiline autoFocus value={d.text} selection={openSelection} font={CODE_FONT}
              ref={(input) => {
                field = input;
                if (input) pollField();
              }}
              style={{ grow: 1, minHeight: 0, minWidth: 0, padding: [spacing.unit * 2, spacing.unit * 3, spacing.unit * 2, spacing.unit * 3] }}
              paint={() => ({ color: palette().textPrimary, scrollbar: editorScrollbar(), a11y: { label: "Source editor" } })}
              onChange={(text) => {
                if (!d.pending.value) d.status.value = "";
                confirmingDiscard.value = false;
                updateEditorDraft(d, text);
              }} />;
          }}
        </Dynamic>
        <Show when={() => editing.value && caret.value.selected > 0}
          style={() => ({
            position: "absolute",
            layer: editorLayout.chipLayer,
            left: fieldBox.value[0] - editorLayout.chipWidth - editorLayout.chipGap,
            top: fieldBox.value[1] - editorLayout.chipHeight - editorLayout.chipGap,
          })}>
          {() => <AddToChatChip onPress={addToChat} />}
        </Show>
        <SessionMenu cwd={props.spec.cwd ?? workspacePath.value} open={menuOpen} captured={menuText} anchor={() => bodyId} />
        <Box ref={(id) => { contextAnchor = id; }}
          style={() => ({ position: "absolute", left: contextAt.value[0], top: contextAt.value[1], width: 0, height: 0 })} />
        <EditorMenu open={contextOpen} anchor={() => contextAnchor} selected={() => caret.value.selected > 0} onAddToChat={addToChat} />
      </Column>
      <EditorFooter doc={props.doc} caret={() => caret.value} status={status}>
        <Show when={() => !!draft.value?.dirty.value && !draft.value?.pending.value} style={{ shrink: 0 }}>
          {() => <IconButton icon="trash" label={() => confirmingDiscard.value ? "Confirm discard" : "Discard changes"} size="sm"
            highlighted={confirmingDiscard} onPress={() => {
              if (!confirmingDiscard.value) { confirmingDiscard.value = true; return; }
              editorDrafts.delete(key);
              draft.value = undefined;
              confirmingDiscard.value = false;
              editing.value = false;
            }} />}
        </Show>
      </EditorFooter>
    </Column>
  );
}

function footerItems(doc: ViewerView, c: Caret): string[] {
  const items: string[] = [];
  if (c.line > 0) items.push(`Ln ${c.line}, Col ${c.col}${c.selected > 0 ? ` (${c.selected} selected)` : ""}`);
  const info = doc.textInfo;
  if (info) {
    if (info.indent) items.push(info.indent.kind === "tabs" ? `Tab Size: ${editorLayout.tabColumns}` : `Spaces: ${info.indent.size}`);
    items.push(info.bom ? "UTF-8 with BOM" : "UTF-8");
    items.push(info.crlf ? "CRLF" : "LF");
    items.push(info.language);
  }
  return items;
}

// Status line under the source: branch and blame on the left, then the editor's status (saving,
// unsaved changes, why the file is read-only) and its actions; caret and file facts on the right.
function EditorFooter(props: { doc: () => ViewerView; caret: () => Caret; status: () => string; children?: Children }): Element {
  const branch = () => {
    void blameVersion.value;
    return branchCache.get(props.doc().path) ?? "";
  };
  const blame = () => {
    void blameVersion.value;
    const b = blameCache.get(props.doc().path)?.get(props.caret().line);
    return b ? blameText(b) : "";
  };
  const footerText = { family: fontFamilies.sans, size: typeScale.caption };
  return (
    <Row
      style={{ height: editorLayout.footerHeight, padding: editorLayout.footerPadding, gap: editorLayout.footerGap, align: "center", shrink: 0 }}
      paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [1, 0, 0, 0] })}
    >
      <Row style={{ grow: 1, minWidth: 0, gap: spacing.unit * 2, align: "center", clip: true }}>
        <Show when={() => !!branch()}>
          {() => (
            <Row style={{ gap: spacing.unit, align: "center", shrink: 0 }}>
              <Glyph name="git-branch" size={editorLayout.footerIcon} color={() => palette().textMuted} />
              <Text lines={1} font={footerText} paint={() => ({ color: palette().textMuted })}>{branch}</Text>
            </Row>
          )}
        </Show>
        <Text lines={1} style={{ minWidth: 0 }} font={footerText} paint={() => ({ color: palette().textMuted })}>{blame}</Text>
      </Row>
      <Row style={{ shrink: 0, gap: spacing.unit, align: "center" }}>
        <Text lines={1} font={footerText} paint={() => ({ color: palette().textSecondary })}>{props.status}</Text>
        {props.children}
      </Row>
      <Row style={{ shrink: 0, gap: spacing.unit * 3, align: "center" }}>
        <For each={() => footerItems(props.doc(), props.caret())} key={(text) => text}
          style={{ direction: "row", gap: spacing.unit * 3, align: "center" }}>
          {(text) => <Text lines={1} font={footerText} paint={() => ({ color: palette().textMuted })}>{() => text()}</Text>}
        </For>
      </Row>
    </Row>
  );
}

function SpecialFilePreview(props: { keyForDoc: () => ViewerView }): Element {
  const font = () => {
    const preview = props.keyForDoc().preview;
    return preview?.kind === "font" ? preview : undefined;
  };
  return (
    <Column style={{ grow: 1, minHeight: 0, padding: emptyStateLayout.padding, gap: spacing.unit * 3, scroll: true }}>
      <Text font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textPrimary })}>{() => font()?.name ?? "Binary or unsupported text encoding"}</Text>
      <Text font={{ family: fontFamilies.sans, size: typeScale.caption }} paint={() => ({ color: palette().textMuted })}>
        {() => font() ? "Font preview · read-only" : "Use Open in… to view this file in another application."}
      </Text>
      <Dynamic value={() => font()?.key}>
        {(key) => {
          const size = font()?.size;
          if (!key || !size) return <Box />;
          return <Column style={{ padding: emptyStateLayout.padding, clip: true }}
          paint={{ background: editorLayout.sampleBackground, radius: radii.lg }}>
            <Image source={key} style={{ width: size[0] / editorLayout.sampleScale,
              height: size[1] / editorLayout.sampleScale, shrink: 0 }} />
          </Column>;
        }}
      </Dynamic>
    </Column>
  );
}
