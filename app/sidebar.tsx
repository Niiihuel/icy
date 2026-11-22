// Accordee-faithful sidebar with top update announcement, animated workspace/GitHub menu,
// functional sections, official SVGL CLI usage rings with Accordee tooltips, and Archived count.

function SidebarNavRow(props: {
  icon?: string;
  image?: string;
  label: Source<string>;
  active?: Source<boolean>;
  onPress: () => void;
  right?: Element;
}): Element {
  const isActive = () => read(props.active ?? false);
  return (
    <Row
      style={() => ({
        height: rowControlHeight(),
        padding: [0, 8, 0, 6],
        align: "center",
        justify: "between",
        gap: 8,
        shrink: 0,
      })}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: isActive() ? palette().sidebarActive : 0,
        hover: { background: palette().sidebarActive },
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Row style={{ align: "center", gap: 8, grow: 1, minWidth: 0 }}>
        {props.image ? (
          <Image source={props.image} style={{ width: 16, height: 16, shrink: 0 }} />
        ) : props.icon ? (
          <Glyph
            name={props.icon}
            size={15}
            color={() => (isActive() ? palette().textPrimary : palette().textSecondary)}
          />
        ) : null}
        <Text
          lines={1}
          font={{
            family: fontFamilies.sans,
            size: typeScale.label,
            weight: fontWeights.normal,
          }}
          paint={() => ({
            color: isActive() ? palette().textPrimary : palette().textSecondary,
          })}
        >
          {props.label}
        </Text>
      </Row>
      {props.right ?? null}
    </Row>
  );
}

