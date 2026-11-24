// The launcher and session search share one keyboard-first Accordee dialog.
type PaletteMode = "launcher" | "sessions";
interface PaletteRequest {
  mode: PaletteMode;
  cwd: string;
}

interface PaletteResult {
  key: string;
  icon: string;
  image?: string;
  label: string;
  detail?: string;
  onSelect: () => void;
}

interface PaletteGroup {
  label: string;
  results: PaletteResult[];
}

type PaletteRow = { key: string; heading: string } | { key: string; result: PaletteResult };

const paletteRequest = signal<PaletteRequest | null>(null);
let paletteKeyHandler: ((action: string) => void) | undefined;

function togglePalette(mode: PaletteMode): void {
  if (activeNav.value !== "agents" || projectDialog.value || imageViewer.value ||
      releaseNotesOpen.value || restartConfirmOpen.value) return;
  if (paletteRequest.value?.mode === mode) {
    closePalette();
    return;
  }
  paletteRequest.value = { mode, cwd: workspacePath.value };
  requestHost({ type: "paletteKeys", on: true });
}

function closePalette(): void {
  paletteRequest.value = null;
  requestHost({ type: "paletteKeys", on: false });
}

function paletteMatches(query: string, ...values: string[]): boolean {
  const haystack = values.join(" ").toLowerCase();
  return query.toLowerCase().split(/\s+/).every((part) => haystack.includes(part));
}

