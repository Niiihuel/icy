// Right auxiliary panel modelled on Antigravity's Overview pane: an inset card with an Overview
// list (Review +adds -dels, Files, Terminal, sections with count badges), a file tree with
// indent guides and VCS dots, a Review list of changed files, and a code / diff viewer.

function codeSpans(line: { text: string; highlights?: CodeHighlight[] }): Span[] {
  const colors = palette().syntax;
  if (line.text.length > editorLayout.maxDrawColumns) return [{ text: visibleCode(line.text), color: colors.plain }];
  if (!line.highlights?.length) return [{ text: line.text || " ", color: colors.plain }];
  const spans: Span[] = [];
  let cursor = 0;
  for (const [start, end, kind] of line.highlights) {
    if (cursor < start) spans.push({ text: line.text.slice(cursor, start), color: colors.plain });
    spans.push({ text: line.text.slice(start, end), color: colors[kind] });
    cursor = end;
  }
  if (cursor < line.text.length) spans.push({ text: line.text.slice(cursor), color: colors.plain });
  return spans;
}

function vcsColor(status: string): number {
  if (status === "U") return palette().vcsUntracked;
  if (status === "A") return palette().vcsAdded;
  if (status === "D") return palette().vcsDeleted;
  return palette().vcsModified;
}

function changeFor(path: string, cwd?: string): FileChangeView | undefined {
  return changeIndex(projectChanges(cwd)).byPath.get(path);
}

// `+12 -3` in Antigravity's green-500 / red-500, tabular.
function DiffStat(props: { additions: () => number; deletions: () => number }): Element {
  return (
    <Show when={() => props.additions() + props.deletions() > 0}>
      {() => (
        <Row style={{ gap: spacing.unit, shrink: 0 }}>
          <Text
            lines={1}
            font={{ family: fontFamilies.mono, size: typeScale.caption }}
            paint={() => ({ color: palette().diffAddText })}
          >
            {() => `+${props.additions()}`}
          </Text>
          <Text
            lines={1}
            font={{ family: fontFamilies.mono, size: typeScale.caption }}
            paint={() => ({ color: palette().diffDelText })}
          >
            {() => `-${props.deletions()}`}
          </Text>
        </Row>
      )}
    </Show>
  );
}

function VcsDot(props: { status: () => string | undefined }): Element {
  return (
    <Show when={() => props.status() !== undefined} style={{ shrink: 0 }}>
      {() => (
        <Box
          style={{ width: reviewLayout.statusDot, height: reviewLayout.statusDot, shrink: 0 }}
          paint={() => ({ radius: radii.sm, background: vcsColor(props.status() ?? "M") })}
        />
      )}
    </Show>
  );
}

