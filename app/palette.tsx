// Every launcher entry point shares this modal. A launch target may need a folder, not a second choice.
type PaletteMode = "launcher" | "sessions" | "new-session" | "folder" | "workspaces";
type LauncherTarget = { kind: "project" } | { kind: "session"; label: string; cliId?: string };
interface PaletteRequest {
  mode: PaletteMode;
  cwd: string;
  target?: LauncherTarget;
}

type PaletteResult = {
  key: string;
  icon: string;
  image?: string;
  label: string;
  detail?: string;
} & ({ onSelect: () => void; target?: never } | { target: LauncherTarget; onSelect?: never });

interface PaletteGroup {
  label: string;
  results: PaletteResult[];
  cards?: boolean;
}

type PaletteRow = { key: string; heading: string } | { key: string; result: PaletteResult } |
  { key: string; cards: PaletteResult[]; columns: number };

const paletteRequest = signal<PaletteRequest | null>(null);
let paletteKeyHandler: ((action: string) => void) | undefined;

function togglePalette(mode: "launcher" | "sessions" | "workspaces"): void {
  if (activeNav.value !== "agents") return;
  if (paletteRequest.value?.mode === mode) {
    closePalette();
    return;
  }
  openPalette({ mode, cwd: workspacePath.value });
}

function openPalette(request: PaletteRequest): void {
  if (imageViewer.value || releaseNotesOpen.value || restartConfirmOpen.value) return;
  if (!paletteRequest.value) requestHost({ type: "paletteKeys", on: true });
  paletteRequest.value = request;
}

function showProjectDialog(mode: "session" | "add", path?: string, cliId?: string): void {
  const cwd = path ?? workspacePath.value;
  if (mode === "add") openPalette({ mode: "folder", cwd, target: { kind: "project" } });
  else if (cliId) {
    const label = configState.value.clis.find((cli) => cli.id === cliId)?.name ?? cliId;
    openPalette({ mode: "folder", cwd, target: { kind: "session", label, cliId } });
  } else openPalette({ mode: "new-session", cwd });
}

function closePalette(): void {
  paletteRequest.value = null;
  requestHost({ type: "paletteKeys", on: false });
}

function paletteMatches(query: string, ...values: string[]): boolean {
  const haystack = values.join(" ").toLowerCase();
  return query.toLowerCase().split(/\s+/).every((part) => haystack.includes(part));
}

function paletteSessions(query: string, recent = false): PaletteResult[] {
  const items = sessionsState.value.items;
  return (recent ? items.filter((session) => session.alive).sort((a, b) => b.id - a.id)
    .slice(0, launcherLayout.recentLimit) : items)
    .filter((s) => paletteMatches(query, cleanTitle(s.title), s.cli, s.cwd))
    .map((s) => ({
      key: `session:${s.id}`,
      icon: "square-terminal",
      image: "cli:" + s.cli,
      label: s.cli === "terminal" ? basename(s.cwd) || "Terminal" : cleanTitle(s.title),
      detail: s.cwd,
      onSelect: () => focusTile(s.id),
    }));
}

function launcherAgents(q: string): PaletteResult[] {
  const terminal: PaletteResult = {
    key: "terminal", icon: "square-terminal", label: "Terminal", detail: "Plain shell",
    target: { kind: "session", label: "Terminal" },
  };
  return [terminal, ...configState.value.clis.filter((cli) => cli.enabled).map((cli): PaletteResult => ({
    key: `plugin:${cli.id}`, icon: "puzzle", image: "cli:" + cli.id, label: cli.name, detail: cli.description,
    target: { kind: "session", label: cli.name, cliId: cli.id },
  }))].filter((item) => paletteMatches(q, item.label, item.detail ?? "",
    item.target?.kind === "session" ? item.target.cliId ?? "" : ""));
}

