// The in-place folder view shared by session launches and Add project.
interface ProjectFolder {
  name: string;
  path: string;
  branch?: string | null;
}

interface ProjectFolders {
  path: string;
  parent: string | null;
  ancestors: ProjectFolder[];
  branch?: string | null;
  folders: ProjectFolder[];
}

interface ProjectPlace {
  name: string;
  path: string;
}

let launcherHome: string | undefined;
// XDG user dirs as (kind, path) and GTK bookmarks, read once like Nautilus' sidebar.
let launcherPlaces: { special: ProjectPlace[]; bookmarks: ProjectPlace[] } | undefined;

// Nautilus' folder-<kind> icons for the XDG user dirs.
const LAUNCHER_PLACE_GLYPHS: Record<string, string> = {
  desktop: "monitor", documents: "article", download: "download", music: "music", pictures: "image",
  videos: "video", templates: "file", publicshare: "share", projects: "file-code",
};

function launcherPlaceGlyph(path: string, fallback: string): string {
  if (path === launcherHome) return "home";
  const place = launcherPlaces?.special.find((entry) => entry.path === path);
  return (place && LAUNCHER_PLACE_GLYPHS[place.name]) || fallback;
}

interface LaunchRow {
  key: string;
  kind: "ancestor" | "here" | "folder" | "recent";
  name: string;
  path: string;
  depth: number;
  last: boolean;
  // Children are listed right below (the folder's ancestors and the current folder).
  stem: boolean;
  branch: string;
  detail: string;
  count: number;
}

function launcherPathMode(text: string): boolean {
  return /^(~|\/|\\\\|[A-Za-z]:[\\/])/.test(text);
}

function launcherMatch(filter: string, ...values: string[]): boolean {
  const needle = filter.trim().toLowerCase();
  return !needle || values.some((value) => value.toLowerCase().includes(needle));
}

function BranchBadge(props: { name: () => string; muted?: boolean }): Element {
  return (
    <Row style={{ gap: spacing.unit, align: "center", shrink: 0 }}>
      <Glyph name="git-branch" size={typeScale.tiny} color={() => props.muted ? palette().textMuted : accent().brandText} />
      <Text lines={1} font={{ family: fontFamilies.mono, size: typeScale.tiny }}
        paint={() => ({ color: props.muted ? palette().textMuted : accent().brandText })}>{props.name}</Text>
    </Row>
  );
}

// Tree guide: a vertical line under the parent's icon and a tick into this row; rows are
// contiguous, so the lines join into one tree.
function LauncherTreeGuide(props: { depth: number; last: boolean; stem: boolean }): Element {
  const layout = launcherFolderLayout;
  const line = spacing.hairline * 2;
  const x = (props.depth - 1) * layout.indent + layout.gap + layout.icon / 2;
  const paint = () => ({ background: palette().border });
  const iconBottom = (layout.rowHeight + layout.icon) / 2 + spacing.unit / 2;
  return (
    <Box style={{ width: props.depth * layout.indent, height: layout.rowHeight, shrink: 0 }}>
      {/* Stem from this row's icon down to its first child. */}
      {props.stem ? (
        <Box style={{ position: "absolute", left: x + layout.indent, top: iconBottom, width: line,
          height: layout.rowHeight - iconBottom }} paint={paint} />
      ) : null}
      {props.depth > 0 ? (
        <Box style={{ position: "absolute", left: x, top: 0, width: line,
          height: props.last ? layout.rowHeight / 2 : layout.rowHeight }} paint={paint} />
      ) : null}
      {props.depth > 0 ? (
        <Box style={{ position: "absolute", left: x, top: layout.rowHeight / 2,
          width: props.depth * layout.indent + layout.gap - x - spacing.unit, height: line }} paint={paint} />
      ) : null}
    </Box>
  );
}

