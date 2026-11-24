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
const agentDigitsHeld = signal(false);
const agentDigitsFlashed = signal(false);
let agentFlashVersion = 0;
let agentHoldVersion = 0;

// Badges appear only after Alt is held alone for a moment; a quick Alt+arrow never flashes them.
function setAgentDigitsHeld(held: boolean): void {
  const version = ++agentHoldVersion;
  if (!held) {
    agentDigitsHeld.value = false;
    return;
  }
  afterView(agentListLayout.holdDelay, () => {
    if (version === agentHoldVersion) agentDigitsHeld.value = true;
  });
}

function handleAgentDigit(digit: number): void {
  const list = sidebarAgentRows.value;
  if (digit === 0 && list.length) {
    const version = ++agentFlashVersion;
    agentDigitsFlashed.value = true;
    afterView(agentListLayout.flashDuration, () => {
      if (version === agentFlashVersion) agentDigitsFlashed.value = false;
    });
  } else if (list[digit - 1]) {
    focusTile(list[digit - 1].id);
  }
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
  if (activeNav.value !== "agents" || overviewOpen2.value || imageViewer.value || projectDialog.value
    || paletteRequest.value || releaseNotesOpen.value || restartConfirmOpen.value) return 0;
  return Math.min(9, sidebarAgentRows.value.length);
}

function agentNumber(id: number): number {
  if (!agentDigitsHeld.value && !agentDigitsFlashed.value) return 0;
  return sidebarAgentRows.value.slice(0, 9).findIndex((item) => item.id === id) + 1;
}

