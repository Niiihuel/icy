// Accordee sidebar: expanded (search, nav, sessions, usage, account) and the compact
// icon rail with right-side tooltips. Both share the same menus.

// Live from the keymap, so tooltips follow rebinding in Settings.
const KEYS_NEW_SESSION = (): string[] => keysFor("session.new");

function activeSession(): SessionItem | undefined {
  return sessionsState.value.items.find((s) => s.id === sessionsState.value.activeId);
}

function accountName(): string {
  return githubState.value.username ? githubState.value.username : "Local user";
}

// Second line under the account name: the repository icy is in and how GitHub is reached.
function accountSummary(): string {
  const gh = githubState.value;
  if (!gh.connected && !gh.sshGithubReady) return "GitHub not connected";
  const via = gh.sshGithubReady ? "SSH" : gh.method;
  return [gh.repoSlug, gh.branch, via].filter((part) => !!part).join(" · ");
}

// Account menu (bottom avatar row): who is signed in, then app-level places and actions.
function AccountMenu(props: {
  open: Signal<boolean>;
  anchor: () => number | undefined;
  side: Side;
}): Element {
  const close = () => {
    props.open.value = false;
  };
  const go = (run: () => void) => () => {
    close();
    run();
  };
  return (
    <MenuPanel open={props.open} anchor={props.anchor} side={props.side} align="end">
      <Row style={{ padding: [spacing.unit * 1.5, spacing.unit * 2, spacing.unit * 1.5, spacing.unit * 2], align: "center", gap: spacing.unit * 2.5 }}>
        <AccountAvatar />
        <Column style={{ grow: 1, minWidth: 0, gap: 1 }}>
          <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
            paint={() => ({ color: palette().textPrimary })}>{accountName}</Text>
          <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
            paint={() => ({ color: palette().textMuted })}>{accountSummary}</Text>
        </Column>
      </Row>
      <MenuSeparator />
      <MenuItem icon="settings" label="Settings" onPress={go(() => { activeNav.value = "settings"; })} />
      <MenuItem icon="palette" label="Themes" onPress={go(() => {
        settingsTab.value = "appearance";
        activeNav.value = "settings";
      })} />
      <ReleaseNotesMenuItem label="Release notes" onPress={go(() => { releaseNotesOpen.value = true; })} />
      <MenuSeparator />
      <MenuItem icon="refresh" label="Refresh GitHub status" onPress={go(refreshGitHub)} />
    </MenuPanel>
  );
}

function SidebarNavRow(props: {
  icon?: string;
  image?: string;
  label: Source<string>;
  active?: Source<boolean>;
  onPress?: () => void;
  right?: Element;
}): Element {
  const isActive = () => read(props.active ?? false);
  return (
    <Row
      style={() => ({
        height: rowControlHeight(),
        padding: [0, 8, 0, 8],
        align: "center",
        justify: "between",
        gap: 8,
        shrink: 0,
      })}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: isActive() ? palette().sidebarActive : 0,
        hover: props.onPress ? { background: palette().sidebarActive } : undefined,
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Row style={{ align: "center", gap: 8, grow: 1, minWidth: 0 }}>
        {props.image ? (
          <Image source={props.image} style={{ width: 16, height: 16, shrink: 0 }} />
        ) : props.icon ? (
          <Glyph
            name={props.icon}
            size={16}
            color={() => (isActive() ? palette().textPrimary : palette().textSecondary)}
          />
        ) : null}
        <Text
          lines={1}
          font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.normal }}
          paint={() => ({
            color: isActive() ? palette().textPrimary : palette().textSubtle,
          })}
        >
          {props.label}
        </Text>
      </Row>
      {props.right ?? null}
    </Row>
  );
}

interface SidebarCli {
  id: string;
  name: string;
  usage?: CliUsageItem;
}

function sidebarClis(): SidebarCli[] {
  const tracked = cliUsageState.value;
  const ids = new Set(tracked.map((cli) => cli.id));
  return [
    ...tracked.map((cli) => ({
      id: cli.id,
      name: configState.value.clis.find((c) => c.id === cli.id)?.name ?? CLI_NAMES[cli.id] ?? cli.id,
      usage: cli,
    })),
    ...configState.value.clis
      .filter((cli) => cli.enabled && !ids.has(cli.id))
      .map((cli) => ({ id: cli.id, name: cli.name })),
  ];
}

// Display names for tracked CLIs that are not in the config.
const CLI_NAMES: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  antigravity: "Antigravity",
  grok: "Grok CLI",
  opencode: "OpenCode",
  pi: "Pi",
};

// Ring and bar colour by how close a limit is to running out.
function usageLevelColor(percent: number, calm: number): number {
  const layout = usageTooltipLayout;
  return percent >= layout.criticalAt ? palette().destructive
    : percent >= layout.warnAt ? palette().warningText : calm;
}