function launcherGroups(q: string, cwd: string): PaletteGroup[] {
  const localUrl = screenState.value.rows.flatMap((row) => row.links)
    .find((link) => link.kind === "url" && isLocalUrl(link.target));
  const session = activeSession();
  const actions: PaletteResult[] = [
    { key: "files", icon: "folder", label: "Open File", onSelect: () => showTile("files", cwd) },
    { key: "review", icon: "git-branch", label: "Git", onSelect: () => showTile("review", cwd) },
    { key: "browser", icon: "globe", label: "Browser", onSelect: () => openBrowser(undefined, cwd) },
    { key: "metrics", icon: "dashboard", label: "Metrics", onSelect: openMetricsTab },
  ];
  if (localUrl) actions.push({ key: "local-url", icon: "globe", label: "Open local URL", detail: localUrl.target,
    onSelect: () => openBrowser(localUrl.target, cwd) });
  if (session && session.cli !== "terminal") actions.push({
    key: "side-question", icon: "message-square-plus", label: "Side Question",
    target: { kind: "session", cliId: session.cli,
      label: configState.value.clis.find((cli) => cli.id === session.cli)?.name ?? session.cli },
  });
  if (q) {
    const view = tilingState.value;
    const column = view.columns.find((column) => column.panes.includes(view.focusedSession));
    if (view.mode === "niri" && !view.maximized && column) actions.push({
      key: "column-width", icon: "columns", label: column.expanded ? "Restore column width" : "Maximize column",
      onSelect: () => tilingAction("maximizeColumn"),
    });
  }

  const artifacts = detectedFilesList().filter((link) => basename(link.target) && paletteMatches(q, link.target))
    .map((link) => ({
      key: `artifact:${link.target}`, icon: "file-code", label: basename(link.target), detail: link.target,
      onSelect: () => openLink(link, session?.id),
    }));
  const groups: PaletteGroup[] = [];
  const address = q ? resolveAddress(q, false) : "";
  if (address) groups.push({ label: "Browser", results: [{ key: "url", icon: "globe", label: address, onSelect: () => openBrowser(address, cwd) }] });
  if (!q) groups.push({ label: "Recent sessions", results: paletteSessions("", true), cards: true });
  groups.push({ label: "New session", results: launcherAgents(q) });
  groups.push({ label: "Actions", results: actions.filter((action) => paletteMatches(q, action.label, action.detail ?? "")) });
  if (q) groups.push({ label: "Sessions", results: paletteSessions(q) });
  if (q && fileSearch.value.cwd === cwd && fileSearch.value.query === q) {
    groups.push({ label: "Files", results: fileSearch.value.items.map((path) => ({
      key: `file:${path}`, icon: "file", label: basename(path), detail: path,
      onSelect: () => openFile(path, undefined, cwd),
    })) });
  }
  groups.push({ label: "Files in output", results: artifacts });
  if (!q) groups.push({ label: "Sessions", results: paletteSessions("").filter((item) =>
    !groups.some((group) => group.cards && group.results.some((recent) => recent.key === item.key))) });
  return groups;
}

function PaletteResultRow(props: { result: () => PaletteResult; selected: () => boolean; onSelect: () => void;
  cardWidth?: () => number }): Element {
  const layout = launcherLayout;
  const card = !!props.cardWidth;
  return (
    <Row
      style={() => ({ height: card ? layout.recentHeight : layout.rowHeight, width: props.cardWidth?.(),
        margin: card ? undefined : [0, layout.scrollGutter, layout.rowGap, 0],
        minWidth: 0,
        padding: [0, layout.rowInset, 0, layout.rowInset],
        align: "center", gap: layout.rowInset, shrink: 0 })}
      onPress={props.onSelect}
      paint={() => ({
        ...selectableRowPaint(props.selected()),
        // A recent-session card keeps its frame; the selection only brightens it.
        ...(card && !props.selected() ? { edge: palette().border } : {}),
        shadows: card ? [shadows.panel] : undefined,
        transitions: [smoothTransition("edge", motion.fast)],
      })}
    >
      <Box style={{ width: spacing.control, height: spacing.control, align: "center", justify: "center", shrink: 0 }}>
        {props.result().image ? (
          <Image source={() => props.result().image ?? ""} style={{ width: layout.icon, height: layout.icon }} />
        ) : (
          <Glyph name={() => props.result().icon} size={layout.icon} color={() => palette().textSecondary} />
        )}
      </Box>
      <Column style={{ grow: 1, minWidth: 0, gap: spacing.unit / 2 }}>
        <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.body }}
          paint={() => ({ color: palette().textPrimary })}>{() => props.result().label}</Text>
        <Show when={() => !!props.result().detail}>
          {() => <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
            paint={() => ({ color: palette().textMuted })}>{() => props.result().detail ?? ""}</Text>}
        </Show>
      </Column>
      <Show when={() => !card && (props.selected() || !!props.result().target)} style={{ shrink: 0 }}>
        {() => <Dynamic value={props.selected}>
          {(selected) => selected ? <KbdKey label="↵" /> :
            <Glyph name="arrow-right" size={layout.icon} color={() => palette().textMuted} />}
        </Dynamic>}
      </Show>
    </Row>
  );
}

