// Accordee "Settings" screen: full-window settings sidebar (Back to workspace, quick search,
// grouped nav) and a centered content column with section headings over hairline cards.

const SETTINGS_ACCENTS: AccentName[] = [
  "neutral",
  "blue",
  "violet",
  "rose",
  "orange",
  "green",
  "teal",
];

interface SettingsPage {
  id: SettingsTab;
  icon: string;
  label: string;
  group: string;
}

const SETTINGS_PAGES: SettingsPage[] = [
  {
    id: "appearance",
    icon: "palette",
    label: "Appearance",
    group: "Workspace",
  },
  {
    id: "tiling",
    icon: "columns",
    label: "Layout & Shortcuts",
    group: "Workspace",
  },
  {
    id: "clis",
    icon: "terminal",
    label: "CLIs & Agents",
    group: "Integrations",
  },
  {
    id: "github",
    icon: "git-branch",
    label: "GitHub & SSH",
    group: "Integrations",
  },
  {
    id: "updates",
    icon: "download",
    label: "Updates",
    group: "App",
  },
];

function currentSettingsPage(): SettingsPage {
  return SETTINGS_PAGES.find((p) => p.id === settingsTab.value) ?? SETTINGS_PAGES[0];
}

// Section heading (Accordee "Appearance") above a hairline card (radius 12, card background).
// `flush` cards start with rows, whose top dividers then sit on the card edge.
function SettingsCard(props: {
  title: Source<string>;
  description?: Source<string>;
  right?: Element;
  flush?: boolean;
  children: Element | Element[];
}): Element {
  const layout = settingsLayout;
  const stackHeading = () => narrowSettings() && props.right !== undefined;
  return (
    <Column style={{ gap: layout.inset, shrink: 0 }} paint={accordeeFadeUpMotion()}>
      <Box style={() => ({
        direction: stackHeading() ? "column" : "row",
        align: stackHeading() ? "start" : "center",
        justify: "between", gap: layout.fieldGap,
      })}>
        <Column style={{ gap: spacing.unit / 2, grow: 1, minWidth: 0 }}>
          <Text
            font={{ family: fontFamilies.sans, size: typeScale.title, weight: fontWeights.medium }}
            paint={() => ({ color: palette().textPrimary })}
          >
            {props.title}
          </Text>
          {props.description ? (
            <Text
              font={{ family: fontFamilies.sans, size: typeScale.label }}
              paint={() => ({ color: palette().textMuted })}
            >
              {props.description}
            </Text>
          ) : null}
        </Column>
        {props.right ?? null}
      </Box>
      <Column
        style={{ padding: props.flush ? 0 : layout.inset, gap: props.flush ? 0 : layout.inset, clip: true }}
        paint={() => ({
          radius: radii.lg,
          background: palette().sidebarBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
        })}
      >
        {props.children}
      </Column>
    </Column>
  );
}

function narrowSettings(): boolean {
  return viewportSize.value[0] < settingsLayout.stackedAt;
}

// Text and controls are stacked when the content column becomes narrow.
function SettingsRowItem(props: {
  title: Source<string>;
  subtitle?: Source<string>;
  divider?: Source<boolean>;
  control: Element;
}): Element {
  return (
    <Box
      style={() => ({
        direction: narrowSettings() ? "column" : "row",
        padding: settingsLayout.rowPadding,
        align: narrowSettings() ? "start" : "center",
        justify: "between",
        gap: narrowSettings() ? settingsLayout.inset : settingsLayout.fieldGap,
      })}
      paint={() => ({
        edge: palette().border,
        edgeWidth: read(props.divider ?? true) ? spacing.hairline : 0,
        edgeSides: [1, 0, 0, 0],
      })}
    >
      <Column style={{ gap: spacing.unit / 2, grow: 1, minWidth: 0 }}>
        <Text font={{ family: fontFamilies.sans, size: typeScale.body }}
          paint={() => ({ color: palette().textPrimary })}>
          {props.title}
        </Text>
        {props.subtitle !== undefined ? (
          <Text font={{ family: fontFamilies.sans, size: typeScale.caption, lineHeight: 1.4 }}
            paint={() => ({ color: palette().textMuted })}>
            {props.subtitle}
          </Text>
        ) : null}
      </Column>
      <Box style={{ shrink: 0 }}>{props.control}</Box>
    </Box>
  );
}