function resetsIn(at: number): string {
  const left = Math.max(0, at - Math.floor(Date.now() / 1000));
  const days = Math.floor(left / 86400);
  const hours = Math.floor((left % 86400) / 3600);
  const minutes = Math.ceil((left % 3600) / 60);
  if (left === 0) return "Resets now";
  if (days > 0) return `Resets in ${days}d ${hours}h`;
  return hours > 0 ? `Resets in ${hours}h ${minutes}m` : `Resets in ${minutes}m`;
}

function UsageTooltipRow(props: { row: UsageRowView }): Element {
  const layout = usageTooltipLayout;
  const row = props.row;
  const text = (value: string, color: () => number) => (
    <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
      paint={() => ({ color: color() })}>{value}</Text>
  );
  return (
    <Column style={{ gap: spacing.unit, shrink: 0 }}>
      <Row style={{ align: "center", gap: spacing.unit * 3 }}>
        <Box style={{ grow: 1, minWidth: 0 }}>{text(row.label, () => palette().tooltipFg)}</Box>
        {row.value ? text(row.value, () => palette().tooltipMuted) : null}
      </Row>
      {row.percent !== null ? (
        <Box style={{ height: layout.barHeight, shrink: 0 }}
          paint={() => ({ radius: layout.barHeight / 2, background: palette().tooltipKbdBg })}>
          <Box style={{ width: Math.max(layout.barHeight, (layout.width * (row.percent ?? 0)) / 100), height: layout.barHeight }}
            paint={() => ({ radius: layout.barHeight / 2,
              background: usageLevelColor(row.percent ?? 0, palette().tooltipFg) })} />
        </Box>
      ) : null}
      {row.resetsAt !== null ? (
        <Text font={{ family: fontFamilies.sans, size: typeScale.tiny }}
          paint={() => ({ color: palette().tooltipMuted })}>{resetsIn(row.resetsAt)}</Text>
      ) : null}
    </Column>
  );
}

// Every limit and model the CLI reports, one section per group with a hairline between them.
function UsageTooltipBody(props: { usage: CliUsageItem }): Element {
  const layout = usageTooltipLayout;
  const sections = props.usage.sections;
  if (sections.length === 0) {
    return (
      <Box style={{ width: layout.width, padding: [spacing.unit, 0, 0, 0] }}>
        <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
          paint={() => ({ color: palette().tooltipMuted })}>No local usage yet</Text>
      </Box>
    );
  }
  return (
    <Column style={{ width: layout.width, gap: layout.sectionGap }}>
      {sections.map((section, index) => (
        <Column style={{ gap: layout.rowGap }}>
          {index > 0 ? (
            <Box style={{ height: 1, margin: [0, 0, spacing.unit, 0] }}
              paint={() => ({ background: palette().tooltipKbdBg })} />
          ) : null}
          <Text font={{ family: fontFamilies.sans, size: typeScale.tiny, weight: fontWeights.medium, letterSpacing: 0.04 }}
            paint={() => ({ color: palette().tooltipMuted })}>{section.title.toUpperCase()}</Text>
          {section.rows.map((row) => <UsageTooltipRow row={row} />)}
        </Column>
      ))}
    </Column>
  );
}

// Tracked CLIs show their usage in a rich tooltip; the corner ring appears only for real limits.
function CliUsageChip(props: { cli: () => SidebarCli }): Element {
  const pct = () => Math.max(0, Math.min(100, props.cli().usage?.percent ?? 0));
  const limited = () => props.cli().usage?.percent != null;
  const step = () => Math.round(pct() / 5);
  const detail = () => (limited() ? `${Math.round(pct())}% used` : "");
  const layout = cliUsageLayout;
  return (
    <Dynamic value={() => !!props.cli().usage} style={{ shrink: 0 }}>
      {(tracked) => (
        <HintTooltip side="top" label={() => props.cli().name} detail={tracked ? detail : undefined}
          body={tracked ? () => <UsageTooltipBody usage={props.cli().usage as CliUsageItem} /> : undefined}>
          <Box
            style={{ width: layout.chipSize, height: layout.chipSize, align: "center", justify: "center" }}
            onPress={() => launchCli(props.cli().id)}
            paint={() => ({
              radius: radii.md,
              hover: { background: palette().sidebarActive },
              active: { scale: controlState.iconPressScale },
              transitions: [
                smoothTransition("background", motion.fast),
                smoothTransition("scale", motion.fast),
              ],
            })}
          >
            {/* Every id resolves to a logo; unknown CLIs get the host's terminal fallback. */}
            <Image source={() => "cli:" + props.cli().id} style={{ width: layout.iconSize, height: layout.iconSize }} />
            <Show when={limited} style={{ position: "absolute", left: layout.badgeLeft, top: layout.badgeTop }}>
              {() => (
                <Box
                  style={{ width: layout.badgeSize, height: layout.badgeSize }}
                  paint={() => ({ radius: layout.badgeSize / 2, background: palette().sidebarBg })}
                >
                  <Image
                    source="glyph:uring-track"
                    style={{ position: "absolute", left: 0, top: 0, width: layout.badgeSize, height: layout.badgeSize }}
                    paint={() => ({ color: palette().kbdBg })}
                  />
                  <Show when={() => step() > 0}>
                    {() => (
                      <Image
                        source={() => "glyph:uring-" + step()}
                        style={{ position: "absolute", left: 0, top: 0, width: layout.badgeSize, height: layout.badgeSize }}
                        paint={() => ({ color: usageLevelColor(pct(), palette().statusOnline) })}
                      />
                    )}
                  </Show>
                </Box>
              )}
            </Show>
          </Box>
        </HintTooltip>
      )}
    </Dynamic>
  );
}