// `PanelItemRow`: rounded-lg row, 16px icon at 70% opacity, secondary text, hover bg-secondary.
function PanelRow(props: {
  icon?: string;
  image?: Source<string>;
  leading?: Element;
  label: Source<string>;
  detail?: Element;
  trailing?: Element;
  mono?: boolean;
  selected?: Source<boolean>;
  onPress?: () => void;
}): Element {
  const selected = () => read(props.selected ?? false);
  return (
    <Row
      style={{ minHeight: 32, padding: [6, 8, 6, 8], align: "center", gap: 6, shrink: 0 }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.lg,
        background: selected() ? palette().sidebarActive : 0,
        hover: props.onPress ? { background: palette().sidebarActive } : undefined,
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      {props.leading ??
        (props.image ? (
          <Image source={props.image} style={{ width: 16, height: 16, shrink: 0 }} />
        ) : props.icon ? (
          <Box style={{ shrink: 0 }} paint={{ opacity: 0.7 }}>
            <Glyph name={props.icon} size={16} color={() => palette().textSecondary} />
          </Box>
        ) : null)}
      <Text
        lines={1}
        style={{ grow: props.detail ? 0 : 1, shrink: 1, minWidth: 0 }}
        font={{
          family: props.mono ? fontFamilies.mono : fontFamilies.sans,
          size: props.mono ? typeScale.caption : typeScale.label,
        }}
        paint={() => ({ color: selected() ? palette().textPrimary : palette().textSecondary })}
      >
        {props.label}
      </Text>
      {props.detail ?? null}
      {props.detail ? <Box style={{ grow: 1 }} /> : null}
      {props.trailing ?? null}
    </Row>
  );
}

// Overview section header (`k5`): muted label + pill count badge + rotating chevron.
function PanelSection(props: {
  title: string;
  count: () => number;
  children: () => Element;
}): Element {
  const open = signal(true);
  return (
    <Column style={{ padding: [4, 4, 4, 4], gap: 2, shrink: 0 }}>
      <Row
        style={{ height: 32, padding: [0, 8, 0, 8], align: "center", gap: 6 }}
        onPress={() => {
          open.value = !open.value;
        }}
        paint={() => ({ radius: radii.lg, hover: { background: palette().sidebarActive } })}
      >
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          paint={() => ({ color: palette().textMuted })}
        >
          {props.title}
        </Text>
        <Box
          style={{ height: 16, padding: [0, 6, 0, 6], align: "center", justify: "center" }}
          paint={() => ({ radius: 8, background: palette().kbdBg })}
        >
          <Text
            font={{ family: fontFamilies.sans, size: typeScale.micro }}
            paint={() => ({ color: palette().textMuted })}
          >
            {() => String(props.count())}
          </Text>
        </Box>
        <Box style={{ grow: 1 }} />
        <Glyph
          name={() => (open.value ? "chevron-down" : "chevron-right")}
          size={13}
          color={() => palette().textMuted}
        />
      </Row>
      <Show when={() => open.value}>{props.children}</Show>
    </Column>
  );
}

function PanelDivider(): Element {
  return <Box style={{ height: 1, shrink: 0 }} paint={() => ({ background: palette().border })} />;
}

function OverviewView(props: { cwd: string }): Element {
  const changes = () => projectChanges(props.cwd);
  return (
    <Column style={{ grow: 1, minHeight: 0 }}>
      <Column style={{ padding: [4, 4, 4, 4], gap: 1, shrink: 0 }}>
        <PanelRow
          icon="file-diff"
          label="Review"
          detail={<DiffStat additions={() => changes().additions} deletions={() => changes().deletions} />}
          onPress={() => showTile("review", props.cwd)}
        />
        <PanelRow icon="file" label="Files" onPress={() => showTile("files", props.cwd)} />
        <PanelRow
          icon="square-terminal"
          label="Terminal"
          onPress={() => showProjectDialog("session", props.cwd)}
        />
      </Column>
      <PanelDivider />
      <Column style={{ grow: 1, minHeight: 0, scroll: true }}>
        <PanelSection title="Sessions" count={() => sessionsState.value.items.length}>
          {() => (
            <For each={() => sessionsState.value.items} key={(s) => s.id} style={{ gap: 1 }}>
              {(item) => (
                <PanelRow
                  image={() => "cli:" + item().cli}
                  label={() => cleanTitle(item().title)}
                  selected={() => activeNav.value === "agents" && tilingState.value.focusedSession === item().id}
                  trailing={<StatusDot alive={() => item().alive} />}
                  onPress={() => {
                    activeNav.value = "agents";
                    selectSession(item().id);
                  }}
                />
              )}
            </For>
          )}
        </PanelSection>
        <PanelDivider />
        <PanelSection title="Modified files" count={() => changes().files.length}>
          {() => (
            <For each={() => changes().files.slice(0, 8)} key={(f) => f.path} style={{ gap: 1 }}>
              {(f) => (
                <PanelRow
                  icon="file"
                  label={() => basename(f().path)}
                  detail={<DiffStat additions={() => f().additions} deletions={() => f().deletions} />}
                  trailing={<VcsDot status={() => f().status} />}
                  onPress={() => openDiff(f().path, props.cwd)}
                />
              )}
            </For>
          )}
        </PanelSection>
        <PanelDivider />
        <PanelSection title="Detected in terminal" count={detectedFilesCount}>
          {() => (
            <For each={detectedFilesList} key={(l) => l.target} style={{ gap: 1 }}>
              {(link) => (
                <PanelRow
                  icon="file-code"
                  mono={true}
                  label={() => (link().line ? `${link().target}:${link().line}` : link().target)}
                  onPress={() => openLink(link())}
                />
              )}
            </For>
          )}
        </PanelSection>
      </Column>
    </Column>
  );
}

function flattenTree(tree: ProjectTree): FileEntryView[] {
  const out: FileEntryView[] = [];
  const expanded = tree.expanded.value;
  const walk = (dir: string) => {
    for (const entry of treeDirectory(tree, dir).value ?? []) {
      out.push(entry);
      if (entry.isDir && expanded.has(entry.path)) walk(entry.path);
    }
  };
  walk("");
  return out;
}

// `FileTreeNode`: indent guides (10px, right border shown on hover), 16px icon, truncating name,
// trailing VCS dot; selected row uses bg-secondary. Includes Accordee-styled inline file search.
function FilesTreeView(props: { cwd: string }): Element {
  const tree = projectTree(props.cwd);
  const layout = treeLayout;
  const hovering = signal(false);
  const query = signal("");
  let searchField: Field | undefined;
  if (treeDirectory(tree, "").value === undefined) listFiles("", props.cwd);

  const trimmed = () => query.value.trim();
  const matches = () => {
    const q = trimmed();
    if (!q) return [];
    const res = fileSearch.value;
    return res.cwd === props.cwd && res.query === q ? res.items : [];
  };
  const clearSearch = () => {
    query.value = "";
    searchField?.replace("");
    searchWorkspaceFiles("", props.cwd);
  };
  const openFirstMatch = () => {
    const first = matches()[0];
    if (first) openFile(first, undefined, props.cwd);
  };

  return (
    <Column style={{ grow: 1, minHeight: 0 }}>
      {/* Accordee search wrapper: 28px height, hairline border, appBg surface, hoverBorder on hover. */}
      <Row
        style={{ padding: layout.searchPadding, shrink: 0 }}
        paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [0, 0, 1, 0] })}
      >
        <Row
          style={{
            height: layout.searchHeight,
            grow: 1,
            minWidth: 0,
            padding: [0, spacing.unit * 1.5, 0, spacing.unit * 2],
            align: "center",
            gap: layout.iconGap,
          }}
          paint={() => ({
            radius: radii.md,
            background: palette().appBg,
            edge: palette().border,
            edgeWidth: spacing.hairline,
            hover: { edge: hoverBorder() },
            transitions: [smoothTransition("edge", motion.fast)],
          })}
        >
          <Glyph name="search" size={layout.rowIcon} color={() => palette().textMuted} />
          <TextInput
            value={query.value}
            ref={(input) => { searchField = input; }}
            retainOnSubmit
            style={{ grow: 1, minWidth: 0, height: layout.searchHeight }}
            font={{ family: fontFamilies.sans, size: typeScale.caption }}
            placeholder="Search files…"
            placeholderColor={palette().textMuted}
            onChange={(text) => {
              query.value = text;
              searchWorkspaceFiles(text, props.cwd);
            }}
            onSubmit={openFirstMatch}
            paint={() => ({ color: palette().textPrimary })}
          />
          <Show when={() => query.value.length > 0} style={{ shrink: 0 }}>
            {() => (
              <Box
                style={{ width: 20, height: 20, align: "center", justify: "center" }}
                onPress={clearSearch}
                paint={() => ({
                  radius: radii.sm,
                  hover: { background: palette().sidebarActive },
                  transitions: [smoothTransition("background", motion.fast)],
                })}
              >
                <Glyph name="close" size={10} color={() => palette().textMuted} hover={() => palette().textPrimary} />
              </Box>
            )}
          </Show>
        </Row>
      </Row>

      <Dynamic value={() => trimmed().length > 0} style={{ grow: 1, minHeight: 0 }}>
        {(searching) =>
          searching ? (
            <Column style={{ grow: 1, minHeight: 0, padding: [0, layout.inset, 0, layout.inset] }}>
              <Show when={() => {
                const res = fileSearch.value;
                return res.cwd === props.cwd && res.query === trimmed() && res.items.length === 0;
              }}>
                {() => <EmptyState compact title="No matching files" style={{ padding: emptyStateLayout.padding }} />}
              </Show>
              <LazyRows
                items={matches}
                key={(path) => path}
                rowHeight={layout.searchRowHeight}
                padding={[layout.inset, layout.paddingBottom]}
              >
                {(path) => {
                  const dir = () => {
                    const parts = path().split("/");
                    return parts.length > 1 ? parts.slice(0, -1).join("/") : "";
                  };
                  const selected = () => tree.selected.value === path();
                  return (
                    <Row
                      style={{ height: layout.searchRowHeight, padding: layout.rowPadding, align: "center", gap: layout.iconGap, shrink: 0 }}
                      onPress={() => {
                        tree.selected.value = path();
                        openFile(path(), undefined, props.cwd);
                      }}
                      paint={() => ({
                        radius: radii.sm,
                        background: selected() ? rowHoverBackground() : 0,
                        hover: { background: rowHoverBackground() },
                        transitions: [smoothTransition("background", motion.fast)],
                      })}
                    >
                      <Glyph name="file" size={layout.rowIcon} color={() => selected() ? palette().textPrimary : palette().textMuted} />
                      <Column style={{ grow: 1, minWidth: 0, gap: 1 }}>
                        <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
                          paint={() => ({ color: selected() ? palette().textPrimary : palette().textSecondary })}>
                          {() => basename(path())}
                        </Text>
                        <Show when={() => dir().length > 0}>
                          {() => (
                            <Text lines={1} font={{ family: fontFamilies.mono, size: typeScale.micro }}
                              paint={() => ({ color: palette().textMuted })}>
                              {dir}
                            </Text>
                          )}
                        </Show>
                      </Column>
                      <Dynamic value={() => changeFor(path(), props.cwd)?.status ?? ""} style={{ shrink: 0 }}>
                        {(status) => status ? <VcsLetter status={() => status} /> : <Box />}
                      </Dynamic>
                    </Row>
                  );
                }}
              </LazyRows>
            </Column>
          ) : (
            <Column style={{ grow: 1, minHeight: 0, padding: [0, layout.inset, 0, layout.inset] }} onHover={(inside) => (hovering.value = inside)}>
              <Show when={() => treeDirectory(tree, "").value?.length === 0}>
                {() => <EmptyState compact title="No files to show" style={{ padding: emptyStateLayout.padding }} />}
              </Show>
              <LazyRows items={() => flattenTree(tree)} key={(entry) => entry.path}
                rowHeight={layout.rowHeight + layout.rowGap} padding={[layout.inset, layout.paddingBottom]}>
                {(row) => {
                  const depth = Math.min(layout.maxGuides, row().path.split("/").length - 1);
                  const isDir = () => row().isDir;
                  const selected = () => tree.selected.value === row().path;
                  const open = () => tree.expanded.value.has(row().path);
                  return (
                    <Row
                      style={{ height: layout.rowHeight, margin: [0, 0, layout.rowGap, 0], align: "stretch", shrink: 0 }}
                      onPress={() => {
                        tree.selected.value = row().path;
                        if (isDir()) toggleTreeDir(row().path, props.cwd);
                        else openFile(row().path, undefined, props.cwd);
                      }}
                      paint={() => ({
                        radius: radii.sm,
                        background: selected() ? rowHoverBackground() : 0,
                        hover: { background: rowHoverBackground() },
                        transitions: [smoothTransition("background", motion.fast)],
                      })}
                    >
                      <Row style={{ shrink: 0 }}>
                        {Array.from({ length: depth }, () => (
                          <Box
                            style={{ width: layout.indent, shrink: 0 }}
                            paint={() => ({
                              edge: palette().border,
                              edgeWidth: spacing.hairline,
                              edgeSides: [0, 1, 0, 0],
                              opacity: hovering.value ? 1 : 0,
                              transitions: [smoothTransition("opacity", motion.fast)],
                            })}
                          />
                        ))}
                      </Row>
                      <Row style={{ grow: 1, minWidth: 0, padding: layout.rowPadding, align: "center", gap: layout.iconGap }}>
                        <Glyph
                          name={() => (isDir() ? (open() ? "folder-open" : "folder") : "file")}
                          size={layout.rowIcon}
                          color={() => (selected() ? palette().textPrimary : palette().textMuted)}
                        />
                        <Text
                          lines={1}
                          style={{ grow: 1, minWidth: 0 }}
                          font={{ family: fontFamilies.sans, size: typeScale.caption }}
                          paint={() => ({ color: selected() ? palette().textPrimary : palette().textSecondary })}
                        >
                          {() => row().name}
                        </Text>
                        {/* Git-style status: a letter on changed files, a dot on folders holding changes. */}
                        <Dynamic value={() => isDir()
                          ? (changeIndex(projectChanges(props.cwd)).changedDirs.has(row().path) ? "dir" : "")
                          : changeFor(row().path, props.cwd)?.status ?? ""} style={{ shrink: 0 }}>
                          {(status) => status === "dir" ? <VcsDot status={() => "M"} />
                            : status ? <VcsLetter status={() => status} /> : <Box />}
                        </Dynamic>
                      </Row>
                    </Row>
                  );
                }}
              </LazyRows>
            </Column>
          )
        }
      </Dynamic>
    </Column>
  );
}

// Review › Changes | Graph. Module-level so the chosen tab survives tile re-renders.
const reviewMode = signal<"changes" | "graph">("changes");