function KeybindRow(props: { item: () => KeybindView; divider: () => boolean }): Element {
  const recording = () => keybindRecording.value === props.item().id;
  const actionSlot = { width: iconButtonLayout.md.size, shrink: 0 };
  return (
    <SettingsRowItem title={() => props.item().label} subtitle={() => props.item().description}
      divider={props.divider}
      control={
        <Row style={{ align: "center", gap: settingsLayout.inset, shrink: 0 }}>
          <Dynamic value={() => recording() ? "\u0002" : props.item().chords.join("\u0001")} style={{ shrink: 0 }}>
            {(value) => value === "\u0002" ? (
              <Row style={{ align: "center", gap: spacing.unit }}>
                <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
                  paint={() => ({ color: accent().brandText })}>Press keys…</Text>
                <KbdKey label="Esc" />
              </Row>
            ) : value === "" ? (
              <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
                paint={() => ({ color: palette().textMuted })}>Unassigned</Text>
            ) : (
              <Box style={() => ({
                direction: narrowSettings() ? "column" : "row", gap: spacing.unit, align: "start",
              })}>
                {value.split("\u0001").map((chord) => (
                  <Row style={{ gap: spacing.unit / 2 }}>
                    {chordParts(chord).map((key) => <KbdKey label={key} />)}
                  </Row>
                ))}
              </Box>
            )}
          </Dynamic>
          <Row style={{ align: "center", gap: spacing.unit / 2, shrink: 0 }}>
            <Dynamic value={recording} style={actionSlot}>
              {(active) => active ? (
                <IconButton icon="close" label="Cancel recording" highlighted onPress={stopKeybindRecording} />
              ) : (
                <IconButton icon="keyboard" label="Record shortcut"
                  onPress={() => startKeybindRecording(props.item().id)} />
              )}
            </Dynamic>
            <Show when={() => !recording() && props.item().chords.length > 0} style={actionSlot}>
              {() => <IconButton icon="close" label="Unassign shortcut"
                onPress={() => requestHost({ type: "setKeybind", id: props.item().id, chord: "" })} />}
            </Show>
            <Show when={() => !recording() && props.item().customized} style={actionSlot}>
              {() => <IconButton icon="refresh" label="Restore default shortcut"
                onPress={() => requestHost({ type: "resetKeybind", id: props.item().id })} />}
            </Show>
          </Row>
        </Row>
      }
    />
  );
}

