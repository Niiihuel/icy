// Accordee two-column Settings panel: Appearance & Custom JSNIR Themes, CLIs & Agents,
// GitHub SSH Integration, Niri/Hyprland Tiling, and Updates.

type SettingsTab = "appearance" | "clis" | "github" | "tiling" | "updates";

const SETTINGS_ACCENTS: AccentName[] = [
  "neutral",
  "blue",
  "violet",
  "rose",
  "orange",
  "green",
  "teal",
];

function SettingsCard(props: {
  title: Source<string>;
  description?: Source<string>;
  right?: Element;
  children: Element | Element[];
}): Element {
  return (
    <Column
      style={{ padding: [14, 16, 14, 16], gap: 12, shrink: 0 }}
      paint={() => ({
        radius: radii.xl,
        background: palette().cardBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        ...accordeeFadeUpMotion(),
      })}
    >
      <Row style={{ align: "center", justify: "between", gap: 10 }}>
        <Column style={{ gap: 2, grow: 1, minWidth: 0 }}>
          <Text
            font={{
              family: fontFamilies.sans,
              size: typeScale.body,
              weight: fontWeights.semibold,
            }}
            paint={() => ({ color: palette().textPrimary })}
          >
            {props.title}
          </Text>
          {props.description ? (
            <Text
              font={{ family: fontFamilies.sans, size: typeScale.caption }}
              paint={() => ({ color: palette().textMuted })}
            >
              {props.description}
            </Text>
          ) : null}
        </Column>
        {props.right ?? null}
      </Row>
      {props.children}
    </Column>
  );
}

function SettingsRowItem(props: {
  title: Source<string>;
  subtitle: Source<string>;
  control: Element;
}): Element {
  return (
    <Row
      style={{
        padding: [8, 0, 8, 0],
        align: "center",
        justify: "between",
        gap: 12,
      }}
      paint={() => ({
        edge: palette().border,
        edgeWidth: spacing.hairline,
        edgeSides: [1, 0, 0, 0],
      })}
    >
      <Column style={{ gap: 2, grow: 1, minWidth: 0 }}>
        <Text
          font={{
            family: fontFamilies.sans,
            size: typeScale.label,
            weight: fontWeights.medium,
          }}
          paint={() => ({ color: palette().textPrimary })}
        >
          {props.title}
        </Text>
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.tiny }}
          paint={() => ({ color: palette().textMuted })}
        >
          {props.subtitle}
        </Text>
      </Column>
      {props.control}
    </Row>
  );
}