function PaletteHint(props: { keys: string[]; label: string; onPress?: () => void }): Element {
  return (
    <Row style={{ align: "center", gap: spacing.unit }} onPress={props.onPress}>
      {props.keys.map((key) => <KbdKey label={key} />)}
      <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
        paint={() => ({ color: palette().textSecondary })}>{props.label}</Text>
    </Row>
  );
}

function launcherEdge(sides: [number, number, number, number]) {
  return () => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: sides });
}

// The floating search bar every modal opens with, at the same height on screen.
function LauncherSearchBar(props: { children: Element | Element[] }): Element {
  const layout = launcherLayout;
  return (
    <Row style={{ height: floatingLayout.searchHeight, padding: [0, layout.gutter, 0, layout.gutter],
      align: "center", gap: layout.rowInset, shrink: 0 }}
      onPress={() => {}}
      paint={() => ({ ...floatingSurface(true), ...accordeeMenuMotion("top") })}>
      {props.children}
    </Row>
  );
}

// Where the floating chrome starts: a fixed height, so the bar never moves while results change.
function floatingTop(): number {
  return Math.max(floatingLayout.minTop, viewportSize.value[1] * floatingLayout.topFraction);
}

// Room left under the search bar, above the hints at the bottom of the window.
function floatingRoom(): number {
  return Math.max(0, viewportSize.value[1] - floatingTop() - floatingLayout.searchHeight - floatingLayout.gap * 2
    - floatingLayout.hintsHeight - floatingLayout.hintsBottom);
}

// Shortcut hints at the bottom of the window, outside every panel.
function PaletteHints(props: { children: Element | Element[] }): Element {
  return (
    <Row style={() => ({ position: "fixed", left: 0, width: viewportSize.value[0], height: floatingLayout.hintsHeight,
      top: viewportSize.value[1] - floatingLayout.hintsBottom - floatingLayout.hintsHeight,
      align: "center", justify: "center", gap: launcherLayout.gutter, layer: launcherLayout.layer + 1 })}>
      {props.children}
    </Row>
  );
}

// The dimmed layer under the floating chrome; a click on it closes the modal.
function PaletteBackdrop(props: { children: Element | Element[] }): Element {
  return (
    <Column
      style={() => ({
        position: "fixed", left: 0, top: 0, width: viewportSize.value[0], height: viewportSize.value[1],
        align: "center", padding: [floatingTop(), 0, 0, 0], layer: launcherLayout.layer,
      })}
      onPress={closePalette}
      paint={() => ({
        background: withAlpha(palette().backdrop,
          windowTransparency() > 0 ? glassChrome.scrimAlpha : floatingLayout.backdropDim),
        backdropBlur: floatingLayout.backdropBlur, backdropSaturation: floatingLayout.saturation,
        // The blur grows in and out with the dim, as in the reference video.
        starting: { opacity: 0, backdropBlur: 0 }, exiting: { opacity: 0, backdropBlur: 0 },
        transitions: [smoothTransition("opacity", motion.fast), smoothTransition("backdropBlur", motion.base)] })}
    >
      {props.children}
    </Column>
  );
}

function LauncherHeading(props: { label: Source<string> }): Element {
  const layout = launcherLayout;
  return (
    <Row style={{ height: layout.headingHeight, align: "center", gap: layout.rowInset,
      padding: [0, layout.rowInset, 0, layout.scrollGutter], shrink: 0 }}>
      <Text font={{ family: fontFamilies.sans, size: typeScale.label }}
        paint={() => ({ color: palette().textMuted })}>{props.label}</Text>
      <Box style={{ grow: 1, height: spacing.hairline }} paint={() => ({ background: palette().border })} />
    </Row>
  );
}

function LauncherFooter(props: { children: Element | Element[] }): Element {
  const layout = launcherLayout;
  return (
    <Row style={{ height: layout.footerHeight, padding: [0, layout.gutter, 0, layout.gutter],
      align: "center", gap: layout.rowInset, shrink: 0 }} paint={launcherEdge([1, 0, 0, 0])}>
      {props.children}
    </Row>
  );
}