// Hover only highlights, as in the launcher palette: the keyboard selection is what the footer
// action uses, so a list re-laid out under a still pointer never changes it.
function LauncherRow(props: { row: () => LaunchRow; selected: () => boolean; onPress: () => void }): Element {
  const layout = launcherFolderLayout;
  const row = props.row;
  const glyph = () => launcherPlaceGlyph(row().path,
    row().kind === "here" ? "folder-open" : row().branch ? "git-branch" : "folder");
  const strong = () => props.selected() || row().kind === "here";
  return (
    <Row style={{ height: layout.rowHeight, padding: layout.rowPadding, align: "center", gap: layout.gap, shrink: 0 }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: props.selected() ? chromeHighlight(true) : 0,
        hover: { background: props.selected() ? chromeHighlight(true) : rowHoverBackground() },
        transitions: [smoothTransition("background", motion.fast)],
      })}>
      <Dynamic value={() => `${row().depth}:${row().last}:${row().stem}`} style={{ shrink: 0 }}>
        {() => <LauncherTreeGuide depth={row().depth} last={row().last} stem={row().stem} />}
      </Dynamic>
      <Glyph name={glyph} size={layout.icon}
        color={() => row().kind === "here" ? accent().brandText : strong() ? palette().textPrimary : palette().textMuted} />
      {/* Read row() only inside sources: a read while mounting would subscribe the parent For. */}
      <Text lines={1} style={{ shrink: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.label }}
        paint={() => ({ color: strong() ? palette().textPrimary : row().kind === "ancestor"
          ? palette().textMuted : palette().textSecondary })}>
        {() => row().kind === "folder" ? `${row().name}/` : row().name}
      </Text>
      <Show when={() => !!row().branch} style={{ shrink: 0 }}>
        {() => <BranchBadge name={() => row().branch} muted={row().kind !== "here"} />}
      </Show>
      <Show when={() => row().count > 0} style={{ shrink: 0 }}>
        {() => <Text font={{ family: fontFamilies.mono, size: typeScale.tiny }}
          paint={() => ({ color: palette().statusOnline })}>{() => `● ${row().count}`}</Text>}
      </Show>
      <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.mono, size: typeScale.tiny }}
        paint={() => ({ color: palette().textMuted })}>{() => row().detail}</Text>
      {/* What Enter / Tab do on the selected row. */}
      <Show when={props.selected} style={{ shrink: 0 }}>
        {() => <Row style={{ gap: spacing.unit, align: "center" }}>
          <Show when={() => row().kind !== "here"}>
            {() => <KbdKey label="Tab" />}
          </Show>
          <KbdKey label="↵" />
        </Row>}
      </Show>
    </Row>
  );
}

function LauncherPlace(props: { name: Source<string>; glyph: string; count: Source<number>;
  active: () => boolean; onPress: () => void }): Element {
  const layout = launcherFolderLayout;
  return (
    <Row style={{ height: layout.placeHeight, padding: [0, spacing.unit * 2, 0, spacing.unit * 2],
      gap: layout.gap, align: "center", shrink: 0 }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: props.active() ? chromeHighlight(true) : 0,
        hover: { background: props.active() ? chromeHighlight(true) : rowHoverBackground() },
        transitions: [smoothTransition("background", motion.fast)],
      })}>
      <Glyph name={props.glyph} size={layout.icon}
        color={() => props.active() ? palette().textPrimary : palette().textMuted} />
      <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.caption }}
        paint={() => ({ color: props.active() ? palette().textPrimary : palette().textSecondary })}>{props.name}</Text>
      <Show when={() => read(props.count) > 0} style={{ shrink: 0 }}>
        {() => <Text font={{ family: fontFamilies.mono, size: typeScale.tiny }}
          paint={() => ({ color: palette().statusOnline })}>{() => `● ${read(props.count)}`}</Text>}
      </Show>
    </Row>
  );
}