function CliUsageStrip(): Element {
  const layout = cliUsageLayout;
  const contentWidth = () => Math.max(0, sidebarClis().length * (layout.chipSize + layout.gap) - layout.gap);
  return (
    <Column
      style={{ padding: layout.padding, margin: [spacing.unit, 0, 0, 0], shrink: 0 }}
      paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [1, 0, 0, 0] })}
    >
      <HorizontalStrip contentWidth={contentWidth} label="CLIs" stretch>
        <For each={sidebarClis} key={(cli) => cli.id}
          style={{ direction: "row", align: "center", justify: "between", gap: layout.gap, grow: 1 }}>
          {(cli) => <CliUsageChip cli={cli} />}
        </For>
      </HorizontalStrip>
    </Column>
  );
}

type SessionFilter = "all" | "idle" | "working";
const sessionFilter = signal<SessionFilter>("all");
const agentSort = signal<"priority" | "project">("priority");
function handleAgentDigit(digit: number): void {
  const item = sidebarAgentRows.value[digit - 1];
  if (item) focusTile(item.id);
}

type AgentCategory = "blocked" | "done" | "working" | "idle" | "exited";
const agentSections: ReadonlyArray<{ id: AgentCategory; title: string }> = [
  { id: "blocked", title: "Needs you" }, { id: "done", title: "Done" },
  { id: "working", title: "Working" }, { id: "idle", title: "Idle" }, { id: "exited", title: "Exited" },
];

function agentCategory(item: SessionItem): AgentCategory {
  if (!item.alive) return "exited";
  if (item.status === "blocked" || item.status === "failed") return "blocked";
  if (item.status === "done") return "done";
  return sessionWorking(item) ? "working" : "idle";
}

function agentStatusColor(category: AgentCategory): number {
  if (category === "blocked" || category === "working") return palette().warningText;
  if (category === "done") return palette().successText;
  return palette().textMuted;
}

function matchesSessionFilter(item: SessionItem, filter: SessionFilter): boolean {
  if (filter === "all") return true;
  if (filter === "working") return sessionWorking(item);
  return item.alive && (item.status ? item.status === "idle" : !sessionWorking(item));
}

interface ProjectGroup {
  path: string;
  name: string;
  pinned: boolean;
  sessions: SessionItem[];
}

// Keep pinned folders visible even when the current filters leave them empty.
function projectGroups(): ProjectGroup[] {
  const groups = new Map<string, ProjectGroup>();
  for (const path of configState.value.projects) {
    groups.set(path, { path, name: basename(path) || path, pinned: true, sessions: [] });
  }
  const filter = sessionFilter.value;
  for (const s of sessionsState.value.items) {
    if (!matchesSessionFilter(s, filter)) continue;
    let g = groups.get(s.cwd);
    if (!g) {
      g = { path: s.cwd, name: basename(s.cwd) || s.cwd, pinned: false, sessions: [] };
      groups.set(s.cwd, g);
    }
    g.sessions.push(s);
  }
  return Array.from(groups.values());
}

const priorityAgentGroups = signal<Array<{ id: AgentCategory; title: string; sessions: SessionItem[] }>>([]);
const sidebarAgentRows = signal<SessionItem[]>([]);
effect(() => {
  const sections = agentSections.map((section) => ({ ...section, sessions: [] as SessionItem[] }));
  const byStatus = new Map(sections.map((section) => [section.id, section.sessions]));
  for (const item of sessionsState.value.items) {
    if (matchesSessionFilter(item, sessionFilter.value)) byStatus.get(agentCategory(item))?.push(item);
  }
  const groups = sections.filter((group) => group.sessions.length > 0);
  priorityAgentGroups.value = groups;
  sidebarAgentRows.value = (agentSort.value === "priority" ? groups : projectGroups()).flatMap((group) => group.sessions);
});

function agentShortcutCount(): number {
  if (activeNav.value !== "agents" || overviewOpen2.value || imageViewer.value
    || paletteRequest.value || releaseNotesOpen.value || restartConfirmOpen.value) return 0;
  return Math.min(9, sidebarAgentRows.value.length);
}