function launcherWidth(): number {
  return Math.min(launcherLayout.width, Math.max(0, viewportSize.value[0] - launcherLayout.margin * 2));
}

function SearchPaletteBody(props: { request: PaletteRequest }): Element {
  const layout = launcherLayout;
  const { mode, cwd, target } = props.request;
  const folder = signal<LauncherTarget | undefined>(target);
  let folderKeys: ((action: string) => void) | undefined;
  const query = signal("");
  const cursor = signal<string | null>(null);
  const rows = signal<PaletteRow[]>([]);
  const choices = signal<PaletteResult[]>([]);
  const gridWidth = () => Math.max(0, launcherWidth() - layout.rowInset - layout.scrollGutter - spacing.hairline * 2);
  effect(() => {
    if (folder.value) return;
    const q = query.value.trim();
    const groups: PaletteGroup[] = mode === "sessions" ? [{ label: "Sessions", results: paletteSessions(q) }]
      : mode === "new-session" ? [{ label: "New session", results: launcherAgents(q) }] : launcherGroups(q, cwd);
    rows.value = groups.filter((group) => group.results.length > 0).flatMap(({ label, results, cards }): PaletteRow[] => {
      const heading = { key: `heading:${label}`, heading: label };
      if (!cards) return [heading, ...results.map((result) => ({ key: result.key, result }))];
      const columns = Math.max(1, Math.min(layout.recentColumns,
        Math.floor((gridWidth() + layout.recentGap) / (layout.recentCardMin + layout.recentGap))));
      const grid: PaletteRow[] = [heading];
      for (let i = 0; i < results.length; i += columns) {
        grid.push({ key: `recent-row:${i}:${columns}`, cards: results.slice(i, i + columns), columns });
      }
      return grid;
    });
    choices.value = groups.flatMap((group) => group.results);
  });
  const selection = () => choices.value.find((choice) => choice.key === cursor.value) ?? choices.value[0];
  const pick = (result = selection()) => {
    if (!result) return;
    if (result.target) {
      cursor.value = result.key;
      folder.value = result.target;
      return;
    }
    closePalette();
    if (overviewOpen2.value) closeOverviewMode();
    result.onSelect();
  };
  const move = (step: -1 | 1) => {
    const items = choices.value;
    const index = items.findIndex((item) => item.key === selection()?.key);
    const next = items[index + step];
    if (next) cursor.value = next.key;
  };
  const back = () => {
    if (mode === "folder") closePalette();
    else folder.value = undefined;
  };
  const handleKey = (action: string) => {
    if (action === "escape") (folder.value ? back() : closePalette());
    else if (folder.value) folderKeys?.(action);
    else if (action === "up") move(-1);
    else if (action === "down") move(1);
    else if (action === "enter") pick();
  };
  paletteKeyHandler = handleKey;
  onCleanup(() => {
    if (paletteKeyHandler === handleKey) paletteKeyHandler = undefined;
  });
  const placeholder = mode === "sessions" ? "Search sessions"
    : mode === "new-session" ? "Search agents or terminal" : "Search or enter a URL";
  const pickedRow = signal<string | null>(null);
  effect(() => {
    const key = cursor.value;
    pickedRow.value = key === null ? null : rows.value.find((row) => row.key === key ||
      ("cards" in row && row.cards.some((card) => card.key === key)))?.key ?? null;
  });
  const estimate = (row: PaletteRow) => "heading" in row ? layout.headingHeight + layout.sectionGap
    : "cards" in row ? layout.recentHeight + layout.recentGap : layout.rowHeight + layout.rowGap;
  // The panel fits its results (up to the room on screen): no empty card under a short list.
  const panelHeight = () => rows.value.length === 0 ? floatingLayout.emptyHeight
    : Math.min(floatingRoom(), layout.height,
      rows.value.reduce((sum, row) => sum + estimate(row), layout.rowInset * 2));
  const search = () => (
    <Column style={{ gap: floatingLayout.gap }}>
      <LauncherSearchBar>
        <Glyph name="search" size={layout.icon} color={() => palette().textMuted} />
        <TextInput autoFocus retainOnSubmit value={query.value}
          style={{ grow: 1, minWidth: 0, height: spacing.controlLg }}
          font={{ family: fontFamilies.sans, size: typeScale.title }}
          placeholder={placeholder} placeholderColor={palette().textMuted}
          onChange={(text) => {
            query.value = text;
            cursor.value = null;
            if (mode === "launcher") searchWorkspaceFiles(text, cwd);
          }}
          onSubmit={() => pick()}
          paint={() => ({ color: palette().textPrimary })}
        />
      </LauncherSearchBar>
      {/* No right padding: the rows' gutter holds the scrollbar, so it sits against the panel edge. */}
      <Box style={() => ({ height: panelHeight(), minHeight: 0, padding: [0, 0, 0, layout.rowInset],
        clip: true })}
        onPress={() => {}}
        paint={() => ({ ...floatingSurface(true), ...accordeeMenuMotion("top"),
          transitions: [...accordeeMenuMotion("top").transitions, smoothTransition("height", motion.fast)] })}>
        <Dynamic value={() => rows.value.length > 0} style={{ grow: 1, minHeight: 0 }}>
          {(hasResults) => hasResults ? (
            <LazyRows items={() => rows.value} key={(row) => row.key} padding={[layout.rowInset, layout.rowInset]}
              estimate={estimate}
              resetScrollOn={() => query.value.trim()} reveal={() => pickedRow.value}
              revealAlign={0.5} revealSmooth>
              {(row) => {
                const entry = row();
                if ("heading" in entry) return (
                  <Column style={{ padding: [layout.sectionGap, 0, 0, 0], shrink: 0 }}>
                    <LauncherHeading label={entry.heading} />
                  </Column>
                );
                if ("cards" in entry) return (
                  <For each={() => (row() as { cards: PaletteResult[] }).cards} key={(card) => card.key}
                    style={{ direction: "row", gap: layout.recentGap, margin: [0, 0, layout.recentGap, 0], shrink: 0 }}>
                    {(card) => <PaletteResultRow result={card} selected={() => selection()?.key === card().key}
                      cardWidth={() => Math.max(0, (gridWidth() - layout.recentGap * (entry.columns - 1)) / entry.columns)}
                      onSelect={() => pick(card())} />}
                  </For>
                );
                const result = () => (row() as { result: PaletteResult }).result;
                return <PaletteResultRow result={result} selected={() => selection()?.key === result().key}
                  onSelect={() => pick(result())} />;
              }}
            </LazyRows>
          ) : (
            <Box style={{ grow: 1, align: "center", justify: "center" }}>
              <EmptyState compact title={() => mode === "sessions" ? query.value.trim() ? "No matching sessions" : "No sessions yet" : "No results"} />
            </Box>
          )}
        </Dynamic>
      </Box>
      <PaletteHints>
        <PaletteHint keys={["↑", "↓"]} label="Navigate" />
        <PaletteHint keys={["↵"]} label="Open" />
        <PaletteHint keys={["Esc"]} label="Close" onPress={closePalette} />
      </PaletteHints>
    </Column>
  );
  return (
    <PaletteBackdrop>
      {/* The folder browser needs a fixed height; search results size the panel to themselves. */}
      <Dynamic value={() => folder.value}
        style={() => ({ width: launcherWidth(), minHeight: 0,
          height: folder.value ? Math.min(layout.height, floatingRoom() + floatingLayout.searchHeight
            + floatingLayout.gap * 2 + floatingLayout.hintsHeight) : undefined })}>
        {(selected) => selected ? <LauncherFolders cwd={cwd} target={selected} back={back} canGoBack={mode !== "folder"}
          onKeys={(handler) => { folderKeys = handler; onCleanup(() => { folderKeys = undefined; }); }} /> : search()}
      </Dynamic>
    </PaletteBackdrop>
  );
}