function SettingsNavItem(props: { page: SettingsPage }): Element {
  const active = () => settingsTab.value === props.page.id;
  return (
    <Row
      style={{ height: 30, padding: [0, 8, 0, 8], align: "center", gap: 8, shrink: 0 }}
      onPress={() => {
        if (settingsTab.value !== props.page.id && keybindRecording.value) stopKeybindRecording();
        settingsTab.value = props.page.id;
      }}
      paint={() => ({
        radius: radii.md,
        background: active() ? palette().sidebarActive : 0,
        hover: { background: palette().sidebarActive },
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Glyph
        name={props.page.icon}
        size={16}
        color={() => (active() ? palette().textPrimary : palette().textSecondary)}
      />
      <Text
        lines={1}
        style={{ grow: 1, minWidth: 0 }}
        font={{ family: fontFamilies.sans, size: typeScale.label }}
        paint={() => ({ color: active() ? palette().textPrimary : palette().textSubtle })}
      >
        {() => narrowSettings() && props.page.id === "tiling" ? "Shortcuts" : props.page.label}
      </Text>
    </Row>
  );
}

function SettingsSidebar(): Element {
  const query = signal("");
  const visible = (group: string) => () =>
    SETTINGS_PAGES.filter(
      (p) => p.group === group && p.label.toLowerCase().includes(query.value.trim().toLowerCase())
    );
  const groups = ["Workspace", "Integrations", "App"];
  return (
    <Column
      style={() => ({
        width: narrowSettings() ? settingsLayout.narrowSidebarWidth : settingsLayout.sidebarWidth,
        shrink: 0, padding: [settingsLayout.fieldGap, settingsLayout.inset, settingsLayout.inset, settingsLayout.inset],
        gap: spacing.unit / 2,
      })}
      paint={() => ({ background: palette().sidebarBg, ...accordeeFadeUpMotion() })}
    >
      <Row
        style={{ height: 28, padding: [0, 8, 0, 6], align: "center", gap: 8, shrink: 0 }}
        onPress={() => {
          if (keybindRecording.value) stopKeybindRecording();
          activeNav.value = "agents";
        }}
        paint={() => ({ radius: radii.md, hover: { background: palette().sidebarActive } })}
      >
        <Glyph name="arrow-left" size={15} color={() => palette().textSecondary} hover={() => palette().textPrimary} />
        <Text
          lines={1}
          style={{ grow: 1, minWidth: 0 }}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          paint={() => ({ color: palette().textSecondary })}
        >
          {() => narrowSettings() ? "Back" : "Back to workspace"}
        </Text>
      </Row>
      <Box style={{ padding: [14, 8, 14, 8], shrink: 0 }}>
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.heading, weight: fontWeights.medium }}
          paint={() => ({ color: palette().textPrimary })}
        >
          Settings
        </Text>
      </Box>
      <Row
        style={{ height: 32, margin: [0, 0, 8, 0], padding: [0, 6, 0, 10], align: "center", gap: 8, shrink: 0 }}
        paint={() => ({
          radius: radii.md,
          background: palette().appBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
        })}
      >
        <Glyph name="search" size={15} color={() => palette().textMuted} />
        <TextInput
          style={{ grow: 1, height: 24, padding: [3, 2, 2, 0] }}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          placeholder="Find a setting"
          placeholderColor={palette().textMuted}
          onChange={(text) => {
            query.value = text;
          }}
          paint={() => ({ color: palette().textPrimary })}
        />
      </Row>
      {groups.map((group) => (
        <Show when={() => visible(group)().length > 0} style={{ shrink: 0 }}>
          {() => (
            <Column style={{ gap: 1 }}>
              <Box style={{ padding: [14, 8, 6, 8] }}>
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.caption }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {group}
                </Text>
              </Box>
              <For each={visible(group)} key={(p) => p.id} style={{ gap: 1 }}>
                {(page) => <SettingsNavItem page={page()} />}
              </For>
            </Column>
          )}
        </Show>
      ))}
      <Box style={{ grow: 1 }} />
      <Row
        style={{ height: 36, padding: [0, 8, 0, 8], align: "center", gap: 8, shrink: 0 }}
        paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [1, 0, 0, 0] })}
      >
        <Image source="brand:icy" style={{ width: 16, height: 16 }} />
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.caption }}
          paint={() => ({ color: palette().textMuted })}
        >
          {() => `icy v${updateState.value.currentVersion}`}
        </Text>
      </Row>
    </Column>
  );
}

function SettingsShell(): Element {
  return (
    <Row style={{ grow: 1, minHeight: 0, align: "stretch" }}>
      <SettingsSidebar />
      <Column
        style={{ grow: 1, minWidth: 0, minHeight: 0, margin: [8, 8, 8, 0], clip: true }}
        paint={() => ({
          radius: radii.lg,
          // No outline: the frame reads through the sidebar/app background contrast.
          background: palette().appBg,
          shadows: [shadows.panel],
        })}
      >
        <SettingsView />
      </Column>
    </Row>
  );
}