function AgentsHeader(): Element {
  const layout = agentListLayout;
  const menu = signal(false);
  let anchor: number | undefined;
  return (
    <Row style={{ height: layout.headerHeight, padding: [0, layout.padding, 0, layout.padding], align: "center", justify: "between", shrink: 0 }}>
      <Text font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textMuted })}>Agents</Text>
      <Row style={{ align: "center", shrink: 0 }}>
      <IconButton icon="folder-plus" label="Add project folder" size="sm" onPress={() => showProjectDialog("add")} />
      <Box ref={(id) => { anchor = id; }} style={{ shrink: 0 }}>
        <IconButton icon={() => agentSort.value === "priority" ? "clock" : "folder-open"} label="Sort agents" size="sm"
          highlighted={menu} onPress={() => { menu.value = !menu.value; }} />
        <MenuPanel open={menu} anchor={() => anchor} side="bottom">
          <MenuLabel label="Sort agents" />
          <MenuItem icon="clock" label="Priority" checked={() => agentSort.value === "priority"}
            onPress={() => { agentSort.value = "priority"; menu.value = false; }} />
          <MenuItem icon="folder-open" label="Grouped by project" checked={() => agentSort.value === "project"}
            onPress={() => { agentSort.value = "project"; menu.value = false; }} />
          <MenuSeparator />
          <MenuLabel label="Show" />
          {([['all', 'All sessions'], ['working', 'Working only'], ['idle', 'Idle only']] as const).map(([filter, label]) =>
            <MenuItem label={label} checked={() => sessionFilter.value === filter}
              onPress={() => { sessionFilter.value = filter; menu.value = false; }} />)}
        </MenuPanel>
      </Box>
      </Row>
    </Row>
  );
}

function ActiveSessionActions(props: { item: () => SessionItem; open: Signal<boolean> }): Element {
  const action = (run: () => void) => () => {
    props.open.value = false;
    run();
  };
  return (
    <Column>
      <MenuItem icon="square-terminal" label="Focus session" onPress={action(() => focusTile(props.item().id))} />
      <MenuItem icon="folder-open" label="Show project folder" onPress={action(() => revealPath(props.item().cwd))} />
      <MenuSeparator />
      <MenuItem icon="archive" label="Archive session"
        onPress={action(() => requestHost({ type: "archiveSession", id: props.item().id }))} />
    </Column>
  );
}

function SessionRow(props: { item: () => SessionItem; withinProject?: boolean }): Element {
  const layout = agentListLayout;
  const hovered = signal(false);
  const menu = signal(false);
  const isActive = () => activeNav.value === "agents" && sessionsState.value.activeId === props.item().id;
  const category = () => agentCategory(props.item());
  const title = () => cleanTitle(props.item().title) || cliDisplayName(props.item().cli);
  const project = () => basename(props.item().cwd) || props.item().cwd;
  const label = () => props.withinProject ? title() : `${project()} · ${title()}`;
  const status = () => props.item().alive && props.item().status === "failed" ? "Failed"
    : agentSections.find((section) => section.id === category())?.title ?? "Idle";
  const color = () => status() === "Failed" ? palette().destructive : agentStatusColor(category());
  return (
    <Row style={{ height: layout.rowHeight, padding: [0, layout.padding, 0, layout.padding],
      align: "center", gap: spacing.unit, shrink: 0 }}
      onPress={() => focusTile(props.item().id)}
      onHover={(inside) => { hovered.value = inside; }}
      onContextMenu={() => { menu.value = true; }}
      paint={() => ({
        radius: radii.lg,
        background: isActive() || hovered.value || menu.value ? palette().sidebarActive : 0,
        a11y: { label: `${label()}, ${status()}` },
        transitions: [smoothTransition("background", motion.fast)],
      })}>
      <HintTooltip label={title} detail={status} side="right" disabled={menu} style={{ grow: 1, minWidth: 0 }}>
        <Row style={{ align: "center", gap: layout.gap, minWidth: 0 }}>
          <Image source={() => `cli:${props.item().cli}`} style={{ width: layout.icon, height: layout.icon, shrink: 0 }} />
          <FadeText text={label} font={{ family: fontFamilies.sans, size: typeScale.label }}
            background={() => isActive() || hovered.value || menu.value ? palette().sidebarActive : palette().sidebarBg}
            spans={() => props.withinProject ? [{ text: label(), color: isActive() ? palette().textPrimary : palette().textSecondary }]
              : [{ text: project(), color: isActive() ? palette().textPrimary : palette().textSecondary },
                  { text: ` · ${title()}`, color: palette().textMuted }]} />
          <Box style={{ width: layout.statusSize, height: layout.statusSize, align: "center", justify: "center", shrink: 0 }}>
            <Dynamic value={category}>
              {(kind) => kind === "blocked" ? <Glyph name="alert" size={layout.statusSize} color={color} />
                : kind === "working" ? <Glyph name={() => `agent-working-${agentSpinnerTick.value % layout.spinnerFrames}`}
                  size={layout.statusSize} color={color} />
                : <Box style={{ width: layout.dot, height: layout.dot }} paint={() => ({
                  radius: layout.dot / 2,
                  background: kind === "done" || kind === "exited" ? color() : 0,
                  opacity: kind === "exited" ? workspaceTabLayout.dimOpacity : 1,
                  edge: kind === "idle" ? color() : 0, edgeWidth: spacing.hairline * 2,
                })} />}
            </Dynamic>
          </Box>
        </Row>
      </HintTooltip>
      <SessionMenuButton open={menu} visible={hovered} side="right">
        <ActiveSessionActions item={props.item} open={menu} />
      </SessionMenuButton>
    </Row>
  );
}