interface WorkspaceRow {
  index: number;
  label: string;
  // Its windows, by name.
  detail: string;
}

function workspaceRows(query: string): WorkspaceRow[] {
  return workspaceList().map((workspace, index): WorkspaceRow => ({
    index,
    label: workspaceLabel(index),
    detail: workspace.panes.map(windowLabel).filter((name) => name.length > 0).join(" · "),
  })).filter((row) => paletteMatches(query, row.label, row.detail));
}

// The workspace switcher, as clean as the overview: the floating search bar and a wheel of one-line
// rows whose selected row stays centred (outlined) while the list slides under it.
function WorkspaceSwitcherBody(): Element {
  const layout = workspaceSwitcherLayout;
  const query = signal("");
  const cursor = signal(tilingState.value.activeWorkspace);
  const rows = () => workspaceRows(query.value.trim());
  const position = () => Math.max(0, rows().findIndex((row) => row.index === cursor.value));
  const width = () => Math.min(layout.width, Math.max(0, viewportSize.value[0] - launcherLayout.margin * 2));
  const pitch = layout.rowHeight + layout.rowGap;
  // The wheel's centre: the middle of the room, but never more than `visibleRows` rows below the bar.
  const centre = () => Math.min(floatingRoom() / 2, layout.visibleRows * pitch + layout.rowHeight / 2);
  const pick = (row = rows()[position()]) => {
    if (!row) return;
    closePalette();
    focusWorkspace(row.index);
  };
  const move = (step: -1 | 1) => {
    const next = rows()[position() + step];
    if (next) cursor.value = next.index;
  };
  const handleKey = (action: string) => {
    if (action === "escape") closePalette();
    else if (action === "up") move(-1);
    else if (action === "down") move(1);
    else if (action === "enter") pick();
  };
  paletteKeyHandler = handleKey;
  onCleanup(() => {
    if (paletteKeyHandler === handleKey) paletteKeyHandler = undefined;
  });
  return (
    <PaletteBackdrop>
      <Column style={() => ({ width: width(), gap: floatingLayout.gap })}>
        <LauncherSearchBar>
          <Glyph name="search" size={floatingLayout.icon} color={() => palette().textMuted} />
          <TextInput autoFocus retainOnSubmit value=""
            style={{ grow: 1, minWidth: 0, height: spacing.controlLg }}
            font={{ family: fontFamilies.sans, size: typeScale.title }}
            placeholder="Switch workspace" placeholderColor={palette().textMuted}
            onChange={(text) => { query.value = text; }}
            onSubmit={() => pick()}
            paint={() => ({ color: palette().textPrimary })}
          />
        </LauncherSearchBar>

        {/* Rows are placed from the wheel's centre, so a move only changes their `y` (one spring). */}
        <Box style={() => ({ width: width(), height: floatingRoom(), shrink: 0 })}>
          <For each={rows} key={(row) => row.index} style={{ position: "absolute", left: 0, top: 0 }}>
            {(row) => {
              const at = () => rows().findIndex((entry) => entry.index === row().index);
              const distance = () => Math.abs(at() - position());
              return (
                <Row
                  style={() => ({ position: "absolute", left: 0, top: 0, width: width(), height: layout.rowHeight,
                    padding: [0, launcherLayout.gutter, 0, launcherLayout.gutter], align: "center",
                    gap: launcherLayout.rowInset * 2 })}
                  onPress={() => pick(row())}
                  paint={() => ({
                    ...selectableRowPaint(distance() === 0),
                    y: centre() - layout.rowHeight / 2 + (at() - position()) * pitch,
                    opacity: distance() > layout.visibleRows ? 0 : Math.max(0, 1 - distance() * layout.opacityStep),
                    scale: Math.max(layout.minScale, 1 - distance() * layout.scaleStep),
                    transitions: [springTransition("y"), springTransition("scale"),
                      smoothTransition("opacity", motion.fast), smoothTransition("edge", motion.fast)],
                  })}
                >
                  <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.title, weight: fontWeights.medium }}
                    paint={() => ({ color: palette().textPrimary })}>{() => row().label}</Text>
                  <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.caption }}
                    paint={() => ({ color: palette().textMuted })}>{() => row().detail}</Text>
                  <Text font={{ family: fontFamilies.mono, size: typeScale.caption }}
                    paint={() => ({ color: palette().textMuted })}>{() => String(row().index + 1)}</Text>
                </Row>
              );
            }}
          </For>
          <Show when={() => rows().length === 0} style={{ grow: 1, align: "center", justify: "center" }}>
            {() => <EmptyState compact centered title="No matching workspaces" />}
          </Show>
        </Box>
      </Column>

      <PaletteHints>
        <PaletteHint keys={["↑", "↓"]} label="Navigate" />
        <PaletteHint keys={["↵"]} label="Open" />
        <PaletteHint keys={["Esc"]} label="Close" onPress={closePalette} />
      </PaletteHints>
    </PaletteBackdrop>
  );
}

function SearchPalette(): Element {
  return (
    <Dynamic value={() => paletteRequest.value} style={{ position: "absolute", left: 0, top: 0 }}>
      {(request) => !request ? <Box />
        : request.mode === "workspaces" ? <WorkspaceSwitcherBody /> : <SearchPaletteBody request={request} />}
    </Dynamic>
  );
}