function SettingsView(): Element {
  const tab = signal<SettingsTab>("appearance");

  const navItem = (id: SettingsTab, icon: string, label: string): Element => {
    const active = () => tab.value === id;
    return (
      <Row
        style={() => ({
          height: rowControlHeight(),
          padding: [0, 10, 0, 10],
          align: "center",
          gap: 8,
          shrink: 0,
        })}
        onPress={() => {
          tab.value = id;
        }}
        paint={() => ({
          radius: radii.md,
          background: active() ? palette().sidebarActive : 0,
          hover: { background: palette().sidebarActive },
          transitions: [smoothTransition("background", motion.fast)],
        })}
      >
        <Glyph
          name={icon}
          size={14}
          color={() => (active() ? palette().textPrimary : palette().textMuted)}
        />
        <Text
          font={{
            family: fontFamilies.sans,
            size: typeScale.label,
            weight: fontWeights.medium,
          }}
          paint={() => ({
            color: active() ? palette().textPrimary : palette().textSecondary,
          })}
        >
          {label}
        </Text>
      </Row>
    );
  };

  return (
    <Row style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      {/* Left Settings Category Rail */}
      <Column
        style={{
          width: 196,
          shrink: 0,
          padding: [14, 10, 14, 12],
          gap: 4,
        }}
        paint={() => ({
          edge: palette().border,
          edgeWidth: spacing.hairline,
          edgeSides: [0, 1, 0, 0],
        })}
      >
        <Text
          style={{ padding: [0, 8, 8, 8] }}
          font={{
            family: fontFamilies.sans,
            size: typeScale.subtitle,
            weight: fontWeights.semibold,
          }}
          paint={() => ({ color: palette().textPrimary })}
        >
          Settings
        </Text>
        {navItem("appearance", "palette", "Themes & Visuals")}
        {navItem("clis", "terminal", "CLIs & Agents")}
        {navItem("github", "git-branch", "GitHub & SSH")}
        {navItem("tiling", "columns", "Tiling & Layout")}
        {navItem("updates", "download", "Updates")}
      </Column>

      {/* Right Scrollable Settings Body */}
      <Column
        style={{
          grow: 1,
          minWidth: 0,
          minHeight: 0,
          scroll: true,
          padding: [16, 22, 24, 22],
          gap: 14,
        }}
      >
        <Dynamic value={() => tab.value}>
          {(current) => {
            if (current === "appearance") {
              return (
                <Column style={{ gap: 14 }}>
                  <SettingsCard
                    title="Custom JSNIR Themes"
                    description={() =>
                      `Edit or drop any .json theme file in ${themesDirPath.value} and click Reload.`
                    }
                    right={
                      <Row style={{ gap: 6 }}>
                        <SecondaryButton label="Reload" height={26} onPress={reloadThemes} />
                        <SecondaryButton label="Open Folder" height={26} onPress={openThemesDir} />
                      </Row>
                    }
                  >
                    <For
                      each={() => themesList.value}
                      key={(item) => item.id}
                      style={{ gap: 8 }}
                    >
                      {(item) => {
                        const selected = () => activeThemeId.value === item().id;
                        return (
                          <Row
                            style={{
                              padding: [10, 12, 10, 12],
                              align: "center",
                              justify: "between",
                              gap: 10,
                            }}
                            onPress={() => {
                              activeThemeId.value = item().id;
                              themeMode.value = item().mode === "light" ? "light" : "dark";
                              updateConfig({
                                theme: item().id,
                                mode: item().mode === "light" ? "light" : "dark",
                              });
                            }}
                            paint={() => ({
                              radius: radii.lg,
                              background: selected()
                                ? palette().sidebarActive
                                : palette().panelBg,
                              edge: selected() ? accent().brand : palette().border,
                              edgeWidth: selected() ? 1 : spacing.hairline,
                              hover: { background: palette().sidebarActive },
                              transitions: [
                                smoothTransition("background", motion.fast),
                                smoothTransition("edge", motion.fast),
                              ],
                            })}
                          >
                            <Row style={{ align: "center", gap: 10 }}>
                              {/* 4-swatch preview pill */}
                              <Row style={{ gap: 3 }}>
                                <Box
                                  style={{ width: 14, height: 14 }}
                                  paint={() => ({
                                    radius: 4,
                                    background: item().colors.appBg,
                                    edge: palette().border,
                                    edgeWidth: 0.5,
                                  })}
                                />
                                <Box
                                  style={{ width: 14, height: 14 }}
                                  paint={() => ({
                                    radius: 4,
                                    background: item().colors.sidebarBg,
                                    edge: palette().border,
                                    edgeWidth: 0.5,
                                  })}
                                />
                                <Box
                                  style={{ width: 14, height: 14 }}
                                  paint={() => ({
                                    radius: 4,
                                    background: item().colors.panelBg,
                                    edge: palette().border,
                                    edgeWidth: 0.5,
                                  })}
                                />
                                <Box
                                  style={{ width: 14, height: 14 }}
                                  paint={() => ({
                                    radius: 4,
                                    background: item().colors.highlight,
                                  })}
                                />
                              </Row>
                              <Column style={{ gap: 1 }}>
                                <Text
                                  font={{
                                    family: fontFamilies.sans,
                                    size: typeScale.label,
                                    weight: fontWeights.medium,
                                  }}
                                  paint={() => ({ color: palette().textPrimary })}
                                >
                                  {() => item().name}
                                </Text>
                                <Text
                                  font={{ family: fontFamilies.mono, size: typeScale.micro }}
                                  paint={() => ({ color: palette().textMuted })}
                                >
                                  {() => `${item().mode} · ${item().author}`}
                                </Text>
                              </Column>
                            </Row>
                            <Show when={selected}>
                              {() => (
                                <Glyph name="check" size={14} color={() => accent().brandText} />
                              )}
                            </Show>
                          </Row>
                        );
                      }}
                    </For>
                  </SettingsCard>

                  <SettingsCard
                    title="Accent, Density & Typography"
                    description="Fine-tune Accordee accents, UI spacing density, and terminal font size."
                  >
                    <SettingsRowItem
                      title="Accent color"
                      subtitle="Used for focused tiling borders, active indicators, and primary actions."
                      control={
                        <Row style={{ gap: 6 }}>
                          {SETTINGS_ACCENTS.map((name) => (
                            <Box
                              style={{ width: 20, height: 20, shrink: 0 }}
                              onPress={() => {
                                themeAccent.value = name;
                                updateConfig({ accent: name });
                              }}
                              paint={() => ({
                                radius: 10,
                                background:
                                  themeMode.value === "dark"
                                    ? accentsDark[name].brand
                                    : accentsLight[name].brand,
                                edge:
                                  themeAccent.value === name
                                    ? palette().textPrimary
                                    : palette().border,
                                edgeWidth: themeAccent.value === name ? 1.5 : 0.5,
                                hover: { scale: 1.15 },
                                transitions: [smoothTransition("scale", motion.fast)],
                              })}
                            />
                          ))}
                        </Row>
                      }
                    />
                    <SettingsRowItem
                      title="Interface density"
                      subtitle="Compact shrinks row heights to fit more agents and files on screen."
                      control={
                        <Row style={{ gap: 6 }}>
                          <SecondaryButton
                            label="Comfortable"
                            height={26}
                            onPress={() => {
                              densityMode.value = "comfortable";
                              updateConfig({ density: "comfortable" });
                            }}
                          />
                          <SecondaryButton
                            label="Compact"
                            height={26}
                            onPress={() => {
                              densityMode.value = "compact";
                              updateConfig({ density: "compact" });
                            }}
                          />
                        </Row>
                      }
                    />
                    <SettingsRowItem
                      title="Terminal font size"
                      subtitle="Geist Mono size in logical pixels (recomputes PTY columns/rows live)."
                      control={
                        <Row style={{ align: "center", gap: 8 }}>
                          <SecondaryButton
                            label="-"
                            height={26}
                            onPress={() => {
                              const next = Math.max(11, terminalFontSize.value - 1);
                              terminalFontSize.value = next;
                              updateConfig({ fontSize: next });
                            }}
                          />
                          <Text
                            font={{ family: fontFamilies.mono, size: typeScale.label }}
                            paint={() => ({ color: palette().textPrimary })}
                          >
                            {() => `${terminalFontSize.value}px`}
                          </Text>
                          <SecondaryButton
                            label="+"
                            height={26}
                            onPress={() => {
                              const next = Math.min(18, terminalFontSize.value + 1);
                              terminalFontSize.value = next;
                              updateConfig({ fontSize: next });
                            }}
                          />
                        </Row>
                      }
                    />
                  </SettingsCard>
                </Column>
              );
            }

            if (current === "clis") {
              return (
                <SettingsCard
                  title="Agent CLIs (herdr-style)"
                  description="Enable, configure, and launch terminal agents directly in new tiled columns."
                >
                  <For
                    each={() => configState.value.clis}
                    key={(cli) => cli.id}
                    style={{ gap: 8 }}
                  >
                    {(cli) => (
                      <Row
                        style={{
                          padding: [10, 12, 10, 12],
                          align: "center",
                          justify: "between",
                          gap: 10,
                        }}
                        paint={() => ({
                          radius: radii.lg,
                          background: palette().panelBg,
                          edge: palette().border,
                          edgeWidth: spacing.hairline,
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
                  title="GitHub SSH Integration"
                  description="Automatically detects ~/.ssh keys, git@github.com handshake, and workspace repository."
                  right={
                    <SecondaryButton label="Re-detect SSH" height={26} onPress={refreshGitHub} />
                  }
                >
                  <Row
                    style={{
                      padding: [12, 14, 12, 14],
                      align: "center",
                      justify: "between",
                      gap: 12,
                    }}
                    paint={() => ({
                      radius: radii.lg,
                      background: gh().connected
                        ? palette().successBg
                        : palette().panelBg,
                      edge: palette().border,
                      edgeWidth: spacing.hairline,
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
              const mode = () => tilingState.value.mode;
              return (
                <Column style={{ gap: 14 }}>
                  <SettingsCard
                    title="Window Tiling Engine"
                    description="Choose between Niri scrollable columns or Hyprland automatic dynamic splits."
                  >
                    <Row style={{ gap: 10 }}>
                      <Column
                        style={{ grow: 1, padding: [12, 14, 12, 14], gap: 4 }}
                        onPress={() => setTilingMode("niri")}
                        paint={() => ({
                          radius: radii.lg,
                          background:
                            mode() === "niri" ? palette().sidebarActive : palette().panelBg,
                          edge: mode() === "niri" ? accent().brand : palette().border,
                          edgeWidth: mode() === "niri" ? 1 : spacing.hairline,
                        })}
                      >
                        <Text
                          font={{
                            family: fontFamilies.sans,
                            size: typeScale.label,
                            weight: fontWeights.semibold,
                          }}
                          paint={() => ({ color: palette().textPrimary })}
                        >
                          Niri · Scrollable Columns
                        </Text>
                        <Text
                          font={{ family: fontFamilies.sans, size: typeScale.tiny, lineHeight: 1.4 }}
                          paint={() => ({ color: palette().textMuted })}
                        >
                          2 wide columns on screen; extra agents scroll horizontally in an infinite strip.
                        </Text>
                      </Column>

                      <Column
                        style={{ grow: 1, padding: [12, 14, 12, 14], gap: 4 }}
                        onPress={() => setTilingMode("hyprland")}
                        paint={() => ({
                          radius: radii.lg,
                          background:
                            mode() === "hyprland" ? palette().sidebarActive : palette().panelBg,
                          edge: mode() === "hyprland" ? accent().brand : palette().border,
                          edgeWidth: mode() === "hyprland" ? 1 : spacing.hairline,
                        })}
                      >
                        <Text
                          font={{
                            family: fontFamilies.sans,
                            size: typeScale.label,
                            weight: fontWeights.semibold,
                          }}
                          paint={() => ({ color: palette().textPrimary })}
                        >
                          Hyprland · Dynamic Splits
                        </Text>
                        <Text
                          font={{ family: fontFamilies.sans, size: typeScale.tiny, lineHeight: 1.4 }}
                          paint={() => ({ color: palette().textMuted })}
                        >
                          Shows up to 3 columns side-by-side with vertical stacking inside each column.
                        </Text>
                      </Column>
                    </Row>
                  </SettingsCard>

                  <SettingsCard
                    title="Keyboard Shortcuts"
                    description="Works directly while typing in any terminal session."
                  >
                    <SettingsRowItem
                      title="New tiled column"
                      subtitle="Opens a new PTY column next to the focused pane."
                      control={<KbdKey label="Ctrl+Shift+N" />}
                    />
                    <SettingsRowItem
                      title="Split vertically in column"
                      subtitle="Stacks a second terminal pane below the focused pane."
                      control={<KbdKey label="Ctrl+Shift+D" />}
                    />
                    <SettingsRowItem
                      title="Move focus between panes"
                      subtitle="Directional focus across columns and stacked panes."
                      control={<KbdKey label="Alt + H/J/K/L" />}
                    />
                    <SettingsRowItem
                      title="Toggle pane maximize"
                      subtitle="Expand the focused terminal full-width, or restore tiling."
                      control={<KbdKey label="Ctrl+Shift+M" />}
                    />
                  </SettingsCard>
                </Column>
              );
            }

            // "updates"
            const up = () => updateState.value;
            return (
              <SettingsCard
                title="Software Updates"
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
  );
}