// niri's layout options: how the strip scrolls to the focused column and how wide new ones open.
function TilingSettings(): Element {
  const layout = () => configState.value.layout;
  const center = signal(layout().centerFocusedColumn);
  const width = signal(layout().defaultColumnWidth);
  effect(() => {
    center.value = layout().centerFocusedColumn;
    width.value = layout().defaultColumnWidth;
  });
  const save = (change: Partial<TilingLayoutOptions>) => updateConfig({ layout: { ...layout(), ...change } });
  const centers = [
    { id: "never", label: "Never" },
    { id: "on-overflow", label: "On overflow" },
    { id: "always", label: "Always" },
  ] as const;
  const widths = [
    { id: 1 / 3, label: "1/3" },
    { id: 0.5, label: "1/2" },
    { id: 2 / 3, label: "2/3" },
    { id: 1, label: "Full" },
  ] as const;
  return (
    <SettingsCard flush title="Tiling" description="niri's scrollable layout: how the strip follows focus.">
      <SettingsRowItem title="Center focused column"
        subtitle="Never scrolls the least; on overflow centers when two columns don't fit together."
        divider={false}
        control={<SegmentedControl value={center} choices={centers}
          onSelect={(centerFocusedColumn) => save({ centerFocusedColumn })} />} />
      <SettingsRowItem title="New column width" subtitle="niri default-column-width."
        control={<SegmentedControl value={width} choices={widths}
          onSelect={(defaultColumnWidth) => save({ defaultColumnWidth })} />} />
      <SettingsRowItem title="Center a lone column" subtitle="A single window sits in the middle of the screen."
        control={<ToggleSwitch checked={() => layout().alwaysCenterSingleColumn}
          onToggle={() => save({ alwaysCenterSingleColumn: !layout().alwaysCenterSingleColumn })} />} />
    </SettingsCard>
  );
}

function AppearanceSettings(): Element {
  const layout = settingsLayout;
  const swatches = ["appBg", "sidebarBg", "panelBg", "highlight"] as const;
  const densities = [
    { id: "comfortable", label: "Comfortable" },
    { id: "compact", label: "Compact" },
  ] as const;
  const setFontSize = (delta: number) => {
    const next = Math.min(18, Math.max(11, terminalFontSize.value + delta));
    if (next === terminalFontSize.value) return;
    terminalFontSize.value = next;
    updateConfig({ fontSize: next });
  };

  return (
    <Column style={{ gap: layout.sectionGap }}>
      <SettingsCard flush title="Themes" description="Load custom JSON themes from the themes folder."
        right={
          <Row style={{ gap: spacing.unit, shrink: 0 }}>
            <IconButton icon="refresh" label="Reload themes" onPress={reloadThemes} />
            <IconButton icon="folder-open" label="Open themes folder"
              detail={() => themesDirPath.value} onPress={openThemesDir} />
          </Row>
        }
      >
        <For each={() => themesList.value} key={(theme) => theme.id}>
          {(theme) => {
            const selected = () => activeThemeId.value === theme().id;
            return (
              <Row style={{ padding: layout.rowPadding, gap: layout.inset, align: "center" }}
                onPress={() => {
                  const mode = theme().mode === "light" ? "light" : "dark";
                  activeThemeId.value = theme().id;
                  themeMode.value = mode;
                  updateConfig({ theme: theme().id, mode });
                }}
                paint={() => ({
                  background: selected() ? palette().sidebarActive : 0,
                  edge: palette().border,
                  edgeWidth: themesList.value[0]?.id === theme().id ? 0 : spacing.hairline,
                  edgeSides: [1, 0, 0, 0],
                  hover: { background: palette().sidebarActive },
                  transitions: [smoothTransition("background", motion.fast)],
                })}
              >
                <Column style={{ grow: 1, minWidth: 0, gap: spacing.unit / 2 }}>
                  <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.body, weight: fontWeights.medium }}
                    paint={() => ({ color: palette().textPrimary })}>{() => theme().name}</Text>
                  <Text lines={1} font={{ family: fontFamilies.sans, size: typeScale.caption }}
                    paint={() => ({ color: palette().textMuted })}>{() => theme().author}</Text>
                </Column>
                <Row style={{ gap: spacing.unit, shrink: 0 }}>
                  {swatches.map((color) => (
                    <Box style={{ width: layout.swatchSize, height: layout.swatchSize }}
                      paint={() => ({ radius: radii.xs, background: theme().colors[color],
                        edge: palette().border, edgeWidth: spacing.hairline })} />
                  ))}
                </Row>
                <Box style={{ width: layout.icon, height: layout.icon, shrink: 0 }}
                  paint={() => ({ opacity: selected() ? 1 : 0, transitions: [smoothTransition("opacity", motion.fast)] })}>
                  <Glyph name="check" size={layout.icon} color={() => accent().brandText} />
                </Box>
              </Row>
            );
          }}
        </For>
      </SettingsCard>

      <TilingSettings />
      <SettingsCard flush title="Interface">
        <SettingsRowItem divider={false} title="Accent color" control={
          <Row style={{ gap: spacing.unit, align: "center" }}>
            {SETTINGS_ACCENTS.map((name) => (
              <HintTooltip label={name[0].toUpperCase() + name.slice(1)} side="top">
                <Box style={{ width: layout.accentSize, height: layout.accentSize, shrink: 0 }}
                  onPress={() => {
                    themeAccent.value = name;
                    updateConfig({ accent: name });
                  }}
                  paint={() => ({
                    radius: layout.accentSize / 2,
                    background: (themeMode.value === "dark" ? accentsDark : accentsLight)[name].brand,
                    edge: themeAccent.value === name ? palette().textPrimary : palette().border,
                    edgeWidth: themeAccent.value === name ? spacing.hairline * 3 : spacing.hairline,
                    hover: { scale: 1.12 },
                    transitions: [springTransition("scale"), smoothTransition("edge", motion.fast)],
                  })} />
              </HintTooltip>
            ))}
          </Row>
        } />
        <SettingsRowItem title="Interface density" control={
          <SegmentedControl value={densityMode} choices={densities}
            onSelect={(density) => updateConfig({ density })} />
        } />
        <SettingsRowItem title="Terminal font size" subtitle={fontFamilies.mono} control={
          <Row style={{ align: "center", gap: spacing.unit }}>
            <SecondaryButton label="−" height={spacing.control} onPress={() => setFontSize(-1)} />
            <Text font={{ family: fontFamilies.mono, size: typeScale.label }}
              paint={() => ({ color: palette().textPrimary })}>{() => `${terminalFontSize.value} px`}</Text>
            <SecondaryButton label="+" height={spacing.control} onPress={() => setFontSize(1)} />
          </Row>
        } />
      </SettingsCard>
    </Column>
  );
}