// Git pane header pieces (the pane's own header, see TilePane): tabs drawn like the workspace
// tab bar (outline connected to the divider, hover pill) plus the active view's summary + refresh.
function ReviewTabs(props: { cwd: string }): Element {
  const layout = workspaceTabLayout;
  const tabs = [
    { id: "changes" as const, label: "Changes", icon: "file-diff", count: () => projectChanges(props.cwd).files.length },
    { id: "graph" as const, label: "Graph", icon: "git-branch", count: () => 0 },
  ];
  const outlineWidth = layout.width + layout.overhang * 2;
  const stride = layout.width + layout.stripGap;
  const activeIndex = () => tabs.findIndex((tab) => tab.id === reviewMode.value);
  return (
    <Box style={{ width: (tabs.length - 1) * stride + outlineWidth, height: spacing.headerHeight, shrink: 0 }}>
      {/* The active tab's outline replaces that span of the divider, as in the workspace tab bar. */}
      <Box style={() => ({ position: "absolute", left: activeIndex() * stride, top: spacing.headerHeight - layout.height,
        width: outlineWidth, height: layout.height })}
        paint={{ transitions: [springTransition("left")] }}>
        <Box style={{ position: "absolute", left: 0, top: layout.height - layout.stroke, width: outlineWidth, height: layout.stroke }}
          paint={() => ({ background: palette().appBg })} />
        <Image source="glyph:workspace-tab-outline" style={{ width: outlineWidth, height: layout.height }}
          paint={() => ({ color: palette().border })} />
      </Box>
      <Row style={{ position: "absolute", left: layout.overhang, top: spacing.headerHeight - layout.height,
        height: layout.height, gap: layout.stripGap, align: "end" }}>
        {tabs.map((tab) => {
          const active = () => reviewMode.value === tab.id;
          const hovered = signal(false);
          return (
            <Row style={{ width: layout.width, height: layout.height, padding: [0, spacing.unit * 1.5, 0, layout.gap * 1.5],
              align: "center", gap: layout.gap, shrink: 0 }}
              onHover={(inside) => { hovered.value = inside; }}
              onPress={() => { reviewMode.value = tab.id; }}>
              <Box style={{ position: "absolute", left: spacing.unit / 2, top: spacing.unit, width: layout.width - spacing.unit, height: spacing.button }}
                paint={() => ({ radius: radii.md, background: hovered.value && !active() ? palette().sidebarActive : 0,
                  transitions: [smoothTransition("background", motion.fast)] })} />
              <Glyph name={tab.icon} size={layout.icon}
                color={() => active() || hovered.value ? palette().textSecondary : palette().textMuted} />
              <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.label }}
                paint={() => ({ color: active() ? palette().textPrimary : hovered.value ? palette().textSecondary : palette().textMuted })}>
                {tab.label}
              </Text>
              <Show when={() => tab.count() > 0} style={{ shrink: 0 }}>
                {() => <Text font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                  paint={() => ({ color: palette().textMuted })}>{() => String(tab.count())}</Text>}
              </Show>
            </Row>
          );
        })}
      </Row>
    </Box>
  );
}

function ReviewTools(props: { cwd: string }): Element {
  const graphMode = () => reviewMode.value === "graph";
  const loading = () => graphMode() ? !!gitGraphLoading.value[props.cwd] : !!changesLoading.value[props.cwd];
  const summary = () => {
    if (!graphMode()) return loading() ? "Refreshing…" : basename(props.cwd);
    const g = gitGraphs.value[props.cwd];
    if (!g) return loading() ? "Loading…" : "";
    if (g.error) return g.error;
    return `${g.rows.length}${g.rows.length >= (g.limit ?? Infinity) ? "+" : ""} commits · all branches`;
  };
  return (
    <Row style={{ gap: spacing.unit * 2, align: "center", shrink: 1, minWidth: 0 }}>
      <Text lines={1} style={{ shrink: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
        paint={() => ({ color: palette().textMuted })}>{summary}</Text>
      <IconButton size="sm" side="bottom" icon="refresh" label={() => graphMode() ? "Refresh graph" : "Refresh changes"}
        disabled={loading} onPress={() => (graphMode() ? requestGitGraph(props.cwd) : requestChanges(props.cwd))} />
    </Row>
  );
}

// The pane header holds the tabs; the body only swaps the active view.
function ReviewView(props: { cwd: string }): Element {
  return (
    <Dynamic value={() => reviewMode.value} style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      {(mode) => mode === "graph" ? <GitGraphView cwd={props.cwd} /> : <ChangesView cwd={props.cwd} />}
    </Dynamic>
  );
}

function shortAgo(unixSeconds: number): string {
  const s = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)}d`;
  if (s < 86400 * 365) return `${Math.floor(s / (86400 * 30))}mo`;
  return `${Math.floor(s / (86400 * 365))}y`;
}

function laneColor(index: number): number {
  const lanes = palette().graphLanes;
  return lanes[index % lanes.length];
}

function GitRefBadge(props: { gitRef: GitRefView; color: number }): Element {
  const head = props.gitRef.kind === "head";
  return (
    <Box style={{ height: gitGraphLayout.refHeight, padding: [0, gitGraphLayout.refPadding, 0, gitGraphLayout.refPadding],
      align: "center", justify: "center", shrink: 1, minWidth: 0 }}
      paint={() => ({ radius: radii.sm, edge: head ? props.color : palette().border, edgeWidth: 1 })}>
      <Text lines={1} font={{ family: fontFamilies.mono, size: typeScale.tiny }}
        paint={() => ({ color: head ? props.color : palette().textSecondary })}>
        {props.gitRef.kind === "tag" ? `tag ${props.gitRef.name}` : props.gitRef.name}
      </Text>
    </Box>
  );
}

function GitGraphView(props: { cwd: string }): Element {
  const graph = () => gitGraphs.value[props.cwd];
  const layout = gitGraphLayout;
  const lanes = () => Math.min(graph()?.width ?? 1, layout.maxLanes);
  if (!graph()) requestGitGraph(props.cwd);
  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      <Show when={() => graph()?.error !== undefined} style={{ shrink: 0 }}>
        {() => <EmptyState compact title={() => graph()?.error ?? ""} style={{ padding: emptyStateLayout.padding }} />}
      </Show>
      <LazyRows items={() => graph()?.rows ?? []} key={(r) => r.hash} rowHeight={layout.rowHeight}
        padding={[spacing.unit, spacing.unit]}>
        {(r) => {
          const color = () => laneColor(r().color);
          const dotLeft = () => r().node * layout.lane + (layout.lane - layout.dot) / 2;
          return (
            <Row style={{ height: layout.rowHeight, padding: [0, layout.inset, 0, layout.inset],
              align: "center", gap: spacing.unit * 2, shrink: 0 }}
              paint={() => ({ hover: { background: rowHoverBackground() }, transitions: [smoothTransition("background", motion.fast)] })}>
              <Box style={() => ({ width: lanes() * layout.lane, height: layout.rowHeight, shrink: 0, clip: true })}>
                {/* A refresh can move a commit to other lanes: rebuild its layers when they change. */}
                <Dynamic value={() => r().layers.map((l) => `${l.color}=${l.key}`).join("\u0001")}
                  style={{ position: "absolute", left: 0, top: 0 }}>
                  {() => (
                    <Box style={{ width: (graph()?.width ?? 1) * layout.lane, height: layout.rowHeight }}>
                      {r().layers.map((layer) => (
                        <Image source={layer.key}
                          style={{ position: "absolute", left: 0, top: 0, width: (graph()?.width ?? 1) * layout.lane, height: layout.rowHeight }}
                          paint={() => ({ color: laneColor(layer.color) })} />
                      ))}
                    </Box>
                  )}
                </Dynamic>
                <Show when={() => r().node < layout.maxLanes}>
                  {() => (
                    <Box style={() => ({ position: "absolute", left: dotLeft(), top: (layout.rowHeight - layout.dot) / 2,
                      width: layout.dot, height: layout.dot })}
                      paint={() => ({ radius: layout.dot / 2, background: r().merge ? palette().appBg : color(),
                        edge: color(), edgeWidth: layout.dotRing })} />
                  )}
                </Show>
              </Box>
              <Dynamic value={() => r().refs.map((ref) => `${ref.kind}:${ref.name}`).join("\u0001") + `#${r().color}`}
                style={{ shrink: 1, minWidth: 0 }}>
                {() => (
                  <Row style={{ gap: spacing.unit, align: "center", minWidth: 0 }}>
                    {r().refs.map((ref) => <GitRefBadge gitRef={ref} color={color()} />)}
                  </Row>
                )}
              </Dynamic>
              <Text lines={1} style={{ grow: 1, minWidth: 0 }}
                font={{ family: fontFamilies.sans, size: typeScale.caption, weight: r().merge ? fontWeights.normal : fontWeights.medium }}
                paint={() => ({ color: r().merge ? palette().textMuted : palette().textPrimary })}>
                {() => r().subject}
              </Text>
              <Text lines={1} style={{ shrink: 0 }} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                paint={() => ({ color: palette().textMuted })}>{() => shortAgo(r().time)}</Text>
            </Row>
          );
        }}
      </LazyRows>
    </Column>
  );
}