function LauncherFolders(props: { cwd: string; target: LauncherTarget; back: () => void; canGoBack: boolean;
  onKeys: (handler: (action: string) => void) => void }): Element {
  const layout = launcherFolderLayout;
  const target = props.target;
  const actionLabel = target.kind === "project" ? "Add project"
    : target.cliId ? `Launch ${target.label}` : "Open Terminal";
  const error = signal("");
  const invalidDirectory = signal(false);
  let field: Field | undefined;
  let listId: number | undefined;

  const run = <T,>(action: () => T): T | undefined => {
    error.value = "";
    try {
      return action();
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
    }
  };
  const browseHost = (path: string, base?: string): ProjectFolders =>
    JSON.parse(jsnirRequest(JSON.stringify({ type: "browseProject", path, base })));
  if (launcherHome === undefined) {
    try { launcherHome = browseHost("~").path; } catch (_) { launcherHome = ""; }
  }
  if (launcherPlaces === undefined) {
    try { launcherPlaces = JSON.parse(jsnirRequest(JSON.stringify({ type: "projectPlaces" }))); } catch (_) { /* no places */ }
    launcherPlaces ??= { special: [], bookmarks: [] };
  }
  const tilde = (path: string) => launcherHome && (path === launcherHome || path.startsWith(launcherHome + "/"))
    ? "~" + path.slice(launcherHome.length) : path;

  // A query starting like a path ("/", "~", "C:\\") browses: everything up to the last
  // separator is the folder, the rest filters it. Any other text filters the folder and recents.
  const asDir = (path: string) => /[\\/]$/.test(path) ? path : path + "/";
  const base = run(() => browseHost(props.cwd)) ?? null;
  const query = signal(base?.path ? asDir(base.path) : props.cwd ? asDir(props.cwd) : "");
  const location = signal<ProjectFolders | null>(base);
  let browsedDir = base?.path ? asDir(base.path) : "";
  const browseBase = () => location.value?.path ?? base?.path;
  const split = (text: string): [string, string] => {
    const cut = Math.max(text.lastIndexOf("/"), text.lastIndexOf("\\"));
    if (cut < 0) return [text === "~" ? "~/" : text, ""];
    return [text.slice(0, cut + 1), text.slice(cut + 1)];
  };
  effect(() => {
    const text = query.value;
    if (!launcherPathMode(text)) {
      browsedDir = "";
      invalidDirectory.value = false;
      return;
    }
    const [dir] = split(text);
    if (dir === browsedDir) return;
    browsedDir = dir;
    const found = run(() => browseHost(dir, browseBase()));
    invalidDirectory.value = !found;
    if (found) location.value = found;
  });
  const filter = () => launcherPathMode(query.value) ? split(query.value)[1] : query.value.trim();

  const recentRows = signal<LaunchRow[]>([]);
  const liveCounts = signal(new Map<string, number>());
  effect(() => {
    const paths = new Set(configState.value.projects);
    const counts = new Map<string, number>();
    for (const session of sessionsState.value.items) {
      paths.add(session.cwd);
      if (session.alive) counts.set(session.cwd, (counts.get(session.cwd) ?? 0) + 1);
    }
    liveCounts.value = counts;
    recentRows.value = Array.from(paths, (path) => ({
      key: "recent:" + path, kind: "recent", name: basename(path) || path, path,
      depth: 0, last: true, stem: false, branch: "", detail: tilde(path), count: counts.get(path) ?? 0,
    }));
  });
  // The current folder under its ancestors (from ~ when inside home), then its subfolders.
  const buildRows = (): LaunchRow[] => {
    const here = location.value;
    const text = filter();
    const out: LaunchRow[] = launcherPathMode(query.value) || !text ? []
      : recentRows.value.filter((row) => launcherMatch(text, row.name, row.path));
    if (!here || invalidDirectory.value) return out;
    const counts = liveCounts.value;
    let chain = [...here.ancestors, { name: basename(here.path) || here.path, path: here.path }];
    const homeAt = chain.findIndex((level) => level.path === launcherHome);
    if (homeAt >= 0) chain = [{ name: "~", path: chain[homeAt].path }, ...chain.slice(homeAt + 1)];
    chain = chain.slice(-layout.treeDepth);
    // Nautilus hides dotfolders until you ask for them.
    const children = here.folders.filter((folder) =>
      (!folder.name.startsWith(".") || text.startsWith(".")) && launcherMatch(text, folder.name));
    chain.forEach((level, depth) => {
      const isHere = depth === chain.length - 1;
      out.push({ key: (isHere ? "here:" : "up:") + level.path, kind: isHere ? "here" : "ancestor",
        name: level.name, path: level.path, depth, last: true, stem: !isHere || children.length > 0,
        branch: isHere ? here.branch ?? "" : "", detail: "", count: isHere ? counts.get(level.path) ?? 0 : 0 });
    });
    children.forEach((folder, index) => out.push({
      key: "folder:" + folder.path, kind: "folder", name: folder.name, path: folder.path,
      depth: chain.length, last: index === children.length - 1, stem: false, branch: folder.branch ?? "",
      detail: "", count: counts.get(folder.path) ?? 0,
    }));
    return out;
  };
  const folderRows = signal<LaunchRow[]>([]);
  effect(() => { folderRows.value = buildRows(); });
  const rows = () => folderRows.value;
  const cursor = signal<string | null>(null);
  effect(() => { void query.value; void location.value; cursor.value = null; });
  // An explicit path selects only an exact match. A fuzzy filter can select its first result.
  const current = (): LaunchRow | undefined => {
    const list = rows();
    const text = filter();
    return list.find((row) => row.key === cursor.value) ?? (text
      ? list.find((row) => (row.kind === "folder" || row.kind === "recent") &&
        (!launcherPathMode(query.value) || row.name === text))
      : list.find((row) => row.kind === "here"));
  };
  const selectedKey = signal<string | undefined>(undefined);
  effect(() => { selectedKey.value = current()?.key; });

  const setQuery = (text: string) => {
    field?.replace(text);
    query.value = text;
  };
  const open = (row: LaunchRow | undefined) => {
    if (row && row.kind !== "here") setQuery(asDir(row.path));
  };
  const choose = (row = current()) => {
    const cwd = row?.path ?? run(() => browseHost(query.value, browseBase()))?.path;
    if (!cwd) return;
    const request = target.kind === "project" ? { type: "addProject", path: cwd }
      : target.cliId ? { type: "launchCli", cliId: target.cliId, cwd } : { type: "newSession", cwd };
    if (run(() => jsnirRequest(JSON.stringify(request))) === undefined) return;
    if (target.kind === "session") ++documentIntent;
    if (overviewOpen2.value) closeOverviewMode();
    activeNav.value = "agents";
    closePalette();
  };
  const move = (delta: number) => {
    const list = rows();
    if (!list.length) return;
    const at = list.findIndex((row) => row.key === current()?.key);
    const index = at < 0 ? (delta > 0 ? 0 : list.length - 1) : (at + delta + list.length) % list.length;
    cursor.value = list[index].key;
    if (listId === undefined) return;
    const visible = jsnir.columns.clientHeight[listId] ?? 0;
    scrollTo(listId, { offset: Math.max(0, index * layout.rowHeight - (visible - layout.rowHeight) / 2), smooth: true });
  };
  props.onKeys((action) => {
    if (action === "up") move(-1);
    else if (action === "down") move(1);
    else if (action === "tab") open(current() ?? rows().find((row) => row.kind === "folder"));
    else if (action === "enter") choose();
  });
  const goTo = (path: string) => setQuery(asDir(path));

  // Left column, as in Nautilus: home and the XDG folders, GTK bookmarks, then recent projects.
  const placesColumn = () => {
    const places = launcherPlaces!;
    const home = launcherHome ? [{ key: "home", name: "Home", path: launcherHome, glyph: "home" }] : [];
    const special = places.special.map((place) => ({ key: "xdg:" + place.path, name: basename(place.path),
      path: place.path, glyph: LAUNCHER_PLACE_GLYPHS[place.name] ?? "folder" }));
    const marks = places.bookmarks.map((place) => ({ key: "mark:" + place.path, name: place.name,
      path: place.path, glyph: "bookmark" }));
    const groups = [
      { title: "Places", items: [...home, ...special] },
      { title: "Bookmarks", items: marks },
    ].filter((group) => group.items.length > 0);
    return (
      <Column style={{ width: layout.placesWidth, shrink: 0, scroll: true, clip: true,
        padding: [layout.listInset, layout.listInset, layout.listInset, layout.listInset] }}
        paint={() => ({ scrollbar: editorScrollbar(), ...launcherEdge([0, 1, 0, 0])() })}>
        {groups.map((group) => (
          <Column style={{ shrink: 0 }}>
            <LauncherHeading label={group.title} />
            {group.items.map((item) => <LauncherPlace name={item.name} glyph={item.glyph}
              count={0} active={() => location.value?.path === item.path} onPress={() => goTo(item.path)} />)}
          </Column>
        ))}
        <Show when={() => recentRows.value.length > 0} style={{ shrink: 0 }}>
          {() => (
            <Column>
              <LauncherHeading label="Recent projects" />
              <For each={recentRows} key={(row) => row.key} style={{ shrink: 0 }}>
                {(row) => <LauncherPlace name={() => row().name} glyph="clock" count={() => row().count}
                  active={() => location.value?.path === row().path} onPress={() => goTo(row().path)} />}
              </For>
            </Column>
          )}
        </Show>
      </Column>
    );
  };

  return (
    <Column style={{ grow: 1, minHeight: 0, gap: floatingLayout.gap }}>
      <LauncherSearchBar>
        {props.canGoBack ? <IconButton icon="arrow-left" label="Back to launcher" side="bottom"
          layer={launcherLayout.layer + 2} size="sm" onPress={props.back} /> :
          <Glyph name={target.kind === "project" ? "folder-plus" : "folder"} size={layout.icon} color={() => palette().textMuted} />}
        <TextInput
          value={query.value}
          ref={(input) => { field = input; }}
          autoFocus
          retainOnSubmit
          style={{ grow: 1, minWidth: 0, height: spacing.control }}
          font={{ family: fontFamilies.mono, size: typeScale.body }}
          placeholder="Filter folders or type a path…"
          placeholderColor={palette().textMuted}
          onChange={(text) => { error.value = ""; query.value = text; }}
          onSubmit={() => choose()}
          paint={() => ({ color: palette().textPrimary })}
        />
        <IconButton icon="arrow-up" label="Parent folder" side="bottom" layer={launcherLayout.layer + 2} size="sm"
          disabled={() => !location.value?.parent}
          onPress={() => { const up = location.value?.parent; if (up) goTo(up); }} />
        <IconButton icon="home" label="Home folder" side="bottom" layer={launcherLayout.layer + 2} size="sm"
          onPress={() => goTo(launcherHome || "~")} />
      </LauncherSearchBar>
      {/* Folders and the confirm bar share one floating panel under the search bar. */}
      <Column style={{ grow: 1, minHeight: 0, clip: true }} onPress={() => {}}
        paint={() => ({ ...floatingSurface(true), ...accordeeMenuMotion("top") })}>
      <Row style={{ grow: 1, minHeight: 0 }}>
        <Show when={() => launcherWidth() >= layout.compactAt} style={{ shrink: 0 }}>
          {placesColumn}
        </Show>
        {/* Folder tree */}
        <Column ref={(node) => { listId = node; }}
          style={{ grow: 1, minWidth: 0, minHeight: 0, scroll: true, clip: true,
            padding: [layout.listInset, layout.listInset, layout.listInset, layout.listInset] }}
          paint={() => ({ scrollbar: editorScrollbar() })}>
          <For each={rows} key={(row) => row.key} style={{ shrink: 0 }}>
            {(row) => <LauncherRow row={row}
              selected={() => selectedKey.value === row().key}
              onPress={() => (row().kind === "here" ? choose(row()) : open(row()))} />}
          </For>
          <Show when={() => !invalidDirectory.value && !filter() && location.value?.folders.length === 0}>
            {() => <Text style={{ padding: layout.rowPadding }} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
              paint={() => ({ color: palette().textMuted })}>No subfolders</Text>}
          </Show>
          <Show when={() => !!filter() && !current() && !error.value}>
            {() => <EmptyState compact title="No folder matches" style={{ padding: layout.gap }} />}
          </Show>
        </Column>
      </Row>
      <LauncherFooter>
        <Box style={{ grow: 1, minWidth: 0, clip: true }}>
          <Dynamic value={() => !!error.value}>
            {(hasError) => hasError ? <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
              paint={() => ({ color: palette().destructive })}>{() => error.value}</Text> : (
              <Row style={{ align: "center", gap: launcherLayout.rowInset }}>
                <PaletteHint keys={["Esc"]} label={props.canGoBack ? "Back" : "Close"} onPress={props.back} />
                <Show when={() => launcherWidth() >= launcherLayout.narrowHintsAt}>
                  {() => <PaletteHint keys={["↑", "↓"]} label="Navigate" />}
                </Show>
                <PaletteHint keys={["Tab"]} label="Browse" />
              </Row>
            )}
          </Dynamic>
        </Box>
        <KbdKey label="↵" />
        <PrimaryButton label={actionLabel} onPress={() => choose()} />
      </LauncherFooter>
      </Column>
    </Column>
  );
}