function PriorityAgents(): Element {
  const layout = agentListLayout;
  return <Column style={{ gap: spacing.unit }}>
    <Show when={() => priorityAgentGroups.value.length === 0}>
      {() => <EmptyState compact title={() => sessionFilter.value === "all" ? "No sessions yet" : "No matching sessions"}
        style={{ padding: [0, layout.padding, 0, layout.padding] }} />}
    </Show>
    <For each={priorityAgentGroups} key={(group) => group.id}>
      {(group) => <Column>
        <Row style={{ height: layout.sectionHeight, padding: [0, layout.padding, 0, layout.padding], gap: layout.gap, align: "center" }}>
          <Text font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
            paint={() => ({ color: agentStatusColor(group().id) })}>{() => group().title}</Text>
          <Text font={{ family: fontFamilies.mono, size: typeScale.tiny }}
            paint={() => ({ color: palette().textMuted })}>{() => String(group().sessions.length)}</Text>
        </Row>
        <For each={() => group().sessions} key={(item) => item.id} style={{ gap: layout.rowGap }}>
          {(item) => <SessionRow item={item} />}
        </For>
      </Column>}
    </For>
  </Column>;
}

// Antigravity project folder row: folder icon + name; on hover "⋮" (menu) and "+" (new session).
function ProjectHeader(props: { group: () => ProjectGroup }): Element {
  const hovered = signal(false);
  const menuOpen = signal(false);
  let menuAnchor: number | undefined;
  const showActions = () => hovered.value || menuOpen.value;
  // The project holding the focused session reads brightest, like the active tab.
  const current = () => props.group().sessions.some((s) => s.id === sessionsState.value.activeId);
  return (
    <Row
      style={{ height: 30, padding: [0, 4, 0, 8], align: "center", gap: 8, shrink: 0 }}
      onHover={(inside) => {
        hovered.value = inside;
      }}
      paint={() => ({
        radius: radii.md,
        background: hovered.value ? palette().sidebarActive : 0,
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Row
        style={{ grow: 1, minWidth: 0, align: "center", gap: spacing.unit * 2 }}
        onPress={() => newSessionIn(props.group().path)}
      >
        <Glyph name="folder-open" size={16}
          color={() => (current() || hovered.value ? palette().textSecondary : palette().textMuted)} />
        <Text lines={1} style={{ grow: 1, minWidth: 0 }}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          paint={() => ({
            color: current() ? palette().textPrimary : hovered.value ? palette().textSecondary : palette().textMuted,
            transitions: [smoothTransition("color", motion.fast)],
          })}>
          {() => props.group().name}
        </Text>
      </Row>
      <Row
        style={{ align: "center", gap: 2, shrink: 0 }}
        paint={() => ({ opacity: showActions() ? 1 : 0, transitions: [smoothTransition("opacity", motion.fast)] })}
      >
        <Box
          ref={(id) => {
            menuAnchor = id;
          }}
          style={{ width: 24, height: 24, align: "center", justify: "center" }}
          onPress={() => {
            menuOpen.value = !menuOpen.value;
          }}
          paint={() => ({
            radius: radii.md,
            background: menuOpen.value ? palette().kbdBg : 0,
            hover: { background: palette().kbdBg },
          })}
        >
          <Glyph name="more-vertical" size={14} color={() => palette().textSecondary} />
        </Box>
        <HintTooltip label="New session here" side="right">
          <Box
            style={{ width: 24, height: 24, align: "center", justify: "center" }}
            onPress={() => newSessionIn(props.group().path)}
            paint={() => ({ radius: radii.md, hover: { background: palette().kbdBg } })}
          >
            <Glyph name="plus" size={14} color={() => palette().textSecondary} />
          </Box>
        </HintTooltip>
      </Row>
      <MenuPanel open={menuOpen} anchor={() => menuAnchor} side="bottom" align="start" width={228}>
        <Show when={() => orderedEditors().length > 0}>
          {() => (
            <Column style={{ gap: 1 }}>
              <MenuLabel label="Open in" />
              <OpenInEditorItems
                path={() => props.group().path}
                cwd={() => props.group().path}
                onSelect={() => {
                  menuOpen.value = false;
                }}
              />
              <MenuSeparator />
            </Column>
          )}
        </Show>
        <MenuItem
          icon="folder-open"
          label="Show in file manager"
          onPress={() => {
            menuOpen.value = false;
            revealPath(props.group().path);
          }}
        />
        <MenuItem
          icon="copy"
          label="Copy folder path"
          onPress={() => {
            menuOpen.value = false;
            copyText(props.group().path);
          }}
        />
        <MenuSeparator />
        <Dynamic value={() => props.group().pinned}>
          {(pinned) =>
            pinned ? (
              <MenuItem
                icon="trash"
                label="Remove project"
                onPress={() => {
                  menuOpen.value = false;
                  removeProject(props.group().path);
                }}
              />
            ) : (
              <MenuItem
                icon="plus"
                label="Keep in projects"
                onPress={() => {
                  menuOpen.value = false;
                  addProject(props.group().path);
                }}
              />
            )
          }
        </Dynamic>
      </MenuPanel>
    </Row>
  );
}

function ProjectsHeader(): Element {
  const layout = projectSidebarLayout;
  return (
    <Row style={{ height: spacing.button, padding: [0, 0, 0, layout.inset], align: "center", shrink: 0 }}>
      <Text
        style={{ grow: 1, minWidth: 0 }}
        font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
        paint={() => ({ color: palette().textSecondary })}
      >
        Projects
      </Text>
      <IconButton icon="folder-plus" label="Add project folder" side="right"
        onPress={() => showProjectDialog("add")} />
    </Row>
  );
}

function ExpandedSidebar(): Element {
  const accountOpen = signal(false);
  let accountAnchor: number | undefined;

  return (
    <Column
      style={{
        width: spacing.sidebarWidth,
        grow: 1,
        minHeight: 0,
        shrink: 0,
        padding: [8, 8, 8, 8],
        gap: 2,
      }}
      paint={() => ({
        background: windowUnderlay(palette().sidebarBg),
        ...sidebarSwapMotion(),
      })}
    >
      {/* Search + collapse (workspaces live left of the tabs and in the overview) */}
      <Row style={{ height: 36, margin: [0, 0, 4, 0], align: "center", gap: 4, shrink: 0 }}>
        <Row
          style={{ height: spacing.control, grow: 1, minWidth: 0, padding: [0, spacing.unit, 0, spacing.unit],
            align: "center", gap: spacing.unit }}
          onPress={() => togglePalette("sessions")}
          paint={() => ({
            radius: radii.md, background: palette().appBg, edge: palette().border, edgeWidth: spacing.hairline,
            hover: { background: palette().panelBg, edge: hoverBorder() },
            transitions: [smoothTransition("background", motion.fast), smoothTransition("edge", motion.fast)],
          })}
        >
          <Glyph name="search" size={iconButtonLayout.sm.icon} color={() => palette().textMuted} />
          <Text lines={1} style={{ grow: 1, minWidth: 0 }}
            font={{ family: fontFamilies.sans, size: typeScale.caption }}
            paint={() => ({ color: palette().textMuted })}>Search</Text>
          {/* One chip per key (Ctrl ⇧ K), like the launcher hints; re-rendered on rebind. */}
          <Dynamic value={() => keysFor("session.search").join("\u0001")} style={{ shrink: 0 }}>
            {(joined) => (
              <Row style={{ gap: 3, shrink: 0 }}>
                {(joined ? joined.split("\u0001") : []).map((k) => <KbdKey label={k} />)}
              </Row>
            )}
          </Dynamic>
        </Row>

        <HintTooltip label="Collapse sidebar" side="bottom">
          <Box
            style={{ width: spacing.control, height: spacing.control, align: "center", justify: "center", shrink: 0 }}
            onPress={() => {
              sidebarCompact.value = true;
              syncLayout();
            }}
            paint={() => ({
              radius: radii.md,
              hover: { background: palette().sidebarActive },
              transitions: [smoothTransition("background", motion.fast)],
            })}
          >
            <Glyph
              name="panel-left"
              size={16}
              color={() => palette().textMuted}
              hover={() => palette().textPrimary}
            />
          </Box>
        </HintTooltip>
      </Row>

      {/* Primary navigation */}
      <SidebarNavRow icon="plus" label="New session" onPress={createSession} />
      <SidebarNavRow icon="square-terminal" label="Terminal" onPress={openTiledColumn} />
      <SidebarNavRow
        icon="folder"
        label="Files"
        onPress={toggleFiles}
      />
      <SidebarNavRow
        icon="git-branch"
        label="Git"
        onPress={openReviewTab}
        right={<CountPill count={() => String(projectChanges().files.length)} />}
      />
      <SidebarNavRow icon="globe" label="Browser" onPress={() => openBrowser()} />
      <SidebarNavRow
        icon="dashboard"
        label="Metrics"
        onPress={openMetricsTab}
      />

      <Column style={{ margin: [projectSidebarLayout.inset, 0, spacing.unit, 0], shrink: 0 }}>
        <AgentsHeader />
      </Column>
      <Column style={{ grow: 1, basis: 0, minHeight: 0, scroll: true }}>
        <Show when={() => agentSort.value === "priority"}>
          {() => <Column>
            <PriorityAgents />
            <Show when={() => configState.value.projects.length > 0}>
              {() => <Column>
                <ProjectsHeader />
                <For each={() => projectGroups().filter((group) => group.pinned)} key={(group) => group.path}>
                  {(group) => <ProjectHeader group={group} />}
                </For>
              </Column>}
            </Show>
          </Column>}
        </Show>
        <Show when={() => agentSort.value === "project"}>
          {() => <Column>
        <ProjectsHeader />
        <Show when={() => projectGroups().length === 0}>
          {() => <EmptyState compact title={() => sessionFilter.value === "all" ? "No projects yet" : "No matching sessions"}
            style={{ padding: [spacing.unit, projectSidebarLayout.inset, 0, projectSidebarLayout.inset] }} />}
        </Show>
        <For
          each={projectGroups}
          key={(g) => g.path}
          style={{ gap: spacing.unit }}
        >
          {(group) => (
            <Column style={{ gap: 1, shrink: 0 }}>
              <ProjectHeader group={group} />
              <Show when={() => group().sessions.length === 0}>
                {() => <EmptyState compact title={() => sessionFilter.value === "all" ? "No sessions yet" : "No matching sessions"}
                  style={{ padding: [spacing.unit, projectSidebarLayout.inset, spacing.unit * 1.5, spacing.unit * 8] }} />}
              </Show>
              <For each={() => group().sessions} key={(s) => s.id} style={{ gap: agentListLayout.rowGap }}>
                {(item) => <SessionRow item={item} withinProject />}
              </For>
            </Column>
          )}
        </For>
          </Column>}
        </Show>
      </Column>

      {/* Utility rows */}
      <SidebarNavRow
        icon="archive"
        label="Archived"
        onPress={openArchivedTab}
        right={<CountPill count={() => String(sessionsState.value.archivedCount)} />}
      />

      {/* CLI usage meters */}
      <Show when={() => cliUsageState.value.length > 0 || configState.value.clis.some((cli) => cli.enabled)}
        style={{ shrink: 0 }}>
        {() => <CliUsageStrip />}
      </Show>

      <UpdateChip />

      {/* Account row */}
      <Row
        ref={(id) => {
          accountAnchor = id;
        }}
        style={{ height: 36, padding: [0, 4, 0, 6], align: "center", gap: 8, shrink: 0 }}
        onPress={() => {
          accountOpen.value = !accountOpen.value;
        }}
        paint={() => ({
          radius: radii.lg,
          background: accountOpen.value ? palette().sidebarActive : 0,
          hover: { background: palette().sidebarActive },
          transitions: [smoothTransition("background", motion.fast)],
        })}
      >
        <AccountAvatar />
        <Column style={{ grow: 1, minWidth: 0 }}>
          <Text
            lines={1}
            font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
            paint={() => ({ color: palette().textPrimary })}
          >
            {accountName}
          </Text>
        </Column>
        <AccountMenu open={accountOpen} anchor={() => accountAnchor} side="top" />
      </Row>
    </Column>
  );
}

// 22px round GitHub avatar (profile photo once downloaded) with the presence dot.
function AccountAvatar(): Element {
  return (
    <Box style={{ width: 22, height: 22, shrink: 0, align: "center", justify: "center" }}>
      <Dynamic value={() => avatarReady.value} style={{ width: 22, height: 22 }}>
        {(ready) =>
          ready ? (
            // GitHub profile photo, fetched and circle-clipped by the host.
            <Image source="avatar:github" style={{ width: 22, height: 22 }} />
          ) : (
            <Box
              style={{ width: 22, height: 22, align: "center", justify: "center" }}
              paint={() => ({ radius: 11, gradient: avatarGradient(accountName()) })}
            >
              <Image source="cli:github" style={{ width: 13, height: 13 }} />
            </Box>
          )
        }
      </Dynamic>
      <Box
        style={{ position: "absolute", left: 14, top: 14, width: 9, height: 9 }}
        paint={() => ({
          radius: 5,
          background: githubState.value.connected ? palette().statusOnline : palette().statusIdle,
          edge: palette().sidebarBg,
          edgeWidth: 2,
        })}
      />
    </Box>
  );
}

// Compact rail: 36px icon buttons with right-side Accordee tooltips.
function RailButton(props: {
  label: Source<string>;
  keys?: Source<string[]>;
  icon?: string;
  image?: Source<string>;
  leading?: Element;
  active?: Source<boolean>;
  tooltipDisabled?: Source<boolean>;
  onPress: () => void;
  onContextMenu?: () => void;
  ref?: (id: number) => void;
}): Element {
  const isActive = () => read(props.active ?? false);
  return (
    <HintTooltip label={props.label} keys={props.keys} side="right" disabled={props.tooltipDisabled}>
      <Box
        ref={props.ref}
        style={{
          width: spacing.railButton,
          height: spacing.railButton,
          align: "center",
          justify: "center",
          shrink: 0,
        }}
        onPress={props.onPress}
        onContextMenu={props.onContextMenu}
        paint={() => ({
          radius: radii.lg,
          background: isActive() ? palette().sidebarActive : 0,
          hover: { background: palette().sidebarActive },
          active: { scale: controlState.iconPressScale },
          transitions: [
            smoothTransition("background", motion.fast),
            smoothTransition("scale", motion.fast),
          ],
        })}
      >
        {props.leading ??
          (props.image ? (
            <Image source={props.image} style={{ width: 18, height: 18 }} />
          ) : (
            <Glyph
              name={props.icon ?? "terminal"}
              size={18}
              color={() => (isActive() ? palette().textPrimary : palette().textSecondary)}
              hover={() => palette().textPrimary}
            />
          ))}
      </Box>
    </HintTooltip>
  );
}

function RailDivider(): Element {
  return (
    <Box
      style={{ width: 18, height: 1, margin: [8, 0, 8, 0], shrink: 0 }}
      paint={() => ({ background: palette().border })}
    />
  );
}

function SessionRailButton(props: { item: () => SessionItem }): Element {
  const menu = signal(false);
  let anchor: number | undefined;
  return (
    <Box style={{ shrink: 0 }}>
      <RailButton label={() => cleanTitle(props.item().title)} tooltipDisabled={menu}
        leading={<Image source={() => "cli:" + props.item().cli}
          style={{ width: agentListLayout.icon, height: agentListLayout.icon }} />}
        active={() => activeNav.value === "agents" && sessionsState.value.activeId === props.item().id}
        onPress={() => focusTile(props.item().id)} onContextMenu={() => { menu.value = true; }}
        ref={(id) => { anchor = id; }} />
      <MenuPanel open={menu} anchor={() => anchor} side="right">
        <ActiveSessionActions item={props.item} open={menu} />
      </MenuPanel>
    </Box>
  );
}

function CompactSidebar(): Element {
  const accountOpen = signal(false);
  let accountAnchor: number | undefined;
  const expand = () => {
    sidebarCompact.value = false;
    syncLayout();
  };

  return (
    <Column
      style={{
        width: spacing.sidebarRail,
        grow: 1,
        minHeight: 0,
        shrink: 0,
        padding: [8, 0, 8, 0],
        align: "center",
        gap: 4,
      }}
      paint={() => ({
        background: windowUnderlay(palette().sidebarBg),
        ...sidebarSwapMotion(),
      })}
    >
      <RailButton label="Expand sidebar" icon="panel-left" onPress={expand} />
      <RailButton label="Search sessions" keys={() => keysFor("session.search")} icon="search"
        onPress={() => togglePalette("sessions")} />

      <RailDivider />
      <RailButton
        label="New session"
        keys={KEYS_NEW_SESSION}
        icon="plus"
        onPress={createSession}
      />
      <RailButton label="Terminal" icon="square-terminal" keys={() => keysFor("column.new")} onPress={openTiledColumn} />
      <RailButton label="Files" icon="folder" onPress={toggleFiles} />
      <RailButton label="Add project folder" icon="folder-open" onPress={() => showProjectDialog("add")} />
      <RailButton label="Git" icon="git-branch" onPress={openReviewTab} />
      <RailButton label="Browser" icon="globe" onPress={() => openBrowser()} />
      <RailButton label="Metrics" icon="dashboard" onPress={openMetricsTab} />

      <RailDivider />
      <For
        each={sidebarAgentRows}
        key={(item) => item.id}
        style={{ grow: 1, basis: 0, minHeight: 0, scroll: true, align: "center", gap: 4 }}
      >
        {(item) => <SessionRailButton item={item} />}
      </For>

      <RailButton label="Archived" icon="archive" onPress={openArchivedTab} />
      <UpdateChip compact />
      <RailButton
        label={accountName}
        ref={(id) => {
          accountAnchor = id;
        }}
        active={() => accountOpen.value}
        leading={<AccountAvatar />}
        onPress={() => {
          accountOpen.value = !accountOpen.value;
        }}
      />
      <AccountMenu open={accountOpen} anchor={() => accountAnchor} side="right" />
    </Column>
  );
}

// The outgoing content stays drawn (frozen) and fades while the width springs, the incoming one
// fades in: the two cross-fade inside the clipped container instead of swapping in one frame.
function sidebarSwapMotion(): { starting: Starting; exiting: Exiting; transitions: Transition[] } {
  return {
    starting: { opacity: 0 },
    exiting: { opacity: 0 },
    transitions: [smoothTransition("opacity", sidebarMotion.fade)],
  };
}

// The sidebar width springs between the rail and the full sidebar; the clipped container reveals /
// hides the content like a compositor window resize.
function Sidebar(): Element {
  return (
    <Box
      style={() => ({
        width: sidebarCompact.value ? spacing.sidebarRail : spacing.sidebarWidth,
        shrink: 0,
        minHeight: 0,
        clip: true,
      })}
      paint={{ transitions: [springTransition("width")] }}
    >
      <Dynamic value={() => sidebarCompact.value} style={{ grow: 1, minHeight: 0 }}>
        {(compact) => (compact ? <CompactSidebar /> : <ExpandedSidebar />)}
      </Dynamic>
    </Box>
  );
}