function SettingsView(): Element {
  return (
    <Column style={{ grow: 1, minHeight: 0, scroll: true }}>
      <Row style={{ justify: "center", padding: settingsLayout.pagePadding }}>
        <Column style={{ grow: 1, minWidth: 0, maxWidth: settingsLayout.pageWidth, gap: settingsLayout.sectionGap }}>
          <Text font={{ family: fontFamilies.sans, size: typeScale.displaySm, weight: fontWeights.medium }}
            paint={() => ({ color: palette().textPrimary })}>
            {() => currentSettingsPage().label}
          </Text>
        <Dynamic value={() => settingsTab.value}>
          {(current) => {
            if (current === "appearance") return <AppearanceSettings />;

            if (current === "clis") {
              return (
                <SettingsCard
                  flush
                  title="Configured agents"
                >
                  <For
                    each={() => configState.value.clis}
                    key={(cli) => cli.id}
                  >
                    {(cli) => (
                      <Row
                        style={{
                          padding: settingsLayout.rowPadding,
                          align: "center",
                          justify: "between",
                          gap: 10,
                        }}
                        paint={() => ({
                          edge: palette().border,
                          edgeWidth: configState.value.clis[0]?.id === cli().id ? 0 : spacing.hairline,
                          edgeSides: [1, 0, 0, 0],
                        })}
                      >
                        <Row style={{ align: "center", gap: 10, grow: 1, minWidth: 0 }}>
                          <Image
                            source={() => "cli:" + cli().id}
                            style={{ width: 20, height: 20, shrink: 0 }}
                          />
                          <Column style={{ gap: 2, grow: 1, minWidth: 0 }}>
                            <Row style={{ align: "center", gap: 8 }}>
                              <Text
                                font={{
                                  family: fontFamilies.sans,
                                  size: typeScale.label,
                                  weight: fontWeights.semibold,
                                }}
                                paint={() => ({ color: palette().textPrimary })}
                              >
                                {() => cli().name}
                              </Text>
                              <CountPill count={() => cli().command} />
                            </Row>
                            <Text
                              lines={1}
                              font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                              paint={() => ({ color: palette().textMuted })}
                            >
                              {() => cli().description}
                            </Text>
                          </Column>
                        </Row>
                        <Row style={{ align: "center", gap: 10, shrink: 0 }}>
                          <SecondaryButton
                            label="Launch"
                            height={26}
                            onPress={() => launchCli(cli().id)}
                          />
                          <ToggleSwitch
                            checked={() => cli().enabled}
                            onToggle={() =>
                              updateCliConfig(cli().id, { enabled: !cli().enabled })
                            }
                          />
                        </Row>
                      </Row>
                    )}
                  </For>
                </SettingsCard>
              );
            }

            if (current === "github") {
              const gh = () => githubState.value;
              return (
                <SettingsCard
                  flush
                  title="SSH connection"
                  right={
                    <SecondaryButton label="Re-detect SSH" height={26} onPress={refreshGitHub} />
                  }
                >
                  <Row
                    style={{
                      padding: settingsLayout.rowPadding,
                      align: "center",
                      justify: "between",
                      gap: 12,
                    }}
                    paint={() => ({
                      background: gh().connected
                        ? palette().successBg
                        : 0,
                    })}
                  >
                    <Row style={{ align: "center", gap: 10 }}>
                      <Image source="cli:github" style={{ width: 22, height: 22, shrink: 0 }} />
                      <Column style={{ gap: 2 }}>
                        <Text
                          font={{
                            family: fontFamilies.sans,
                            size: typeScale.label,
                            weight: fontWeights.semibold,
                          }}
                          paint={() => ({ color: palette().textPrimary })}
                        >
                          {() =>
                            gh().username
                              ? `Connected as @${gh().username} (${gh().method})`
                              : "GitHub SSH not authenticated"
                          }
                        </Text>
                        <Text
                          font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                          paint={() => ({ color: palette().textSecondary })}
                        >
                          {() => gh().detail}
                        </Text>
                      </Column>
                    </Row>
                    <CountPill
                      count={() =>
                        gh().sshGithubReady ? "SSH OK" : gh().connected ? gh().method : "off"
                      }
                    />
                  </Row>

                  <SettingsRowItem
                    title="Workspace repository"
                    subtitle={() => gh().remoteUrl ?? "No git remote detected"}
                    control={
                      <Row style={{ align: "center", gap: 6 }}>
                        <CountPill count={() => gh().repoSlug ?? "local"} />
                        <CountPill count={() => gh().branch ?? "—"} />
                      </Row>
                    }
                  />

                  <SettingsRowItem
                    title="Working tree changes"
                    subtitle="Modified and untracked files in the active repository."
                    control={<CountPill count={() => `${gh().dirtyCount} dirty`} />}
                  />

                  <SettingsRowItem
                    title="Local SSH keys (~/.ssh)"
                    subtitle={() =>
                      gh().sshKeys.length > 0
                        ? gh().sshKeys.join(", ")
                        : "No private keys found in ~/.ssh"
                    }
                    control={<CountPill count={() => String(gh().sshKeys.length)} />}
                  />
                </SettingsCard>
              );
            }

            if (current === "tiling") {
              const keys = (k: string[]) => (
                <Row style={{ gap: 3, shrink: 0 }}>
                  {k.map((label) => (
                    <KbdKey label={label} />
                  ))}
                </Row>
              );
              const groups = ["Views", "Sessions & tabs", "Windows", "Workspaces", "Layout", "Editor"];
              return (
                <Column style={{ gap: settingsLayout.sectionGap }}>
                  <Show when={() => keybindNotice.value.length > 0}
                    style={() => ({ shrink: 0, position: keybindNotice.value.length > 0 ? "relative" : "absolute" })}>
                    {() => (
                      <Row
                        style={{ height: 36, padding: [0, 12, 0, 12], align: "center", gap: 8 }}
                        paint={() => ({ radius: radii.lg, background: palette().warningBg, ...accordeeFadeUpMotion() })}
                      >
                        <Glyph name="alert" size={15} color={() => palette().warningText} />
                        <Text
                          style={{ grow: 1 }}
                          font={{ family: fontFamilies.sans, size: typeScale.label }}
                          paint={() => ({ color: palette().warningText })}
                        >
                          {() => keybindNotice.value}
                        </Text>
                      </Row>
                    )}
                  </Show>
                  {groups.map((group) => {
                    const rows = () => keybindsState.value.filter((keybind) => keybind.group === group);
                    return (
                      <Show when={() => rows().length > 0}>
                        {() => (
                          <SettingsCard flush title={group}>
                            <For each={rows} key={(keybind) => keybind.id}>
                              {(keybind) => <KeybindRow item={keybind} divider={() => rows()[0]?.id !== keybind().id} />}
                            </For>
                          </SettingsCard>
                        )}
                      </Show>
                    );
                  })}
                  <SettingsCard flush title="Image viewer" description="Fixed keys while the viewer is open.">
                    <SettingsRowItem divider={false} title="Previous / next" subtitle="Steps through the images on screen." control={keys(["←", "→"])} />
                    <SettingsRowItem title="Zoom" subtitle="Also the mouse wheel or a touchpad pinch." control={keys(["+", "−", "0"])} />
                    <SettingsRowItem title="Close" subtitle="Back to the terminal." control={keys(["Esc"])} />
                  </SettingsCard>
                </Column>
              );
            }

            // "updates"
            const up = () => updateState.value;
            return (
              <SettingsCard
                flush
                title="Software updates"
                description={() => `Installed icy v${up().currentVersion} · GitHub Releases`}
                right={
                  <Row style={{ gap: 6 }}>
                    <SecondaryButton label="Check now" height={26} onPress={checkForUpdates} />
                    <SecondaryButton
                      label="Release notes"
                      height={26}
                      onPress={() => {
                        releaseNotesOpen.value = true;
                      }}
                    />
                  </Row>
                }
              >
                <SettingsRowItem
                  divider={false}
                  title={() =>
                    up().hasUpdate
                      ? `Update v${up().availableVersion} available`
                      : `Up to date (v${up().currentVersion})`
                  }
                  subtitle={() => up().statusText}
                  control={
                    <PrimaryButton
                      label={() =>
                        up().stage === "ready"
                          ? "Install & Restart"
                          : up().stage === "downloading" || up().stage === "verifying"
                            ? `${up().progress}%`
                            : "Download"
                      }
                      height={28}
                      onPress={() => {
                        if (up().stage === "ready") {
                          restartConfirmOpen.value = true;
                        } else {
                          startUpdateDownload();
                        }
                      }}
                    />
                  }
                />
                <SettingsRowItem
                  title="Check for updates on startup"
                  subtitle="Query GitHub Releases in the background when icy starts."
                  control={
                    <ToggleSwitch
                      checked={() => configState.value.checkUpdates}
                      onToggle={() =>
                        updateConfig({ checkUpdates: !configState.value.checkUpdates })
                      }
                    />
                  }
                />
                <SettingsRowItem
                  title="Prompt to restart automatically"
                  subtitle="Shows a confirmation modal before closing sessions and replacing the binary."
                  control={
                    <ToggleSwitch
                      checked={() => configState.value.autoRestartUpdate}
                      onToggle={() =>
                        updateConfig({
                          autoRestartUpdate: !configState.value.autoRestartUpdate,
                        })
                      }
                    />
                  }
                />
              </SettingsCard>
            );
          }}
        </Dynamic>
        </Column>
      </Row>
    </Column>
  );
}