function paletteSessions(query: string): PaletteResult[] {
  return sessionsState.value.items
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

function launcherGroups(q: string, cwd: string): PaletteGroup[] {
  const localUrl = screenState.value.rows.flatMap((row) => row.links)
    .find((link) => link.kind === "url" && isLocalUrl(link.target));
  const session = activeSession();
  const actions: PaletteResult[] = [
    { key: "terminal", icon: "square-terminal", label: "Terminal", onSelect: () => showProjectDialog("session", cwd) },
    { key: "files", icon: "folder", label: "Open File", onSelect: () => showTile("files", cwd) },
    { key: "review", icon: "git-branch", label: "Git", onSelect: () => showTile("review", cwd) },
    { key: "browser", icon: "globe", label: "Browser", onSelect: () => openBrowser(undefined, cwd) },
    { key: "metrics", icon: "dashboard", label: "Metrics", onSelect: openMetricsTab },
  ];
  if (localUrl) actions.push({ key: "local-url", icon: "globe", label: "Open local URL", detail: localUrl.target,
    onSelect: () => openBrowser(localUrl.target, cwd) });
  if (session && session.cli !== "terminal") actions.push({
    key: "side-question", icon: "message-square-plus", label: "Side Question",
    onSelect: () => showProjectDialog("session", cwd, session.cli),
  });
  if (q) {
    const view = tilingState.value;
    const column = view.columns.find((column) => column.panes.includes(view.focusedSession));
    if (view.mode === "niri" && !view.maximized && column) actions.push({
      key: "column-width", icon: "columns", label: column.expanded ? "Restore column width" : "Expand column",
      onSelect: () => tilingAction("maximizeColumn"),
    });
  }

  const plugins = configState.value.clis.filter((cli) => cli.enabled && paletteMatches(q, cli.name, cli.id, "plugins"))
    .map((cli) => ({
      key: `plugin:${cli.id}`, icon: "puzzle", image: "cli:" + cli.id, label: cli.name,
      onSelect: () => showProjectDialog("session", cwd, cli.id),
    }));
  const artifacts = detectedFilesList().filter((link) => basename(link.target) && paletteMatches(q, link.target))
    .map((link) => ({
      key: `artifact:${link.target}`, icon: "file-code", label: basename(link.target), detail: link.target,
      onSelect: () => openLink(link, session?.id),
    }));
  const groups: PaletteGroup[] = [];
  const address = q ? resolveAddress(q, false) : "";
  if (address) groups.push({ label: "Browser", results: [{ key: "url", icon: "globe", label: address, onSelect: () => openBrowser(address, cwd) }] });
  groups.push({ label: "Actions", results: actions.filter((action) => paletteMatches(q, action.label, action.detail ?? "")) });
  if (q) groups.push({ label: "Sessions", results: paletteSessions(q) });
  if (q && fileSearch.value.cwd === cwd && fileSearch.value.query === q) {
    groups.push({ label: "Files", results: fileSearch.value.items.map((path) => ({
      key: `file:${path}`, icon: "file", label: basename(path), detail: path,
      onSelect: () => openFile(path, undefined, cwd),
    })) });
  }
  // Agents open the two-step launcher (folder, then agent) with this one preselected.
  groups.push({ label: "Start an agent", results: plugins }, { label: "Files in output", results: artifacts });
  if (!q) groups.push({ label: "Running sessions", results: paletteSessions("") });
  return groups;
}

function PaletteResultRow(props: { result: () => PaletteResult; selected: () => boolean; onSelect: () => void }): Element {
  const layout = searchDialogLayout;
  return (
    <Row
      style={{ height: layout.rowHeight, margin: [0, layout.scrollGutter, layout.rowGap, 0],
        padding: [0, layout.rowInset, 0, layout.rowInset],
        align: "center", gap: layout.rowInset, shrink: 0 }}
      onPress={props.onSelect}
      paint={() => ({
        radius: radii.lg, background: props.selected() ? palette().sidebarActive : 0,
        hover: { background: props.selected() ? palette().sidebarActive : rowHoverBackground() },
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Box style={{ width: spacing.controlLg, height: spacing.controlLg, align: "center", justify: "center", shrink: 0 }}
        paint={() => ({ radius: radii.md, background: palette().panelBg, edge: palette().border, edgeWidth: spacing.hairline })}>
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
      <Show when={props.selected} style={{ shrink: 0 }}>
        {() => <KbdKey label="↵" />}
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

function SearchPaletteBody(props: { request: PaletteRequest }): Element {
  const layout = searchDialogLayout;
  const { mode, cwd } = props.request;
  const query = signal("");
  const cursor = signal<string | null>(null);
  const rows = signal<PaletteRow[]>([]);
  const choices = signal<PaletteResult[]>([]);
  effect(() => {
    const q = query.value.trim();
    const groups = mode === "sessions" ? [{ label: "Sessions", results: paletteSessions(q) }] : launcherGroups(q, cwd);
    rows.value = groups.filter((group) => group.results.length > 0).flatMap(({ label, results }) => [
      { key: `heading:${label}`, heading: label },
      ...results.map((result) => ({ key: result.key, result })),
    ]);
    choices.value = groups.flatMap((group) => group.results);
  });
  const selection = () => choices.value.find((choice) => choice.key === cursor.value) ?? choices.value[0];
  const pick = (result = selection()) => {
    if (!result) return;
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
  const handleKey = (action: string) => {
    if (action === "up") move(-1);
    else if (action === "down") move(1);
    else if (action === "enter") pick();
  };
  paletteKeyHandler = handleKey;
  onCleanup(() => {
    if (paletteKeyHandler === handleKey) paletteKeyHandler = undefined;
  });
  const placeholder = mode === "sessions" ? "Search sessions" : "Search or enter a URL";
  const edge = (sides: [number, number, number, number]) => ({
    edge: palette().border, edgeWidth: spacing.hairline, edgeSides: sides,
  });
  return (
    <Box
      style={() => ({
        position: "fixed", left: 0, top: 0, width: viewportSize.value[0], height: viewportSize.value[1],
        align: "center", justify: "center", layer: layout.layer,
      })}
      onPress={closePalette}
      paint={() => ({ background: palette().backdrop })}
    >
      <Column
        style={() => ({
          width: Math.min(layout.width, Math.max(0, viewportSize.value[0] - layout.margin * 2)),
          height: Math.min(layout.height, Math.max(0, viewportSize.value[1] - layout.margin * 2)),
          minHeight: 0,
        })}
        onPress={() => {}}
        paint={() => ({ radius: radii.xxl, background: palette().cardBg, edge: palette().border,
          edgeWidth: spacing.hairline, shadows: [shadows.searchModal], ...accordeeDialogMotion() })}
      >
        <Row style={{ height: layout.headerHeight, padding: [0, layout.gutter, 0, layout.gutter],
          align: "center", gap: layout.rowInset, shrink: 0 }}
          paint={() => edge([0, 0, 1, 0])}>
          <Glyph name="search" size={layout.icon} color={() => palette().textMuted} />
          <TextInput autoFocus retainOnSubmit
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
        </Row>
        <Box style={{ grow: 1, minHeight: 0, padding: [0, layout.rowInset, 0, layout.rowInset] }}>
          <Dynamic value={() => rows.value.length > 0} style={{ grow: 1, minHeight: 0 }}>
            {(hasResults) => hasResults ? (
              <LazyRows items={() => rows.value} key={(row) => row.key} padding={[layout.rowInset, layout.rowInset]}
                estimate={(row) => "heading" in row ? layout.headingHeight : layout.rowHeight + layout.rowGap}
                resetScrollOn={() => query.value.trim()} reveal={() => cursor.value}>
                {(row) => "heading" in row() ? (
                  <Row style={{ height: layout.headingHeight, align: "center",
                    padding: [0, layout.rowInset, 0, layout.rowInset], shrink: 0 }}>
                    <Text font={{ family: fontFamilies.sans, size: typeScale.label }}
                      paint={() => ({ color: palette().textMuted })}>{() => (row() as { heading: string }).heading}</Text>
                  </Row>
                ) : (
                  <PaletteResultRow result={() => (row() as { result: PaletteResult }).result}
                    selected={() => selection()?.key === row().key}
                    onSelect={() => pick((row() as { result: PaletteResult }).result)} />
                )}
              </LazyRows>
            ) : (
              <Box style={{ grow: 1, align: "center", justify: "center" }}>
                <EmptyState compact title={() => mode === "launcher" ? "No results" : query.value.trim() ? "No matching sessions" : "No sessions yet"} />
              </Box>
            )}
          </Dynamic>
        </Box>
        <Row style={{ height: layout.footerHeight, padding: [0, layout.gutter, 0, layout.gutter],
          align: "center", gap: layout.gutter, shrink: 0 }} paint={() => edge([1, 0, 0, 0])}>
          <PaletteHint keys={["↑", "↓"]} label="Navigate" />
          <PaletteHint keys={["↵"]} label="Open" />
          <Box style={{ grow: 1 }} />
          <PaletteHint keys={["Esc"]} label="Close" onPress={closePalette} />
        </Row>
      </Column>
    </Box>
  );
}

function SearchPalette(): Element {
  return (
    <Dynamic value={() => paletteRequest.value} style={{ position: "absolute", left: 0, top: 0 }}>
      {(request) => request ? <SearchPaletteBody request={request} /> : <Box />}
    </Dynamic>
  );
}