function VcsLetter(props: { status: () => string }): Element {
  return (
    <Text lines={1} style={{ width: reviewLayout.statusWidth, shrink: 0 }}
      font={{ family: fontFamilies.mono, size: typeScale.tiny, weight: fontWeights.medium }}
      paint={() => ({ color: vcsColor(props.status()) })}>{props.status}</Text>
  );
}

// One line per file, like the graph: status letter, name, muted folder, line counts.
function ChangesView(props: { cwd: string }): Element {
  const changes = () => projectChanges(props.cwd);
  const initial = () => changesByProject.value[props.cwd] === undefined;
  const rowHeight = () => densityMode.value === "compact" ? reviewLayout.rowHeightCompact : reviewLayout.rowHeight;
  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      <Show when={() => changes().files.length === 0} style={{ shrink: 0 }}>
        {() => <EmptyState compact title={() => initial() ? "Loading changes…" : "No changes in the working tree."}
          style={{ padding: emptyStateLayout.padding }} />}
      </Show>
      <LazyRows items={() => changes().files} key={(f) => f.path} rowHeight={rowHeight}
        padding={[spacing.unit, spacing.unit]}>
        {(f) => {
          const dir = () => {
            const parts = f().path.split("/");
            return parts.length > 1 ? parts.slice(0, -1).join("/") : "";
          };
          return (
            <Row
              style={() => ({ height: rowHeight(), padding: [0, reviewLayout.inset, 0, reviewLayout.inset],
                align: "center", gap: spacing.unit * 2, shrink: 0 })}
              onPress={() => openDiff(f().path, props.cwd)}
              paint={() => ({ hover: { background: rowHoverBackground() },
                transitions: [smoothTransition("background", motion.fast)] })}
            >
              <VcsLetter status={() => f().status} />
              <Text lines={1} style={{ shrink: 0, maxWidth: reviewLayout.nameMaxWidth }}
                font={{ family: fontFamilies.sans, size: typeScale.caption }}
                paint={() => ({ color: palette().textPrimary })}>{() => basename(f().path)}</Text>
              <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                paint={() => ({ color: palette().textMuted })}>{dir}</Text>
              <DiffStat additions={() => f().additions} deletions={() => f().deletions} />
            </Row>
          );
        }}
      </LazyRows>
    </Column>
  );
}

// ---- Archived sessions, newest first. ------------------------------------------------------

function archivedSessions(): ArchivedItem[] {
  return sessionsState.value.archived ?? [];
}

function timeAgo(unixSeconds: number): string {
  const elapsed = Math.max(0, Math.floor(Date.now() / 1000) - unixSeconds);
  if (elapsed < 60) return "Just now";
  if (elapsed < 3600) return `${Math.floor(elapsed / 60)} min ago`;
  if (elapsed < 86400) return `${Math.floor(elapsed / 3600)} h ago`;
  return `${Math.floor(elapsed / 86400)} d ago`;
}

function cliDisplayName(id: string): string {
  if (id === "terminal") return "Terminal";
  return configState.value.clis.find((cli) => cli.id === id)?.name ?? CLI_NAMES[id] ?? id;
}