function CliUsageRingButton(props: { cli: () => CliUsageItem }): Element {
  const open = signal(false);
  let anchorId: number | undefined;
  let hovering = false;
  let token = 0;

  return (
    <Box
      ref={(id) => {
        anchorId = id;
      }}
      style={{
        width: 28,
        height: 28,
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      onPress={() => launchCli(props.cli().id)}
      onHover={(inside) => {
        hovering = inside;
        const mine = ++token;
        if (!inside) {
          open.value = false;
          return;
        }
        after(80, () => {
          if (hovering && mine === token) open.value = true;
        });
      }}
      paint={() => ({
        radius: 14,
        hover: { scale: 1.1 },
        active: { scale: 0.95 },
        transitions: [smoothTransition("scale", motion.fast)],
      })}
    >
      <Image
        source={() => "ring:" + props.cli().id}
        style={{ width: 28, height: 28 }}
      />

      {/* Animated Accordee Tooltip */}
      <Popover
        open={open}
        anchor={() => anchorId}
        side="top"
        align="center"
        layer={1000}
        style={{ width: 192, padding: [10, 11, 10, 11] }}
        paint={() => ({
          radius: radii.popover,
          background: palette().cardBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
          shadows: [shadows.popover],
          ...accordeeTooltipMotion("top"),
        })}
      >
        <Column style={{ gap: 7 }}>
          <Row style={{ align: "center", justify: "between", gap: 6 }}>
            <Row style={{ align: "center", gap: 6 }}>
              <Image
                source={() => "cli:" + props.cli().id}
                style={{ width: 14, height: 14, shrink: 0 }}
              />
              <Text
                font={{
                  family: fontFamilies.sans,
                  size: typeScale.caption,
                  weight: fontWeights.medium,
                }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() => props.cli().name}
              </Text>
            </Row>
            <Text
              font={{
                family: fontFamilies.mono,
                size: typeScale.tiny,
                weight: fontWeights.medium,
              }}
              paint={() => ({ color: props.cli().colorRgba })}
            >
              {() => `${Math.round(props.cli().percent)}%`}
            </Text>
          </Row>

          {/* Animated usage bar */}
          <Box
            style={{ width: 170, height: 4, clip: true }}
            paint={() => ({
              radius: 2,
              background: palette().kbdBg,
            })}
          >
            <Box
              style={() => ({
                width: Math.max(0, Math.round((170 * props.cli().percent) / 100)),
                height: 4,
              })}
              paint={() => ({
                radius: 2,
                background: props.cli().colorRgba,
                transitions: [smoothTransition("width", motion.panel)],
              })}
            />
          </Box>

          <Row style={{ align: "center", justify: "between" }}>
            <Text
              font={{ family: fontFamilies.mono, size: typeScale.micro }}
              paint={() => ({ color: palette().textMuted })}
            >
              {() => `in ${props.cli().inputTokens} · out ${props.cli().outputTokens}`}
            </Text>
            <Text
              font={{ family: fontFamilies.sans, size: typeScale.micro }}
              paint={() => ({ color: palette().textSecondary })}
            >
              Click to launch
            </Text>
          </Row>
        </Column>
      </Popover>
    </Box>
  );
}

function Sidebar(): Element {
  const workspaceMenuOpen = signal(false);
  let workspaceAnchorId: number | undefined;

  const filteredSessions = () => {
    const q = searchQuery.value.trim().toLowerCase();
    const items = sessionsState.value.items;
    if (!q) return items;
    return items.filter(
      (s) =>
        s.title.toLowerCase().includes(q) ||
        s.cli.toLowerCase().includes(q) ||
        s.cwd.toLowerCase().includes(q)
    );
  };

  return (
    <Show when={sidebarOpen} style={{ shrink: 0 }}>
      {() => (
        <Column
          style={{
            width: spacing.sidebarWidth,
            grow: 1,
            shrink: 0,
            padding: [8, 8, 8, 8],
            gap: 8,
          }}
          paint={() => ({ background: palette().sidebarBg })}
        >
          {/* Update Announcement Banner ABOVE the account/workspace header */}
          <UpdateSidebarBanner />

          {/* Accordee Account / Workspace Header with animated menu */}
          <Row
            style={{
              height: 30,
              padding: [0, 4, 0, 4],
              align: "center",
              justify: "between",
              shrink: 0,
            }}
          >
            <Row
              ref={(id) => {
                workspaceAnchorId = id;
              }}
              style={{
                height: 28,
                padding: [0, 6, 0, 4],
                align: "center",
                gap: 7,
                grow: 1,
                minWidth: 0,
              }}
              onPress={() => {
                workspaceMenuOpen.value = !workspaceMenuOpen.value;
              }}
              paint={() => ({
                radius: radii.md,
                hover: { background: palette().sidebarActive },
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <Image source="brand:icy" style={{ width: 20, height: 20, shrink: 0 }} />
              <Text
                lines={1}
                font={{
                  family: fontFamilies.sans,
                  size: typeScale.label,
                  weight: fontWeights.semibold,
                }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() =>
                  githubState.value.username
                    ? `@${githubState.value.username}`
                    : "icy workspace"
                }
              </Text>
              <Glyph name="chevrons-up-down" size={12} color={() => palette().textMuted} />

              {/* Accordee-animated dropdown menu */}
              <Popover
                open={workspaceMenuOpen}
                anchor={() => workspaceAnchorId}
                side="bottom"
                align="start"
                layer={1050}
                style={{ width: 212, padding: [6, 6, 6, 6] }}
                paint={() => ({
                  radius: radii.popover,
                  background: palette().cardBg,
                  edge: palette().border,
                  edgeWidth: spacing.hairline,
                  shadows: [shadows.popover],
                  ...accordeeMenuMotion("bottom"),
                })}
              >
                <Column style={{ gap: 4 }}>
                  <Row
                    style={{ padding: [6, 8, 6, 8], align: "center", gap: 8 }}
                    paint={() => ({
                      radius: radii.md,
                      background: palette().panelBg,
                    })}
                  >
                    <Image source="cli:github" style={{ width: 16, height: 16, shrink: 0 }} />
                    <Column style={{ gap: 1, grow: 1, minWidth: 0 }}>
                      <Text
                        lines={1}
                        font={{
                          family: fontFamilies.sans,
                          size: typeScale.caption,
                          weight: fontWeights.medium,
                        }}
                        paint={() => ({ color: palette().textPrimary })}
                      >
                        {() => githubState.value.repoSlug ?? "Local repository"}
                      </Text>
                      <Text
                        lines={1}
                        font={{ family: fontFamilies.mono, size: typeScale.micro }}
                        paint={() => ({ color: palette().textMuted })}
                      >
                        {() =>
                          githubState.value.sshGithubReady
                            ? `SSH · ${githubState.value.branch ?? "main"}`
                            : `${githubState.value.method} · ${githubState.value.branch ?? "local"}`
                        }
                      </Text>
                    </Column>
                  </Row>

                  <SidebarNavRow
                    icon="sparkles"
                    label="Release notes"
                    onPress={() => {
                      workspaceMenuOpen.value = false;
                      releaseNotesOpen.value = true;
                    }}
                  />
                  <SidebarNavRow
                    icon="palette"
                    label="Custom themes"
                    onPress={() => {
                      workspaceMenuOpen.value = false;
                      activeNav.value = "settings";
                    }}
                  />
                  <SidebarNavRow
                    icon="refresh"
                    label="Re-detect GitHub SSH"
                    onPress={() => {
                      workspaceMenuOpen.value = false;
                      refreshGitHub();
                    }}
                  />
                </Column>
              </Popover>
            </Row>

            <Box
              style={{
                width: 24,
                height: 24,
                align: "center",
                justify: "center",
                shrink: 0,
              }}
              onPress={() => {
                sidebarOpen.value = false;
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
                size={15}
                color={() => palette().textMuted}
                hover={() => palette().textPrimary}
              />
            </Box>
          </Row>

          {/* Accordee Search Bar with ⌘ K */}
          <Row
            style={{
              height: 30,
              padding: [0, 6, 0, 8],
              align: "center",
              gap: 6,
              shrink: 0,
            }}
            paint={() => ({
              radius: radii.md,
              background: palette().appBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
            })}
          >
            <Glyph name="search" size={14} color={() => palette().textMuted} />
            <TextInput
              style={{ grow: 1, height: 24, padding: [3, 2, 2, 0] }}
              font={{ family: fontFamilies.sans, size: typeScale.label }}
              placeholder="Search"
              placeholderColor={palette().textMuted}
              onChange={(text) => {
                searchQuery.value = text;
              }}
              paint={() => ({ color: palette().textPrimary })}
            />
            <Row style={{ gap: 3, shrink: 0 }}>
              <KbdKey label="⌘" />
              <KbdKey label="K" />
            </Row>
          </Row>

          {/* Primary Navigation — every section is functional */}
          <Column style={{ gap: 2, shrink: 0 }}>
            <SidebarNavRow
              image="brand:agents"
              label="Agents"
              active={() => activeNav.value === "agents"}
              onPress={() => {
                activeNav.value = "agents";
              }}
              right={
                <Row style={{ gap: 3 }}>
                  <KbdKey label="⌘" />
                  <KbdKey label="Y" />
                </Row>
              }
            />
            <SidebarNavRow
              icon="plus"
              label="New column"
              onPress={() => {
                activeNav.value = "agents";
                openTiledColumn();
              }}
              right={
                <Row style={{ gap: 3 }}>
                  <KbdKey label="⌘" />
                  <KbdKey label="N" />
                </Row>
              }
            />
            <SidebarNavRow
              icon="folder"
              label="Files"
              active={() => activeNav.value === "files"}
              onPress={() => {
                activeNav.value = "files";
                listFiles(filesState.value.subdir);
              }}
              right={
                <CountPill
                  count={() =>
                    String(
                      detectedFilesCount() > 0
                        ? detectedFilesCount()
                        : filesState.value.entries.length
                    )
                  }
                />
              }
            />
            <SidebarNavRow
              icon="tasks"
              label="Tasks"
              active={() => activeNav.value === "tasks"}
              onPress={() => {
                activeNav.value = "tasks";
              }}
              right={<CountPill count={() => String(openTasksCount())} />}
            />
            <SidebarNavRow
              icon="dashboard"
              label="Metrics"
              active={() => activeNav.value === "metrics"}
              onPress={() => {
                activeNav.value = "metrics";
              }}
            />
          </Column>

          {/* Collapsible Section: Your Space (Active Sessions) */}
          <Row
            style={{
              padding: [6, 6, 0, 6],
              align: "center",
              justify: "between",
              shrink: 0,
            }}
          >
            <Row style={{ align: "center", gap: 4 }}>
              <Text
                font={{
                  family: fontFamilies.sans,
                  size: typeScale.tiny,
                  weight: fontWeights.normal,
                }}
                paint={() => ({ color: palette().textMuted })}
              >
                Your Space
              </Text>
              <Glyph name="chevron-down" size={11} color={() => palette().textMuted} />
            </Row>
            <CountPill count={() => String(sessionsState.value.items.length)} />
          </Row>

          <For
            each={filteredSessions}
            key={(item) => item.id}
            style={{ grow: 1, minHeight: 0, scroll: true, gap: 2 }}
          >
            {(item) => {
              const isActive = () =>
                activeNav.value === "agents" && sessionsState.value.activeId === item().id;
              return (
                <Row
                  style={() => ({
                    height: rowControlHeight(),
                    padding: [0, 8, 0, 6],
                    align: "center",
                    justify: "between",
                    gap: 8,
                  })}
                  onPress={() => {
                    activeNav.value = "agents";
                    selectSession(item().id);
                  }}
                  paint={() => ({
                    radius: radii.md,
                    background: isActive() ? palette().sidebarActive : 0,
                    hover: { background: palette().sidebarActive },
                    transitions: [smoothTransition("background", motion.fast)],
                  })}
                >
                  <Row style={{ align: "center", gap: 8, grow: 1, minWidth: 0 }}>
                    <Image
                      source={() => "cli:" + item().cli}
                      style={{ width: 15, height: 15, shrink: 0 }}
                    />
                    <Text
                      lines={1}
                      font={{
                        family: fontFamilies.sans,
                        size: typeScale.label,
                        weight: fontWeights.normal,
                      }}
                      paint={() => ({
                        color: isActive() ? palette().textPrimary : palette().textSecondary,
                      })}
                    >
                      {() => item().title}
                    </Text>
                  </Row>
                  <StatusDot alive={() => item().alive} />
                </Row>
              );
            }}
          </For>

          {/* Accordee Utility Rows (GitHub status + Settings) */}
          <Column style={{ gap: 2, shrink: 0 }}>
            <SidebarNavRow
              image="cli:github"
              label={() =>
                githubState.value.branch
                  ? `${githubState.value.branch}`
                  : "GitHub SSH"
              }
              onPress={() => {
                activeNav.value = "settings";
              }}
              right={
                <CountPill
                  count={() =>
                    githubState.value.sshGithubReady
                      ? "SSH"
                      : githubState.value.connected
                        ? githubState.value.method
                        : "off"
                  }
                />
              }
            />

            <SidebarNavRow
              icon="settings"
              label="Settings"
              active={() => activeNav.value === "settings"}
              onPress={() => {
                activeNav.value = "settings";
              }}
            />
          </Column>

          {/* Bottom Official SVGL CLI Usage Progress Rings Dock */}
          <Column
            style={{
              padding: [8, 2, 2, 2],
              gap: 8,
              shrink: 0,
            }}
            paint={() => ({
              edge: palette().border,
              edgeWidth: spacing.hairline,
              edgeSides: [1, 0, 0, 0],
            })}
          >
            <For
              each={() => cliUsageState.value}
              key={(cli) => cli.id}
              style={{
                direction: "row",
                align: "center",
                justify: "between",
                padding: [0, 2, 0, 2],
                height: 30,
              }}
            >
              {(cli) => <CliUsageRingButton cli={cli} />}
            </For>

            <Row
              style={{
                height: 24,
                padding: [0, 6, 0, 6],
                align: "center",
                justify: "between",
              }}
            >
              <Row style={{ align: "center", gap: 7 }}>
                <Glyph name="archive" size={13} color={() => palette().textMuted} />
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.caption }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  Archived
                </Text>
              </Row>
              <Text
                font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                paint={() => ({ color: palette().textMuted })}
              >
                {() => String(sessionsState.value.archivedCount)}
              </Text>
            </Row>
          </Column>
        </Column>
      )}
    </Show>
  );
}