function AgentNumber(props: { number: () => number }): Element {
  const size = agentListLayout.badge;
  return <Show when={() => props.number() > 0} style={{ shrink: 0 }}>
    {() => <Box style={{ width: size, height: size, align: "center", justify: "center" }}
      paint={() => ({ radius: radii.sm, background: accent().brand })}>
      <Text font={{ family: fontFamilies.mono, size: typeScale.tiny, weight: fontWeights.semibold }}
        paint={() => ({ color: accent().brandFg })}>{() => String(props.number())}</Text>
    </Box>}
  </Show>;
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

function SessionRow(props: { item: () => SessionItem; withinProject?: boolean }): Element {
  const layout = agentListLayout;
  const isActive = () => activeNav.value === "agents" && sessionsState.value.activeId === props.item().id;
  const category = () => agentCategory(props.item());
  const title = () => cleanTitle(props.item().title) || cliDisplayName(props.item().cli);
  const project = () => basename(props.item().cwd) || props.item().cwd;
  const label = () => props.withinProject ? title() : `${project()} · ${title()}`;
  const status = () => props.item().alive && props.item().status === "failed" ? "Failed"
    : agentSections.find((section) => section.id === category())?.title ?? "Idle";
  const color = () => status() === "Failed" ? palette().destructive : agentStatusColor(category());
  return (
    <HintTooltip label={title} detail={status} side="right" style={{ shrink: 0 }}>
      <Row style={{ height: layout.rowHeight, padding: [0, layout.padding, 0, layout.padding], align: "center", gap: layout.gap }}
        onPress={() => focusTile(props.item().id)} paint={() => ({
          radius: radii.lg,
          background: isActive() ? palette().sidebarActive : 0,
          hover: { background: palette().sidebarActive },
          a11y: { label: `${label()}, ${status()}` },
          transitions: [smoothTransition("background", motion.fast)],
        })}>
        {/* The number takes the logo's slot, so holding Alt never shifts the row's text. */}
        <Dynamic value={() => agentNumber(props.item().id) > 0} style={{ shrink: 0 }}>
          {(numbered) => numbered ? <AgentNumber number={() => agentNumber(props.item().id)} />
            : <Image source={() => `cli:${props.item().cli}`} style={{ width: layout.icon, height: layout.icon, shrink: 0 }} />}
        </Dynamic>
        <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.label }}
          spans={() => props.withinProject ? [{ text: label(), color: isActive() ? palette().textPrimary : palette().textSecondary }]
            : [{ text: project(), color: isActive() ? palette().textPrimary : palette().textSecondary },
                { text: ` · ${title()}`, color: palette().textMuted }]}>{label}</Text>
        <Box style={{ width: layout.statusSize, height: layout.statusSize, align: "center", justify: "center", shrink: 0 }}>
          <Dynamic value={category}>
            {(kind) => kind === "blocked" ? <Glyph name="alert" size={layout.statusSize} color={color} />
              : kind === "working" ? <Glyph name={() => `agent-working-${shimmerTick.value % agentListLayout.spinnerFrames}`}
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

type ProjectDialogMode = "session" | "add";

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

const projectDialog = signal<{ mode: ProjectDialogMode; path?: string; cliId?: string } | null>(null);

function showProjectDialog(mode: ProjectDialogMode, path?: string, cliId?: string): void {
  if (overviewOpen2.value) closeOverviewMode();
  projectDialog.value = { mode, path, cliId };
  requestHost({ type: "projectDialogKeys", on: true });
}

function closeProjectDialog(): void {
  projectDialog.value = null;
  requestHost({ type: "projectDialogKeys", on: false });
}

// Keys the window routes to the open launcher (up/down/tab/enter/escape/agentN); set while it is mounted.
let projectDialogKeyHandler: ((action: string) => void) | undefined;
// The agent last launched from the dialog, preselected next time.
let lastLaunchCli: string | undefined;
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

function launcherLiveCount(path: string): number {
  return sessionsState.value.items.filter((session) => session.alive && session.cwd === path).length;
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
  const layout = sessionLauncherLayout;
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
  const layout = sessionLauncherLayout;
  const row = props.row;
  const glyph = () => launcherPlaceGlyph(row().path,
    row().kind === "here" ? "folder-open" : row().branch ? "git-branch" : "folder");
  const strong = () => props.selected() || row().kind === "here";
  return (
    <Row style={{ height: layout.rowHeight, padding: layout.rowPadding, align: "center", gap: layout.gap, shrink: 0 }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: props.selected() ? palette().sidebarActive : 0,
        hover: { background: props.selected() ? palette().sidebarActive : rowHoverBackground() },
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

// Step pill: number (a check once done), the step name, and what it holds.
function LauncherStep(props: { index: number; label: string; value: () => string; branch?: () => string;
  active: () => boolean; done: () => boolean; onPress: () => void }): Element {
  const layout = sessionLauncherLayout;
  return (
    <Row style={{ height: spacing.controlLg, padding: [0, spacing.unit * 3, 0, spacing.unit * 1.5],
      gap: layout.gap, align: "center", minWidth: 0, shrink: 1 }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        edge: props.active() ? hoverBorder() : 0, edgeWidth: spacing.hairline,
        hover: { background: rowHoverBackground() },
        transitions: [smoothTransition("edge", motion.fast), smoothTransition("background", motion.fast)],
      })}>
      <Box style={{ width: layout.stepDot, height: layout.stepDot, align: "center", justify: "center", shrink: 0 }}
        paint={() => ({
          radius: layout.stepDot / 2,
          edge: props.active() ? accent().brandText : palette().border, edgeWidth: spacing.hairline * 2,
          transitions: [smoothTransition("edge", motion.fast)],
        })}>
        <Dynamic value={props.done}>
          {(done) => done
            ? <Glyph name="check" size={typeScale.tiny} color={() => accent().brandText} />
            : <Text font={{ family: fontFamilies.mono, size: typeScale.micro, weight: fontWeights.medium }}
                paint={() => ({ color: props.active() ? accent().brandText : palette().textMuted })}>{String(props.index)}</Text>}
        </Dynamic>
      </Box>
      <Text font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
        paint={() => ({ color: props.active() ? palette().textPrimary : palette().textMuted })}>{props.label}</Text>
      <Text lines={1} style={{ shrink: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.caption }}
        paint={() => ({ color: palette().textSecondary })}>{props.value}</Text>
      {props.branch ? (
        <Show when={() => !!props.branch!()} style={{ shrink: 0 }}>
          {() => <BranchBadge name={props.branch!} muted />}
        </Show>
      ) : null}
    </Row>
  );
}

function LauncherPlace(props: { name: Source<string>; glyph: string; count: Source<number>;
  active: () => boolean; onPress: () => void }): Element {
  const layout = sessionLauncherLayout;
  return (
    <Row style={{ height: layout.placeHeight, padding: [0, spacing.unit * 2, 0, spacing.unit * 2],
      gap: layout.gap, align: "center", shrink: 0 }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: props.active() ? palette().sidebarActive : 0,
        hover: { background: props.active() ? palette().sidebarActive : rowHoverBackground() },
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

function ProjectDialog(): Element {
  return (
    <Show when={() => projectDialog.value !== null}>
      {() => {
        const request = projectDialog.value!;
        const adding = request.mode === "add";
        const layout = sessionLauncherLayout;
        const error = signal("");
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

        // Step 1 picks the folder, step 2 the agent ("add" stops after step 1).
        const step = signal<1 | 2>(1);
        const chosen = signal<{ path: string; branch: string } | null>(null);

        // A query starting like a path ("/", "~", "C:\\") browses: everything up to the last
        // separator is the folder, the rest filters it. Any other text filters the folder and recents.
        const base = run(() => browseHost(request.path ?? workspacePath.value)) ?? null;
        const query = signal(request.path ? request.path + "/" : "");
        const location = signal<ProjectFolders | null>(base);
        let browsedDir = "";
        const split = (text: string): [string, string] => {
          const cut = Math.max(text.lastIndexOf("/"), text.lastIndexOf("\\"));
          if (cut < 0) return [text === "~" ? "~/" : text, ""];
          return [text.slice(0, cut + 1), text.slice(cut + 1)];
        };
        effect(() => {
          const text = query.value;
          if (!launcherPathMode(text)) return;
          const [dir] = split(text);
          if (dir === browsedDir) return;
          browsedDir = dir;
          const found = run(() => browseHost(dir, location.value?.path ?? base?.path));
          if (found) location.value = found;
        });
        const filter = () => launcherPathMode(query.value) ? split(query.value)[1] : query.value.trim();

        const recents = (): LaunchRow[] => Array.from(new Set([
          ...configState.value.projects,
          ...sessionsState.value.items.map((session) => session.cwd),
        ])).map((path) => ({
          key: "recent:" + path, kind: "recent" as const, name: basename(path) || path, path,
          depth: 0, last: true, stem: false, branch: "", detail: tilde(path), count: launcherLiveCount(path),
        }));
        // The current folder under its ancestors (from ~ when inside home), then its subfolders.
        const rows = (): LaunchRow[] => {
          const here = location.value;
          const text = filter();
          const out: LaunchRow[] = launcherPathMode(query.value) || !text ? []
            : recents().filter((row) => launcherMatch(text, row.name, row.path));
          if (!here) return out;
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
              branch: isHere ? here.branch ?? "" : "", detail: "", count: isHere ? launcherLiveCount(level.path) : 0 });
          });
          children.forEach((folder, index) => out.push({
            key: "folder:" + folder.path, kind: "folder", name: folder.name, path: folder.path,
            depth: chain.length, last: index === children.length - 1, stem: false, branch: folder.branch ?? "",
            detail: "", count: launcherLiveCount(folder.path),
          }));
          return out;
        };
        const cursor = signal<string | null>(null);
        effect(() => { void query.value; void location.value; cursor.value = null; });
        // A filter selects its first match (none: Enter validates the typed path); otherwise the current folder.
        const current = (): LaunchRow | undefined => {
          const list = rows();
          return list.find((row) => row.key === cursor.value) ?? (filter()
            ? list.find((row) => row.kind === "folder" || row.kind === "recent")
            : list.find((row) => row.kind === "here"));
        };

        const agents = () => [
          ...configState.value.clis.filter((cli) => cli.enabled).map((cli) => ({ id: cli.id, name: cli.name,
            detail: cli.description || [cli.command, ...cli.args].join(" ") })),
          { id: "", name: "Shell", detail: "Plain terminal" },
        ];
        const initialAgent = request.cliId ?? lastLaunchCli ?? agents()[0]?.id ?? "";
        const agent = signal(agents().some((a) => a.id === initialAgent) ? initialAgent : "");
        const agentQuery = signal("");
        const shownAgents = () => agents().filter((a) => launcherMatch(agentQuery.value, a.name, a.detail));
        const currentAgent = () => shownAgents().find((a) => a.id === agent.value) ?? shownAgents()[0];
        const agentName = () => agents().find((a) => a.id === agent.value)?.name ?? "Shell";

        const setQuery = (text: string) => {
          field?.replace(text);
          query.value = text;
        };
        const open = (row: LaunchRow | undefined) => {
          // The field holds absolute paths (`~` only shortens descriptions), so any host resolves them.
          if (row && row.kind !== "here") setQuery(row.path + "/");
        };
        const choose = (row = current()) => {
          // Nothing matched: validate what was typed as a folder.
          const folder = row ? row.path : run(() => browseHost(query.value, location.value?.path))?.path;
          if (!folder) return;
          if (adding) {
            if (run(() => jsnirRequest(JSON.stringify({ type: "addProject", path: folder }))) !== undefined) closeProjectDialog();
            return;
          }
          // Subfolders share the current repo's branch unless they are repos themselves.
          const inside = row?.kind === "here" || row?.kind === "folder";
          const branch = row?.branch || (inside ? location.value?.branch ?? "" : "");
          chosen.value = { path: folder, branch };
          agentQuery.value = "";
          step.value = 2;
        };
        const back = () => {
          error.value = "";
          step.value = 1;
        };
        const launch = (pick = currentAgent()) => {
          const folder = chosen.value?.path;
          if (!folder || !pick) return;
          const result = run(() => jsnirRequest(JSON.stringify(
            { type: pick.id ? "launchCli" : "newSession", cliId: pick.id, cwd: folder })));
          if (result === undefined) return;
          lastLaunchCli = pick.id;
          activeNav.value = "agents";
          closeProjectDialog();
        };
        const move = (delta: number) => {
          if (step.value === 2) {
            const list = shownAgents();
            const at = list.findIndex((a) => a.id === currentAgent()?.id);
            const next = list[at + delta];
            if (next) agent.value = next.id;
            return;
          }
          const list = rows();
          if (list.length === 0) return;
          const at = Math.max(0, list.findIndex((row) => row.key === current()?.key));
          const index = (at + delta + list.length) % list.length;
          cursor.value = list[index].key;
          if (listId === undefined) return;
          // Rows share one height: keep the selected one centred.
          const visible = jsnir.columns.height[listId] ?? 0;
          scrollTo(listId, { offset: Math.max(0, index * layout.rowHeight - (visible - layout.rowHeight) / 2), smooth: true });
        };
        projectDialogKeyHandler = (action) => {
          if (action === "escape") (step.value === 2 ? back() : closeProjectDialog());
          else if (action === "up") move(-1);
          else if (action === "down") move(1);
          else if (action === "tab" && step.value === 1) open(current());
          else if (action === "enter") (step.value === 1 ? choose() : launch());
          else if (action.startsWith("agent") && !adding) {
            const pick = agents()[Number(action.slice(5)) - 1];
            if (pick) agent.value = pick.id;
          }
        };
        onCleanup(() => { projectDialogKeyHandler = undefined; });

        const hint = (keys: string, label: string) => (
          <Row style={{ gap: spacing.unit, align: "center", shrink: 0 }}>
            <KbdKey label={keys} />
            <Text font={{ family: fontFamilies.sans, size: typeScale.tiny }}
              paint={() => ({ color: palette().textMuted })}>{label}</Text>
          </Row>
        );
        const hairline = (sides: [number, number, number, number]) => () =>
          ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: sides });
        const sectionTitle = (title: string) => (
          <Row style={{ height: layout.sectionHeight, padding: [0, spacing.unit * 2, 0, spacing.unit * 2], align: "center", shrink: 0 }}>
            <Text font={{ family: fontFamilies.sans, size: typeScale.tiny, weight: fontWeights.medium }}
              paint={() => ({ color: palette().textMuted })}>{title.toUpperCase()}</Text>
          </Row>
        );
        const goTo = (path: string) => setQuery(path + "/");

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
              paint={() => ({ scrollbar: editorScrollbar(), ...hairline([0, 1, 0, 0])() })}>
              {groups.map((group) => (
                <Column style={{ shrink: 0 }}>
                  {sectionTitle(group.title)}
                  {group.items.map((item) => <LauncherPlace name={item.name} glyph={item.glyph}
                    count={0} active={() => location.value?.path === item.path} onPress={() => goTo(item.path)} />)}
                </Column>
              ))}
              <Show when={() => recents().length > 0} style={{ shrink: 0 }}>
                {() => (
                  <Column>
                    {sectionTitle("Recent")}
                    <For each={recents} key={(row) => row.key} style={{ shrink: 0 }}>
                      {(row) => <LauncherPlace name={() => row().name} glyph="clock" count={() => row().count}
                        active={() => location.value?.path === row().path} onPress={() => goTo(row().path)} />}
                    </For>
                  </Column>
                )}
              </Show>
            </Column>
          );
        };

        const folderStep = () => (
          <Column style={{ grow: 1, minHeight: 0 }}>
            {/* Search / path field */}
            <Row style={{ height: layout.inputHeight, padding: layout.inputPadding, gap: layout.gap, align: "center", shrink: 0 }}
              paint={hairline([0, 0, 1, 0])}>
              <Glyph name={adding ? "folder-plus" : "search"} size={layout.icon} color={() => palette().textMuted} />
              <TextInput
                value={query.value}
                ref={(input) => { field = input; }}
                autoFocus
                retainOnSubmit
                style={{ grow: 1, minWidth: 0, height: spacing.control }}
                font={{ family: fontFamilies.sans, size: typeScale.body }}
                placeholder={adding ? "Filter folders or type a path…" : "Filter this folder, search recents or type a path…"}
                placeholderColor={palette().textMuted}
                onChange={(text) => { error.value = ""; query.value = text; }}
                onSubmit={() => choose()}
                paint={() => ({ color: palette().textPrimary })}
              />
              <IconButton icon="arrow-up" label="Parent folder" side="bottom" layer={projectDialogLayout.layer + 2} size="sm"
                disabled={() => !location.value?.parent}
                onPress={() => { const up = location.value?.parent; if (up) goTo(up); }} />
              <IconButton icon="home" label="Home folder" side="bottom" layer={projectDialogLayout.layer + 2} size="sm"
                onPress={() => goTo(launcherHome || "~")} />
            </Row>
            <Row style={{ grow: 1, minHeight: 0 }}>
              {placesColumn()}
              {/* Folder tree */}
              <Column ref={(node) => { listId = node; }}
                style={{ grow: 1, minWidth: 0, minHeight: 0, scroll: true, clip: true,
                  padding: [layout.listInset, layout.listInset, layout.listInset, layout.listInset] }}
                paint={() => ({ scrollbar: editorScrollbar() })}>
                <For each={rows} key={(row) => row.key} style={{ shrink: 0 }}>
                  {(row) => <LauncherRow row={row}
                    selected={() => current()?.key === row().key}
                    onPress={() => (row().kind === "here" ? choose(row()) : open(row()))} />}
                </For>
                <Show when={() => !!location.value && location.value.folders.length === 0}>
                  {() => <Text style={{ padding: layout.rowPadding }} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                    paint={() => ({ color: palette().textMuted })}>No subfolders</Text>}
                </Show>
                <Show when={() => !!filter() && !current() && !error.value}>
                  {() => <EmptyState compact title="No folder matches" style={{ padding: layout.gap }} />}
                </Show>
              </Column>
            </Row>
          </Column>
        );

        const agentStep = () => (
          <Column style={{ grow: 1, minHeight: 0 }}>
            <Row style={{ height: layout.inputHeight, padding: layout.inputPadding, gap: layout.gap, align: "center", shrink: 0 }}
              paint={hairline([0, 0, 1, 0])}>
              <Glyph name="search" size={layout.icon} color={() => palette().textMuted} />
              <TextInput autoFocus retainOnSubmit
                style={{ grow: 1, minWidth: 0, height: spacing.control }}
                font={{ family: fontFamilies.sans, size: typeScale.body }}
                placeholder="Filter agents…" placeholderColor={palette().textMuted}
                onChange={(text) => { agentQuery.value = text; }}
                onSubmit={() => launch()}
                paint={() => ({ color: palette().textPrimary })}
              />
            </Row>
            <Column style={{ grow: 1, minHeight: 0, scroll: true, clip: true,
              padding: [layout.listInset, layout.listInset, layout.listInset, layout.listInset * 2] }}
              paint={() => ({ scrollbar: editorScrollbar() })}>
              {sectionTitle("Agents")}
              <For each={shownAgents} key={(entry) => entry.id || "shell"} style={{ shrink: 0 }}>
                {(entry) => {
                  const index = () => agents().findIndex((a) => a.id === entry().id) + 1;
                  return (
                    <PaletteResultRow
                      result={() => ({ key: entry().id || "shell", icon: "square-terminal",
                        image: entry().id ? "cli:" + entry().id : undefined, label: entry().name,
                        detail: index() <= 9 ? `Alt ${index()} · ${entry().detail}` : entry().detail, onSelect: () => {} })}
                      selected={() => currentAgent()?.id === entry().id}
                      onSelect={() => launch(entry())} />
                  );
                }}
              </For>
              <Show when={() => shownAgents().length === 0}>
                {() => <EmptyState compact title="No agent matches" style={{ padding: layout.gap }} />}
              </Show>
            </Column>
          </Column>
        );

        return (
          <Box
            style={() => ({
              position: "fixed", left: 0, top: 0,
              width: viewportSize.value[0], height: viewportSize.value[1],
              layer: projectDialogLayout.layer, align: "center", justify: "center",
            })}
            onPress={closeProjectDialog}
            paint={() => ({ background: palette().backdrop })}
          >
            <Column
              style={() => ({
                width: layout.width, maxWidth: Math.max(0, viewportSize.value[0] - layout.screenMargin * 2),
                height: layout.height, maxHeight: Math.max(0, viewportSize.value[1] - layout.screenMargin * 2),
                clip: true,
              })}
              onPress={() => {}}
              paint={() => ({
                radius: radii.xl, background: palette().cardBg, edge: palette().border,
                edgeWidth: spacing.hairline, shadows: [shadows.modal], ...accordeeDialogMotion(),
              })}
            >
              {/* Steps: folder → agent */}
              {adding ? <Box /> : (
                <Row style={{ height: layout.stepsHeight, padding: [0, layout.listInset * 2, 0, layout.listInset * 2],
                  gap: layout.gap, align: "center", shrink: 0 }} paint={hairline([0, 0, 1, 0])}>
                  <LauncherStep index={1} label="Folder"
                    value={() => tilde(step.value === 2 ? chosen.value?.path ?? "" : location.value?.path ?? "")}
                    branch={() => step.value === 2 ? chosen.value?.branch ?? "" : location.value?.branch ?? ""}
                    active={() => step.value === 1} done={() => step.value === 2} onPress={back} />
                  <Box style={{ width: layout.gap * 2, height: spacing.hairline * 2, shrink: 0 }}
                    paint={() => ({ background: palette().border })} />
                  <LauncherStep index={2} label="Agent" value={agentName}
                    active={() => step.value === 2} done={() => false}
                    onPress={() => { if (step.value === 1) choose(); }} />
                </Row>
              )}

              <Dynamic value={() => step.value} style={{ grow: 1, minHeight: 0 }}>
                {(at) => at === 1 ? folderStep() : agentStep()}
              </Dynamic>

              {/* Hints + the step's action */}
              <Row style={{ padding: layout.barPadding, gap: layout.gap, align: "center", shrink: 0 }}
                paint={hairline([1, 0, 0, 0])}>
                <Dynamic value={() => `${step.value}:${error.value}`} style={{ grow: 1, minWidth: 0 }}>
                  {() => error.value
                    ? <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
                        paint={() => ({ color: palette().destructive })}>{error.value}</Text>
                    : step.value === 1
                      ? <Row style={{ gap: layout.gap * 2, align: "center", clip: true }}>
                          {hint("↑↓", "select")}
                          {hint("Tab", "open folder")}
                          {hint("↵", adding ? "add" : "choose")}
                          {hint("Esc", "close")}
                        </Row>
                      : <Row style={{ gap: layout.gap * 2, align: "center", clip: true }}>
                          {hint("↑↓", "select")}
                          {hint("Alt 1–9", "agent")}
                          {hint("Esc", "back")}
                        </Row>}
                </Dynamic>
                <Row style={{ height: layout.actionHeight, padding: [0, spacing.unit * 2, 0, spacing.unit * 3], gap: spacing.unit * 2, align: "center", shrink: 0 }}
                  onPress={() => (step.value === 1 ? choose() : launch())}
                  paint={() => ({
                    radius: radii.md,
                    background: accent().brand,
                    hover: { opacity: controlState.hoverOpacity },
                    active: { scale: controlState.pressScale },
                    transitions: [
                      smoothTransition("background", motion.fast),
                      smoothTransition("opacity", motion.fast),
                      smoothTransition("scale", motion.fast),
                    ],
                  })}>
                  <Text font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
                    paint={() => ({ color: accent().brandFg })}>
                    {() => adding ? "Add project" : step.value === 1 ? "Choose agent"
                      : currentAgent()?.id ? `Launch ${currentAgent()!.name}` : "Open shell"}
                  </Text>
                  <Glyph name={() => step.value === 1 && !adding ? "arrow-right" : "check"} size={typeScale.caption}
                    color={() => accent().brandFg} />
                </Row>
              </Row>
            </Column>
          </Box>
        );
      }}
    </Show>
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
        background: palette().sidebarBg,
        ...sidebarSwapMotion(),
      })}
    >
      {/* Search + collapse (workspaces live left of the tabs and in the overview) */}
      <Row style={{ height: 36, margin: [0, 0, 4, 0], align: "center", gap: 4, shrink: 0 }}>
        <Row
          style={{ height: spacing.controlLg, grow: 1, minWidth: 0, padding: [0, spacing.unit, 0, spacing.unit],
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
            style={{ width: 32, height: 32, align: "center", justify: "center", shrink: 0 }}
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
  onPress: () => void;
  ref?: (id: number) => void;
}): Element {
  const isActive = () => read(props.active ?? false);
  return (
    <HintTooltip label={props.label} keys={props.keys} side="right">
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
        background: palette().sidebarBg,
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
        {(item) => (
          <RailButton
            label={() => cleanTitle(item().title)}
            leading={<Dynamic value={() => agentNumber(item().id) > 0}>
              {(numbered) => numbered ? <AgentNumber number={() => agentNumber(item().id)} />
                : <Image source={() => `cli:${item().cli}`} style={{ width: agentListLayout.icon, height: agentListLayout.icon }} />}
            </Dynamic>}
            active={() => activeNav.value === "agents" && sessionsState.value.activeId === item().id}
            onPress={() => focusTile(item().id)}
          />
        )}
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