function ArchivedRow(props: { item: () => ArchivedItem }): Element {
  const hovered = signal(false);
  const menu = signal(false);
  const background = () => hovered.value || menu.value
    ? mixColor(palette().appBg, palette().textPrimary, controlState.rowHoverMix)
    : palette().appBg;
  const detail = () => {
    const item = props.item();
    return `${basename(item.cwd) || item.cwd} · ${item.alive ? "Running" : "Exited"} · ${timeAgo(item.archivedAt)}`;
  };
  const restore = () => {
    menu.value = false;
    requestHost({ type: "restoreSession", id: props.item().id });
  };
  return (
    <Row
      style={{ padding: archivedLayout.rowPadding, align: "center", gap: spacing.unit * 3, shrink: 0 }}
      onHover={(inside) => { hovered.value = inside; }}
      onPress={restore}
      onContextMenu={() => { menu.value = true; }}
      paint={() => ({
        radius: radii.lg,
        background: background(),
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Image source={() => "cli:" + props.item().cli}
        style={{ width: archivedLayout.icon, height: archivedLayout.icon, shrink: 0 }}
        paint={() => ({ opacity: hovered.value ? 1 : workspaceTabLayout.dimOpacity, transitions: [smoothTransition("opacity", motion.fast)] })} />
      <Column style={{ grow: 1, minWidth: 0, gap: spacing.unit / 2 }}>
        <FadeText
          text={() => cleanTitle(props.item().title) || cliDisplayName(props.item().cli)}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          color={() => (hovered.value ? palette().textPrimary : palette().textSecondary)}
          background={background}
          layoutChanged={() => { void tilingState.value; void resizeDrag.value; }}
        />
        <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
          paint={() => ({ color: palette().textMuted })}>
          {detail}
        </Text>
      </Column>
      <SessionMenuButton open={menu} visible={hovered} side="bottom">
        <MenuItem icon="arrow-up" label="Restore session" onPress={restore} />
        <MenuItem icon="folder-open" label="Show project folder" onPress={() => {
          menu.value = false;
          revealPath(props.item().cwd);
        }} />
        <Show when={() => !props.item().alive}>
          {() => <MenuItem icon="trash" label="Remove exited session" onPress={() => {
            menu.value = false;
            closeSession(props.item().id);
          }} />}
        </Show>
      </SessionMenuButton>
    </Row>
  );
}

function ArchivedView(): Element {
  const layout = archivedLayout;
  return (
    <Dynamic value={() => archivedSessions().length === 0} style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      {(empty) => empty ? (
        <Column style={{ grow: 1, minHeight: 0, scroll: true, align: "center", padding: layout.padding }}>
          <Box style={{ grow: 1 }} />
          <Column style={{ minWidth: 0, maxWidth: layout.emptyWidth, shrink: 0, align: "center", gap: layout.headerGap }}>
            <Glyph name="archive" size={layout.icon} color={() => palette().textSecondary} />
            <EmptyState centered title="No archived sessions"
              detail="Choose Archive session from a session’s menu. You can restore it here." />
          </Column>
          <Box style={{ grow: 1 }} />
        </Column>
      ) : (
        <Column style={{ grow: 1, minHeight: 0, scroll: true }}>
          <Row style={{ justify: "center", shrink: 0, padding: layout.padding }}>
            <Column style={{ grow: 1, minWidth: 0, maxWidth: layout.maxWidth, gap: layout.headerGap }}>
              <Text font={{ family: fontFamilies.sans, size: typeScale.heading, weight: fontWeights.medium }}
                paint={() => ({ color: palette().textPrimary })}>Archived sessions</Text>
              <For each={archivedSessions} key={(item) => item.id} style={{ gap: spacing.hairline * 2 }}>
                {(item) => <ArchivedRow item={item} />}
              </For>
            </Column>
          </Row>
        </Column>
      )}
    </Dynamic>
  );
}

// Whole-pixel line pitch: a fractional one (12.5 × 1.5 = 18.75) leaves an antialiased seam
// between the selection fills of two lines.
const CODE_LINE = Math.round(editorLayout.codeSize * editorLayout.codeLineHeight);
const CODE_FONT = { family: fontFamilies.mono, size: editorLayout.codeSize, lineHeight: CODE_LINE / editorLayout.codeSize };
const CODE_CHAR = CODE_FONT.size * editorLayout.monoAdvance;
const CODE_GUTTER = editorLayout.gutterWidth + editorLayout.gutterGap + editorLayout.lineEndPadding;
const DIFF_CONTEXT = 3;
const DIFF_STEP = 10;

// Thin auto-hiding scrollbar shared by the editor and tree (Antigravity's overlay scrollbar).
function editorScrollbar(): Scrollbar {
  return {
    width: editorLayout.scrollbarWidth, thumb: palette().ring, radius: radii.sm,
    inset: editorLayout.scrollbarInset, minThumb: editorLayout.scrollbarMinThumb, autoHide: true,
  };
}

const codeColumns = new WeakMap<{ text: string }, number>();

function visibleCode(text: string): string {
  const max = editorLayout.maxDrawColumns;
  if (text.length <= max) return text || " ";
  const last = text.charCodeAt(max - 1);
  return text.slice(0, last >= 0xd800 && last <= 0xdbff ? max - 1 : max) + "…";
}

// Splits a line into visual rows using word-or-glyph wrapping matching JSNIR's cosmic-text
// layout (`Wrap::WordOrGlyph`), so wrapped lines map pointer (x, y) to the exact character.
function wrapVisualLines(text: string, perRow: number): Array<{ start: number; end: number }> {
  const limit = Math.max(1, perRow);
  if (text.length === 0) return [{ start: 0, end: 0 }];
  const out: Array<{ start: number; end: number }> = [];
  let lineStart = 0;
  let col = 0;
  let breakIdx = -1; // index right after the last whitespace on this visual line
  let breakCol = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    const w = ch === "\t" ? editorLayout.tabColumns : 1;
    const isSpace = ch === " " || ch === "\t";
    if (col + w > limit && i > lineStart) {
      if (isSpace) {
        // Consume trailing spaces at the wrap boundary (cosmic-text keeps them on the wrapped row).
        out.push({ start: lineStart, end: i + 1 });
        lineStart = i + 1;
        col = 0;
        breakIdx = -1;
        continue;
      }
      if (breakIdx > lineStart) {
        out.push({ start: lineStart, end: breakIdx });
        lineStart = breakIdx;
        col -= breakCol;
        breakIdx = -1;
        // Re-check if the carried word itself still overflows `limit`.
        if (col + w > limit && i > lineStart) {
          out.push({ start: lineStart, end: i });
          lineStart = i;
          col = 0;
        }
      } else {
        out.push({ start: lineStart, end: i });
        lineStart = i;
        col = 0;
      }
    }
    col += w;
    if (isSpace) {
      breakIdx = i + 1;
      breakCol = col;
    }
  }
  if (lineStart < text.length || out.length === 0) {
    out.push({ start: lineStart, end: text.length });
  }
  return out;
}

function codeRowHeight(line: { text: string }, perRow: number): number {
  const text = visibleCode(line.text);
  let columns = codeColumns.get(line);
  if (columns === undefined) {
    columns = text.length;
    for (let i = text.indexOf("\t"); i !== -1; i = text.indexOf("\t", i + 1)) {
      columns += editorLayout.tabColumns - 1;
    }
    codeColumns.set(line, columns);
  }
  if (columns <= perRow) return CODE_LINE;
  return wrapVisualLines(text, perRow).length * CODE_LINE;
}

function codeWrapColumns(width: number): number {
  return Math.max(editorLayout.minWrapColumns, Math.floor((width - CODE_GUTTER) / CODE_CHAR));
}

function columnAt(text: string, x: number, y: number, rowWidth: number): number {
  const visual = wrapVisualLines(text, codeWrapColumns(rowWidth));
  const rowIndex = Math.max(0, Math.min(visual.length - 1, Math.floor(Math.max(0, y) / CODE_LINE)));
  const seg = visual[rowIndex];
  const cells = Math.max(0, x - editorLayout.gutterWidth - editorLayout.gutterGap) / CODE_CHAR;
  let cell = 0;
  for (let i = seg.start; i < seg.end; i++) {
    const width = text[i] === "\t" ? editorLayout.tabColumns : 1;
    if (cell + width / 2 > cells) return text.charCodeAt(i) >= 0xdc00 && text.charCodeAt(i) <= 0xdfff ? i - 1 : i;
    cell += width;
  }
  return seg.end;
}

// Splits spans at the selection bounds [from, to) so the selected part gets the selection fill;
// a `to` past the text adds the selected line-break cell.
// `radius` rounds a one-line selection; lines of a longer one stay square so they join into one block.
function selectSpans(spans: Span[], from: number, to: number, radius: number): Span[] {
  const out: Span[] = [];
  let start = 0;
  for (const span of spans) {
    const text = span.text ?? "";
    const end = start + text.length;
    for (const [a, b, selected] of [[start, Math.min(end, from), false], [Math.max(start, from), Math.min(end, to), true], [Math.max(start, to), end, false]] as [number, number, boolean][]) {
      if (b <= a) continue;
      const piece = { ...span, text: text.slice(a - start, b - start) };
      if (selected) {
        piece.background = selectionFill();
        piece.backgroundRadius = radius;
      }
      out.push(piece);
    }
    start = end;
  }
  // The line break past the text (see `lineSelection`).
  if (from <= start && to > start) out.push({ text: " ", background: selectionFill(), backgroundRadius: radius });
  return out;
}

function firstAbove(offsets: number[], y: number): number {
  let lo = 0;
  let hi = offsets.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (offsets[mid] <= y) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

// Text position as [0-based line, UTF-16 column]; RowPoint is a pointer in a mounted or estimated row.
type TextPoint = [number, number];
// The selected [from, to) columns of one line and the fill's corner radius.
type LineSelection = [from: number, to: number, radius: number];
interface RowPoint { index: number; x: number; y: number; top: number; width: number }

// Windowed scroll list: only rows near the viewport are mounted, the rest are two spacers sized
// from estimated heights. A 10k-line file or diff keeps ~80 live rows (mounting every line, as
// the side panel did, stalled every layout and every spring frame).
function LazyRows<T>(props: {
  items: () => T[];
  key: (item: T) => string;
  reveal?: () => string | null;
  // Where the revealed row lands, as a fraction of the viewport (its top at 1/3 by default; 0.5 centres
  // it), and whether the list glides there instead of jumping.
  revealAlign?: number;
  revealSmooth?: boolean;
  resetScrollOn?: () => string;
  padding?: [number, number];
  measureKey?: (width: number) => number;
  paintOffset?: () => [number, number];
  select?: (from: RowPoint, to: RowPoint, phase: DragPhase) => void;
  overlay?: () => Element;
  children: (item: () => T, ref?: (id: number) => void) => Element;
} & ({ estimate: (item: T, measure: number) => number; rowHeight?: never } |
  { rowHeight: Source<number>; estimate?: never })): Element {
  const view = signal({ top: 0, client: 0 });
  const measureKey = props.measureKey ?? ((width: number) => width);
  const measureWidth = signal(measureKey(editorLayout.fallbackWidth));
  const items = signal<T[]>([]);
  const offsets = signal<number[]>([0]);
  const visible = signal<[number, number]>([0, 0]);
  const fixedHeight = () => props.rowHeight === undefined ? 0 : read(props.rowHeight);
  const rowTop = (index: number) => {
    const height = fixedHeight();
    return height ? index * height : offsets.value[index] ?? 0;
  };
  let first = 0;
  let last = 0;
  let nodeId: number | undefined;
  const [padTop, padBottom] = props.padding ?? [editorLayout.paddingTop, editorLayout.paddingBottom];

  const measure = () => {
    if (nodeId === undefined) return;
    if (props.estimate) {
      const w = jsnir.columns.width[nodeId] ?? 0;
      if (w > 0) measureWidth.value = measureKey(w);
    }
    const client = jsnir.columns.clientHeight[nodeId] ?? 0;
    if (client > 0 && client !== view.value.client) view.value = { top: view.value.top, client };
  };
  let measuring = false;
  let settling = false;
  effect(() => {
    void viewportSize.value;
    void tilingState.value;
    if (!measuring) {
      measuring = true;
      afterView(editorLayout.measureDelay, () => { measuring = false; measure(); });
    }
    if (!settling) {
      settling = true;
      afterView(editorLayout.settleDelay, () => { settling = false; measure(); });
    }
  });
  // Snapshot the source independently of scroll: flattening a diff/tree is O(N), not per wheel event.
  effect(() => { items.value = props.items(); });
  const estimate = props.estimate;
  effect(() => {
    if (!estimate) return;
    const rows = items.value;
    const w = measureWidth.value;
    const out = new Array<number>(rows.length + 1);
    out[0] = 0;
    for (let i = 0; i < rows.length; i++) out[i + 1] = out[i] + estimate(rows[i], w);
    offsets.value = out;
  });

  let selectionStart: RowPoint = { index: 0, x: 0, y: 0, top: 0, width: 0 };
  const select = props.select;
  const rowNodes = new Map<string, number>();
  const onDrag = select ? (e: DragEvent) => {
    if (nodeId === undefined || !items.value.length) return;
    const [ox, oy] = props.paintOffset?.() ?? [0, 0];
    const y = (jsnir.columns.y[nodeId] ?? 0);
    const scrollTop = jsnir.columns.scrollTop[nodeId] ?? view.value.top;
    const find = (atX: number, atY: number): RowPoint => {
      // e.x/e.y are in window space; Taffy `jsnir.columns.x/y` exclude ancestor `paint.x/y`
      // from tiled columns and stacked panes. Convert into Taffy space first.
      const lx = atX - ox;
      const ly = atY - oy;
      // Wrapped rows may differ from estimates (wide glyphs, word wrapping). Prefer the
      // actual JSNIR layout of mounted rows so we send the lines under the pointer.
      const layoutY = Math.max(y, Math.min(y + view.value.client - 1, ly)) + scrollTop;
      for (let i = first; i < last; i++) {
        const row = items.value[i];
        if (!row) break;
        const node = rowNodes.get(props.key(row));
        if (node === undefined) continue;
        const top = jsnir.columns.y[node];
        if (layoutY >= top && layoutY < top + jsnir.columns.height[node]) {
          return { index: i, x: lx - jsnir.columns.x[node], y: layoutY - top, top: top - y, width: jsnir.columns.width[node] };
        }
      }
      const local = Math.max(0, layoutY - y - padTop);
      const height = fixedHeight();
      const index = Math.min(items.value.length - 1, height ? Math.floor(local / height) : firstAbove(offsets.value, local));
      return { index, x: lx - jsnir.columns.x[nodeId!], y: 0, top: padTop + rowTop(index), width: jsnir.columns.width[nodeId!] };
    };
    if (e.phase === "start") selectionStart = find(e.startX, e.startY);
    select(selectionStart, find(e.x, e.y), e.phase);
  } : undefined;
  effect(() => {
    const height = fixedHeight();
    const n = height ? items.value.length : offsets.value.length - 1;
    const top = Math.max(0, view.value.top - padTop - editorLayout.overscan);
    const bottom = view.value.top + (view.value.client || editorLayout.fallbackHeight) + editorLayout.overscan;
    const at = (y: number) => height ? Math.floor(y / height) : firstAbove(offsets.value, y);
    const a = Math.min(n, at(top));
    const b = Math.min(n, at(bottom) + 1);
    // No For reconciliation or spacer writes until a row crosses the overscan boundary.
    if (a !== first || b !== last) {
      first = a;
      last = b;
      visible.value = [a, b];
    }
  });
  const resetScrollOn = props.resetScrollOn;
  if (resetScrollOn) {
    let resetKey: string | undefined;
    effect(() => {
      const key = resetScrollOn();
      if (key === resetKey) return;
      resetKey = key;
      if (nodeId !== undefined) scrollTo(nodeId, { to: "start" });
    });
  }
  // Scroll a target row (e.g. `file:120`) a third of the way down once laid out.
  let revealVersion = 0;
  effect(() => {
    const target = props.reveal?.() ?? null;
    const version = ++revealVersion;
    if (target === null) return;
    afterView(editorLayout.revealDelay, () => {
      if (nodeId === undefined || version !== revealVersion) return;
      const index = items.value.findIndex((item) => props.key(item) === target);
      if (index < 0) return;
      const client = view.value.client || editorLayout.fallbackHeight;
      const offset = props.revealAlign === undefined
        ? padTop + rowTop(index) - client / 3
        : padTop + (rowTop(index) + rowTop(index + 1)) / 2 - client * props.revealAlign;
      scrollTo(nodeId, { offset: Math.max(0, offset), smooth: props.revealSmooth });
    });
  });

  return (
    <Column
      ref={(id) => {
        nodeId = id;
        afterView(editorLayout.measureDelay, measure);
      }}
      style={{ grow: 1, minHeight: 0, scroll: true }}
      onDrag={onDrag}
      onScroll={(e) => {
        if (Math.abs(e.top - view.value.top) > 4 || e.client !== view.value.client) view.value = { top: e.top, client: e.client };
      }}
      paint={() => ({ scrollbar: editorScrollbar() })}
    >
      <Box style={() => ({ height: padTop + rowTop(visible.value[0]), shrink: 0 })} />
      <For
        each={() => {
          const [a, b] = visible.value;
          return items.value.slice(a, b);
        }}
        key={props.key}
        style={{ shrink: 0 }}
      >
        {(item) => {
          if (!select) return props.children(item);
          const key = props.key(item());
          onCleanup(() => rowNodes.delete(key));
          return props.children(item, (id) => { rowNodes.set(key, id); });
        }}
      </For>
      <Box
        style={() => {
          return { height: padBottom + Math.max(0, rowTop(items.value.length) - rowTop(visible.value[1])), shrink: 0 };
        }}
      />
      {props.overlay?.()}
    </Column>
  );
}

function SourceLines(props: {
  doc: () => ViewerView;
  paintOffset?: () => [number, number];
  select?: (anchor: TextPoint, head: TextPoint, phase: DragPhase, at: RowPoint) => void;
  selection?: (num: number) => LineSelection | null;
  overlay?: () => Element;
}): Element {
  return (
    <LazyRows
      items={() => props.doc().lines}
      key={(line) => String(line.num)}
      estimate={codeRowHeight}
      measureKey={codeWrapColumns}
      paintOffset={props.paintOffset}
      select={props.select && ((from, to, phase) => {
        const lines = props.doc().lines;
        const point = (p: RowPoint): TextPoint => [p.index, columnAt(lines[p.index]?.text ?? "", p.x, p.y, p.width)];
        props.select?.(point(from), point(to), phase, to);
      })}
      overlay={props.overlay}
      reveal={() => {
        const target = props.doc().targetLine;
        return target == null ? null : String(Math.max(1, target));
      }}
    >
      {(line, ref) => {
        // Per-row range: only a row whose selection changed re-lays out its spans.
        const range = signal<LineSelection | null>(null);
        let current = "";
        effect(() => {
          const next = props.selection?.(line().num) ?? null;
          const key = next ? next.join(":") : "";
          if (key === current) return;
          current = key;
          range.value = next;
        });
        return (
        <Row
          ref={ref}
          style={{ padding: [0, editorLayout.lineEndPadding, 0, 0], gap: editorLayout.gutterGap, align: "start", shrink: 0 }}
          paint={() => ({ background: line().highlighted ? accent().brandSoft : undefined })}
        >
          <Text
            style={{ width: editorLayout.gutterWidth, shrink: 0 }}
            align="end"
            font={CODE_FONT}
            paint={() => ({ color: line().highlighted ? accent().brandText : palette().textMuted })}
          >
            {() => String(line().num)}
          </Text>
          <Text style={{ grow: 1, minWidth: 0 }} font={CODE_FONT} spans={() => {
            const r = range.value;
            return r ? selectSpans(codeSpans(line()), r[0], r[1], r[2]) : codeSpans(line());
          }}>
            {() => visibleCode(line().text)}
          </Text>
        </Row>
        );
      }}
    </LazyRows>
  );
}

// Antigravity diff: the whole file as context, unchanged runs folded to DIFF_CONTEXT lines around
// each change. A fold shows "+N more lines" (reveal all) and two "+10" buttons that reveal ten
// lines from its top or bottom edge.
interface FoldState {
  top: number;
  bottom: number;
  all: boolean;
}

type DiffItem =
  | { kind: "line"; key: string; line: DiffLineView }
  | { kind: "fold"; key: string; start: number; hidden: number };

function diffItems(lines: DiffLineView[], folds: Record<number, FoldState>): DiffItem[] {
  const items: DiffItem[] = [];
  let i = 0;
  while (i < lines.length) {
    if (lines[i].kind !== "ctx") {
      items.push({ kind: "line", key: `l${i}`, line: lines[i] });
      i++;
      continue;
    }
    let end = i;
    while (end < lines.length && lines[end].kind === "ctx") end++;
    // Context kept after the previous change and before the next one (none at the file edges).
    const head = i === 0 ? 0 : DIFF_CONTEXT;
    const tail = end === lines.length ? 0 : DIFF_CONTEXT;
    const fold = folds[i] ?? { top: 0, bottom: 0, all: false };
    const shownTop = Math.min(end - i, head + fold.top);
    const shownBottom = Math.min(end - i - shownTop, tail + fold.bottom);
    const hidden = fold.all ? 0 : end - i - shownTop - shownBottom;
    if (hidden <= 2) {
      for (let k = i; k < end; k++) items.push({ kind: "line", key: `l${k}`, line: lines[k] });
    } else {
      for (let k = i; k < i + shownTop; k++) items.push({ kind: "line", key: `l${k}`, line: lines[k] });
      items.push({ kind: "fold", key: `f${i}`, start: i, hidden });
      for (let k = end - shownBottom; k < end; k++) items.push({ kind: "line", key: `l${k}`, line: lines[k] });
    }
    i = end;
  }
  return items;
}

function FoldRow(props: { item: () => { start: number; hidden: number }; reveal: (patch: Partial<FoldState>) => void }): Element {
  const smallButton = (label: string, icon: string, onPress: () => void) => (
    <Row
      style={{ height: 22, padding: [0, 6, 0, 5], align: "center", gap: 3 }}
      onPress={onPress}
      paint={() => ({
        radius: radii.sm,
        background: palette().panelBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        hover: { background: palette().sidebarActive },
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Glyph name={icon} size={11} color={() => palette().textMuted} />
      <Text font={{ family: fontFamilies.sans, size: typeScale.tiny }} paint={() => ({ color: palette().textSecondary })}>
        {label}
      </Text>
    </Row>
  );
  return (
    <Row
      style={{ height: editorLayout.foldHeight, align: "center", justify: "center", gap: 8, shrink: 0 }}
      paint={() => ({ background: palette().panelBg })}
    >
      <Box style={{ position: "absolute", left: 0, top: 26, width: 4000, height: 1 }} paint={() => ({ background: palette().border })} />
      <Row
        style={{ height: 26, padding: [0, 12, 0, 12], align: "center" }}
        onPress={() => props.reveal({ all: true })}
        paint={() => ({
          radius: radii.md,
          background: palette().appBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
          hover: { background: palette().sidebarActive },
          transitions: [smoothTransition("background", motion.fast)],
        })}
      >
        <Text font={{ family: fontFamilies.sans, size: typeScale.caption }} paint={() => ({ color: palette().textSecondary })}>
          {() => `+${props.item().hidden} more lines`}
        </Text>
      </Row>
      <Column style={{ gap: 2 }}>
        {smallButton(`+${DIFF_STEP}`, "chevrons-up-down", () => props.reveal({ top: DIFF_STEP }))}
        {smallButton(`+${DIFF_STEP}`, "chevron-down", () => props.reveal({ bottom: DIFF_STEP }))}
      </Column>
    </Row>
  );
}

function DiffLineRow(props: { line: () => DiffLineView; focused: () => boolean }): Element {
  const kind = () => props.line().kind;
  const tint = () => (kind() === "add" ? palette().diffAddText : kind() === "del" ? palette().diffDelText : 0);
  return (
    <Row
      style={{ padding: [0, editorLayout.lineEndPadding, 0, 0], gap: editorLayout.gutterGap, align: "start", shrink: 0 }}
      paint={() => ({
        background: kind() === "add" ? palette().diffAddBg : kind() === "del" ? palette().diffDelBg : 0,
        edge: props.focused() ? accent().brand : tint(),
        edgeWidth: tint() ? editorLayout.diffMarker : 0,
        edgeSides: [0, 0, 0, 1],
      })}
    >
      <Text style={{ width: editorLayout.gutterWidth, shrink: 0 }} align="end" font={CODE_FONT}
        paint={() => ({ color: props.focused() ? accent().brandText : palette().textMuted })}>
        {() => String(props.line().newNum ?? props.line().oldNum ?? "")}
      </Text>
      <Text style={{ grow: 1, minWidth: 0 }} font={CODE_FONT} spans={() => codeSpans(props.line())}>
        {() => visibleCode(props.line().text)}
      </Text>
    </Row>
  );
}

function DiffLines(props: { doc: () => ViewerView }): Element {
  // Folds belong to this window's mount: opening another file remounts with fresh folds.
  const folds = signal<Record<number, FoldState>>({});
  const target = () => {
    const doc = props.doc();
    return doc.diffFocus === undefined ? null : `l${doc.diffFocus}`;
  };
  const reveal = (start: number, patch: Partial<FoldState>) => {
    const cur = folds.value[start] ?? { top: 0, bottom: 0, all: false };
    folds.value = {
      ...folds.value,
      [start]: { top: cur.top + (patch.top ?? 0), bottom: cur.bottom + (patch.bottom ?? 0), all: patch.all ?? cur.all },
    };
  };
  return (
    <LazyRows
      items={() => {
        const doc = props.doc();
        return doc.edit ? doc.diff.map<DiffItem>((line, index) => ({ kind: "line", key: `l${index}`, line })) :
          diffItems(doc.diff, folds.value);
      }}
      key={(item) => item.key}
      estimate={(item, w) => (item.kind === "fold" ? editorLayout.foldHeight : codeRowHeight(item.line, w))}
      measureKey={codeWrapColumns}
      reveal={target}
    >
      {(item) => (
        <Dynamic value={() => item().kind}>
          {(kind) =>
            kind === "fold" ? (
              <FoldRow
                item={() => item() as { start: number; hidden: number }}
                reveal={(patch) => reveal((item() as { start: number }).start, patch)}
              />
            ) : (
              <DiffLineRow line={() => (item() as { line: DiffLineView }).line} focused={() => item().key === target()} />
            )
          }
        </Dynamic>
      )}
    </LazyRows>
  );
}

// A file or diff window's body: the cached doc, or loading / error states.
function DocView(props: { spec: TileSpec; paintOffset?: () => [number, number] }): Element {
  // A response for a different document must not rebuild this window's folds/height index.
  const current = signal<ViewerView | undefined>(undefined);
  effect(() => { current.value = docCache.value[docKey(props.spec)]; });
  const doc = () => current.value;
  return (
    <Dynamic
      value={() => {
        const d = doc();
        return !d ? "loading" : d.error ? "error" : d.preview?.kind ?? (d.mode === "diff" && d.diff.length === 0 ? "empty" : d.mode);
      }}
      style={{ grow: 1, minWidth: 0, minHeight: 0 }}
    >
      {(state) =>
        state === "diff" ? (
          <DiffLines doc={() => doc() as ViewerView} />
        ) : state === "file" ? (
          <SourceEditor spec={props.spec} doc={() => doc() as ViewerView} paintOffset={props.paintOffset} />
        ) : state === "font" || state === "binary" ? (
          <SpecialFilePreview keyForDoc={() => doc() as ViewerView} />
        ) : state === "error" ? (
          <Column style={{ padding: [16, 16, 16, 16], gap: 6 }}>
            <Text
              font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
              paint={() => ({ color: palette().destructive })}
            >
              {props.spec.kind === "diff" ? (props.spec.edit ? "Edit unavailable" : "Cannot open diff") : "Cannot open file"}
            </Text>
            <Text font={{ family: fontFamilies.mono, size: typeScale.caption }} paint={() => ({ color: palette().textSecondary })}>
              {() => doc()?.error ?? ""}
            </Text>
          </Column>
        ) : (
          <Column style={{ grow: 1, align: "center", justify: "center" }}>
            <EmptyState compact title={state === "empty" ? "No text changes to display." : "Loading…"} />
          </Column>
        )
      }
    </Dynamic>
  );
}

// "+N −M" beside a diff or review window's title.
function diffStatOf(spec: TileSpec): [number, number] | null {
  if (spec.kind === "review") {
    const changes = projectChanges(spec.cwd);
    return [changes.additions, changes.deletions];
  }
  if (spec.kind !== "diff") return null;
  const doc = docCache.value[docKey(spec)];
  return doc ? [doc.additions, doc.deletions] : null;
}

function OpenInButton(props: {
  open: Signal<boolean>;
  path: () => string;
  cwd: () => string | undefined;
  line: () => number | undefined;
  col: () => number | undefined;
}): Element {
  let anchor: number | undefined;
  const close = () => {
    props.open.value = false;
  };
  const systemChecked = () => {
    const preferred = configState.value.preferredEditor;
    return !preferred || !externalEditorsState.value.some((editor) => editor.id === preferred);
  };
  return (
    <Box
      ref={(id) => {
        anchor = id;
      }}
      style={{ shrink: 0 }}
    >
      <IconButton
        size="sm"
        side="bottom"
        icon="external-link"
        label="Open in…"
        highlighted={() => props.open.value}
        onPress={() => {
          props.open.value = !props.open.value;
        }}
      />
      <MenuPanel open={props.open} anchor={() => anchor} side="bottom" align="end">
        <MenuLabel label="Open in" />
        <OpenInEditorItems
          path={props.path}
          cwd={props.cwd}
          line={props.line}
          col={props.col}
          onSelect={close}
        />
        <MenuItem
          icon="external-link"
          label="System default"
          checked={systemChecked}
          onPress={() => {
            close();
            openInEditor("system", props.path(), props.cwd(), props.line(), props.col());
          }}
        />
        <MenuSeparator />
        <MenuItem
          icon="folder-open"
          label="Show in file manager"
          onPress={() => {
            close();
            revealPath(props.path(), props.cwd());
          }}
        />
        <MenuItem
          icon="copy"
          label="Copy path"
          onPress={() => {
            close();
            copyText(props.path());
          }}
        />
      </MenuPanel>
    </Box>
  );
}

// Per-kind actions in a window's header, left of the shared maximize / close.
function TileActions(props: { id: () => number; spec: () => TileSpec; menuOpen: Signal<boolean> }): Element {
  return (
    <Dynamic value={() => props.spec().kind} style={{ shrink: 0 }}>
      {(kind) => {
        if (kind === "file" || kind === "diff") {
          const doc = () => docCache.value[docKey(props.spec())];
          const isDiff = kind === "diff";
          const isSingleEdit = () => isDiff && !!props.spec().edit;
          const filePath = () => doc()?.displayPath || props.spec().path;
          const resolvedPath = () => doc()?.path || props.spec().path;
          const targetLine = () => {
            const explicit = props.spec().line ?? doc()?.targetLine ?? undefined;
            if (explicit) return explicit;
            if (isDiff) {
              const firstChange = doc()?.diff.find((row) => row.kind !== "ctx" && (row.newNum ?? row.oldNum));
              return firstChange?.newNum ?? firstChange?.oldNum ?? undefined;
            }
            return undefined;
          };
          return (
            <Row style={{ align: "center", gap: 0 }}>
              <Show when={() => isDiff || !!props.spec().edit || changeFor(filePath(), props.spec().cwd) !== undefined}>
                {() => (
                  <IconButton size="sm" side="bottom"
                    icon={isDiff ? "file-code" : "file-diff"}
                    label={isDiff ? "View file" : "View diff"}
                    onPress={() => setTile(props.id(), { ...props.spec(), kind: isDiff ? "file" : "diff" })}
                  />
                )}
              </Show>
              <Show when={isSingleEdit}>
                {() => (
                  <IconButton size="sm" side="bottom"
                    icon="file-diff"
                    label="View full diff"
                    onPress={() => setTile(props.id(), { ...props.spec(), kind: "diff", edit: undefined })}
                  />
                )}
              </Show>
              <IconButton size="sm" side="bottom"
                icon="pin"
                highlighted={() => !!props.spec().pinned}
                label={() => (props.spec().pinned ? "Unpin pane" : "Pin pane")}
                onPress={() => setTile(props.id(), { ...props.spec(), pinned: !props.spec().pinned }, false)}
              />
              <IconButton size="sm" side="bottom" icon="refresh" label="Reload" onPress={() => loadTile(props.spec())} />
              <OpenInButton
                open={props.menuOpen}
                path={resolvedPath}
                cwd={() => props.spec().cwd ?? workspacePath.value}
                line={targetLine}
                col={() => props.spec().col ?? doc()?.targetCol ?? undefined}
              />
            </Row>
          );
        }
        if (kind === "review") {
          return <Box />; // Review keeps its refresh control visible in its own toolbar.
        }
        if (kind === "metrics") {
          return <IconButton size="sm" side="bottom" icon="refresh" label="Refresh local usage"
            disabled={() => metricsScanState.value === "loading"} onPress={scanMetrics} />;
        }
        if (kind === "files") {
          return (
            <IconButton size="sm" side="bottom"
              icon="refresh"
              label="Refresh"
              onPress={() => {
                const cwd = props.spec().cwd ?? workspacePath.value;
                const tree = projectTree(cwd);
                listFiles("", cwd);
                for (const entry of flattenTree(tree)) {
                  if (entry.isDir && tree.expanded.value.has(entry.path)) listFiles(entry.path, cwd);
                }
                requestChanges(cwd);
              }}
            />
          );
        }
        return <Box />;
      }}
    </Dynamic>
  );
}

function TileBody(props: { id: number; spec: TileSpec; paintOffset?: () => [number, number] }): Element {
  const kind = props.spec.kind;
  if (kind === "file" || kind === "diff") return <DocView spec={props.spec} paintOffset={props.paintOffset} />;
  if (kind === "browser") return <BrowserView id={props.id} spec={props.spec} />;
  const cwd = props.spec.cwd ?? workspacePath.value;
  if (kind === "files") return <FilesTreeView cwd={cwd} />;
  if (kind === "review") return <ReviewView cwd={cwd} />;
  if (kind === "overview") return <OverviewView cwd={cwd} />;
  if (kind === "metrics") return <MetricsView />;
  return <ArchivedView />;
}

// A non-terminal window: same card, header and focus ring as a terminal pane (`TerminalPane`),
// so files, diffs and review read as windows of one compositor.
function TilePane(props: { id: () => number; focused: () => boolean; multiPane: () => boolean; paintOffset?: () => [number, number] }): Element {
  const hovered = signal(false);
  const menuOpen = signal(false);
  const spec = (): TileSpec => tileOf(props.id()) ?? { kind: "files", path: "" };
  const showActions = () => hovered.value || menuOpen.value;
  // Git panes keep one header with their tabs, even fullscreen.
  const review = () => spec().kind === "review";
  const headerShown = () => props.multiPane() || review();
  return (
    <PaneFrame
      focused={props.focused}
      multiPane={props.multiPane}
      onHover={(inside) => {
        hovered.value = inside;
      }}
      onPress={() => {
        if (!props.focused()) selectSession(props.id());
      }}
    >
      {/* Always mounted so fullscreen folds it with the tiling spring (see TerminalPane). */}
      <Row
        style={() => ({ height: review() ? spacing.headerHeight : props.multiPane() ? 30 : 0,
          padding: [0, 4, 0, review() ? 0 : 10], align: "center", gap: 6, shrink: 0, clip: true })}
        onDrag={windowDragHandler(props.id)}
        paint={() => ({
          cursor: "grab", edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [0, 0, 1, 0],
          opacity: headerShown() ? 1 : 0,
          transitions: [springTransition("height"), smoothTransition("opacity", motion.fast)],
        })}
      >
        <Dynamic value={review} style={() => ({ grow: 1, minWidth: 0, height: review() ? spacing.headerHeight : 30 })}>
          {(git) => git ? (
            <Row style={{ grow: 1, minWidth: 0, align: "center", gap: 6 }}>
              <ReviewTabs cwd={spec().cwd ?? workspacePath.value} />
              <Box style={{ grow: 1, minWidth: 0 }} />
              <ReviewTools cwd={spec().cwd ?? workspacePath.value} />
            </Row>
          ) : (
            <Row style={{ grow: 1, minWidth: 0, align: "center", gap: 6 }}>
              <Glyph name={() => tileGlyph(spec())} size={14} color={() => palette().textMuted} />
              {/* Title always visible; the full path lives in the page header while focused. */}
              <Box style={{ grow: 1, minWidth: 0 }}>
                <FadeText
                  text={() => tileTitle(spec(), props.id())}
                  font={{ family: fontFamilies.sans, size: typeScale.label }}
                  color={() => (props.focused() ? palette().textPrimary : palette().textSecondary)}
                  background={() => palette().appBg}
                  layoutChanged={() => {
                    void tilingState.value;
                    void resizeDrag.value;
                    void hovered.value;
                    void menuOpen.value;
                    void diffStatOf(spec());
                  }}
                />
              </Box>
            </Row>
          )}
        </Dynamic>
        <Show when={() => diffStatOf(spec()) !== null} style={{ shrink: 0 }}>
          {() => (
            <DiffStat additions={() => diffStatOf(spec())?.[0] ?? 0} deletions={() => diffStatOf(spec())?.[1] ?? 0} />
          )}
        </Show>
        {/* Static pin indicator at rest so pinned panes read clearly while browsing other files. */}
        <Show when={() => !!spec().pinned && !showActions()} style={{ shrink: 0 }}>
          {() => (
            <Box style={{ padding: [0, 4, 0, 2], align: "center", justify: "center", shrink: 0 }}>
              <Glyph name="pin" size={12} color={() => accent().brandText} />
            </Box>
          )}
        </Show>
        {/* Actions mount on hover (or while the Open in menu is open), so a narrow window keeps its title. */}
        <Show when={showActions} style={{ shrink: 0 }}>
          {() => (
            <Row
              style={{ align: "center", gap: 0, shrink: 0 }}
              paint={{ starting: { opacity: 0 }, transitions: [smoothTransition("opacity", motion.fast)] }}
            >
              <TileActions id={props.id} spec={spec} menuOpen={menuOpen} />
              <Show when={() => tilingState.value.mode === "niri" && !tilingState.value.maximized} style={{ shrink: 0 }}>
                {() => <IconButton size="sm" side="bottom" icon="columns" label="Maximize column" keys={() => keysFor("layout.maximizeColumn")} onPress={() => {
                  menuOpen.value = false;
                  if (!props.focused()) selectSession(props.id());
                  tilingAction("maximizeColumn");
                }} />}
              </Show>
              <IconButton size="sm" side="bottom" icon="maximize" label="Fullscreen pane" keys={() => keysFor("view.fullscreen")} onPress={() => {
                menuOpen.value = false;
                if (!props.focused()) selectSession(props.id());
                toggleTiledMaximize();
              }} />
              <IconButton size="sm" side="bottom" icon="close" label="Close" keys={() => keysFor("session.close")} onPress={() => {
                menuOpen.value = false;
                closeSession(props.id());
              }} />
            </Row>
          )}
        </Show>
      </Row>
      <Dynamic
        value={() => docKey(spec())}
        style={{ grow: 1, minWidth: 0, minHeight: 0 }}
      >
        {() => <TileBody id={props.id()} spec={spec()} paintOffset={props.paintOffset} />}
      </Dynamic>
    </PaneFrame>
  );
}
