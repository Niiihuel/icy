// Functional workspace sections: Files explorer, persistent Tasks To-do manager,
// and Agent Usage Metrics dashboard (inspired by the user's Agent Usage reference).

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

function FilesView(): Element {
  const files = () => filesState.value;
  const parentSubdir = () => {
    const sub = files().subdir;
    if (!sub) return "";
    const parts = sub.split("/").filter((p) => p.length > 0);
    parts.pop();
    return parts.join("/");
  };

  return (
    <Column
      style={{
        grow: 1,
        minWidth: 0,
        minHeight: 0,
        padding: [16, 20, 16, 20],
        gap: 14,
      }}
      paint={() => accordeeFadeUpMotion()}
    >
      <Row style={{ align: "center", justify: "between", gap: 10 }}>
        <Column style={{ gap: 2 }}>
          <Text
            font={{
              family: fontFamilies.sans,
              size: typeScale.headingSm,
              weight: fontWeights.semibold,
            }}
            paint={() => ({ color: palette().textPrimary })}
          >
            Workspace Files
          </Text>
          <Text
            lines={1}
            font={{ family: fontFamilies.mono, size: typeScale.tiny }}
            paint={() => ({ color: palette().textMuted })}
          >
            {() => (files().subdir ? `${files().cwd}/${files().subdir}` : files().cwd)}
          </Text>
        </Column>

        <Row style={{ gap: 6 }}>
          <Show when={() => files().subdir.length > 0}>
            {() => (
              <SecondaryButton
                label="Up"
                height={28}
                onPress={() => listFiles(parentSubdir())}
              />
            )}
          </Show>
          <SecondaryButton
            label="Refresh"
            height={28}
            onPress={() => listFiles(files().subdir)}
          />
        </Row>
      </Row>

      {/* Recently detected clickable paths in active terminal */}
      <Show when={() => detectedFilesCount() > 0} style={{ shrink: 0 }}>
        {() => (
          <Column
            style={{ padding: [10, 12, 10, 12], gap: 6, shrink: 0 }}
            paint={() => ({
              radius: radii.lg,
              background: palette().panelBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
            })}
          >
            <Text
              font={{
                family: fontFamilies.sans,
                size: typeScale.tiny,
                weight: fontWeights.semibold,
              }}
              paint={() => ({ color: palette().textSecondary })}
            >
              Detected in terminal output (click to inspect)
            </Text>
            <For
              each={detectedFilesList}
              key={(lnk) => lnk.target}
              style={{ direction: "row", gap: 6, clip: true }}
            >
              {(lnk) => (
                <Box
                  style={{ padding: [4, 8, 4, 8] }}
                  onPress={() => openLink(lnk())}
                  paint={() => ({
                    radius: radii.md,
                    background: accent().brandSoft,
                    hover: { brightness: 1.15 },
                  })}
                >
                  <Text
                    font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                    paint={() => ({ color: accent().brandText })}
                  >
                    {() =>
                      lnk().line ? `${lnk().target}:${lnk().line}` : lnk().target
                    }
                  </Text>
                </Box>
              )}
            </For>
          </Column>
        )}
      </Show>

      {/* Directory listing */}
      <For
        each={() => files().entries}
        key={(entry) => entry.path}
        style={{ grow: 1, minHeight: 0, scroll: true, gap: 2 }}
      >
        {(entry) => (
          <Row
            style={() => ({
              height: rowControlHeight() + 2,
              padding: [0, 10, 0, 10],
              align: "center",
              justify: "between",
              gap: 10,
              shrink: 0,
            })}
            onPress={() => {
              if (entry().isDir) {
                listFiles(entry().path);
              } else {
                openLink({
                  kind: "file",
                  target: entry().path,
                  line: 1,
                  col: null,
                  resolved: true,
                });
              }
            }}
            paint={() => ({
              radius: radii.md,
              hover: { background: palette().sidebarActive },
              transitions: [smoothTransition("background", motion.fast)],
            })}
          >
            <Row style={{ align: "center", gap: 8, grow: 1, minWidth: 0 }}>
              <Glyph
                name={() => (entry().isDir ? "folder" : "file-code")}
                size={14}
                color={() =>
                  entry().isDir ? accent().brandText : palette().textSecondary
                }
              />
              <Text
                lines={1}
                font={{
                  family: fontFamilies.mono,
                  size: typeScale.caption,
                  weight: entry().isDir ? fontWeights.medium : fontWeights.normal,
                }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() => entry().name}
              </Text>
            </Row>
            <Text
              font={{ family: fontFamilies.mono, size: typeScale.micro }}
              paint={() => ({ color: palette().textMuted })}
            >
              {() => (entry().isDir ? "dir" : formatBytes(entry().size))}
            </Text>
          </Row>
        )}
      </For>
    </Column>
  );
}

function TasksView(): Element {
  const draft = signal("");
  const filter = signal<"all" | "open" | "done">("all");

  const filtered = () => {
    const items = tasksState.value;
    if (filter.value === "open") return items.filter((t) => !t.done);
    if (filter.value === "done") return items.filter((t) => t.done);
    return items;
  };

  function submitDraft(): void {
    const text = draft.value.trim();
    if (!text) return;
    addTask(text, "workspace");
    draft.value = "";
  }

  return (
    <Column
      style={{
        grow: 1,
        minWidth: 0,
        minHeight: 0,
        padding: [16, 20, 16, 20],
        gap: 14,
      }}
      paint={() => accordeeFadeUpMotion()}
    >
      <Row style={{ align: "center", justify: "between" }}>
        <Column style={{ gap: 2 }}>
          <Text
            font={{
              family: fontFamilies.sans,
              size: typeScale.headingSm,
              weight: fontWeights.semibold,
            }}
            paint={() => ({ color: palette().textPrimary })}
          >
            Tasks & Agent To-dos
          </Text>
          <Text
            font={{ family: fontFamilies.sans, size: typeScale.caption }}
            paint={() => ({ color: palette().textMuted })}
          >
            {() => `${openTasksCount()} open · persisted in ~/.config/icy/tasks.json`}
          </Text>
        </Column>

        <Row style={{ gap: 4 }}>
          {(["all", "open", "done"] as const).map((kind) => (
            <Box
              style={{ height: 26, padding: [0, 10, 0, 10], align: "center", justify: "center" }}
              onPress={() => {
                filter.value = kind;
              }}
              paint={() => ({
                radius: radii.md,
                background: filter.value === kind ? palette().sidebarActive : 0,
                hover: { background: palette().sidebarActive },
              })}
            >
              <Text
                font={{ family: fontFamilies.sans, size: typeScale.caption }}
                paint={() => ({
                  color:
                    filter.value === kind ? palette().textPrimary : palette().textMuted,
                })}
              >
                {kind}
              </Text>
            </Box>
          ))}
        </Row>
      </Row>

      {/* Add new task bar */}
      <Row
        style={{ height: 36, padding: [0, 6, 0, 12], align: "center", gap: 8, shrink: 0 }}
        paint={() => ({
          radius: radii.lg,
          background: palette().panelBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
        })}
      >
        <TextInput
          style={{ grow: 1, height: 26, padding: [4, 0, 2, 0] }}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          placeholder="Add a task for your next agent run..."
          placeholderColor={palette().textMuted}
          onChange={(text) => {
            draft.value = text;
          }}
          onSubmit={submitDraft}
          paint={() => ({ color: palette().textPrimary })}
        />
        <PrimaryButton label="Add Task" height={26} onPress={submitDraft} />
      </Row>

      {/* Task rows */}
      <For
        each={filtered}
        key={(item) => item.id}
        style={{ grow: 1, minHeight: 0, scroll: true, gap: 6 }}
      >
        {(item) => (
          <Row
            style={{
              padding: [10, 12, 10, 12],
              align: "center",
              justify: "between",
              gap: 10,
              shrink: 0,
            }}
            paint={() => ({
              radius: radii.lg,
              background: palette().cardBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
            })}
          >
            <Row
              style={{ align: "center", gap: 10, grow: 1, minWidth: 0 }}
              onPress={() => toggleTask(item().id)}
            >
              {/* Accordee checkbox */}
              <Box
                style={{ width: 16, height: 16, align: "center", justify: "center", shrink: 0 }}
                paint={() => ({
                  radius: 4,
                  background: item().done ? accent().brand : 0,
                  edge: item().done ? accent().brand : palette().border,
                  edgeWidth: 1,
                })}
              >
                <Show when={() => item().done}>
                  {() => <Glyph name="check" size={11} color={() => accent().brandFg} />}
                </Show>
              </Box>
              <Text
                lines={1}
                font={{ family: fontFamilies.sans, size: typeScale.label }}
                paint={() => ({
                  color: item().done ? palette().textMuted : palette().textPrimary,
                })}
              >
                {() => item().title}
              </Text>
            </Row>

            <Row style={{ align: "center", gap: 8, shrink: 0 }}>
              <CountPill count={() => item().tag} />
              <Box
                style={{ width: 22, height: 22, align: "center", justify: "center" }}
                onPress={() => deleteTask(item().id)}
                paint={() => ({
                  radius: radii.sm,
                  hover: { background: palette().sidebarActive },
                })}
              >
                <Glyph name="trash" size={12} color={() => palette().textMuted} />
              </Box>
            </Row>
          </Row>
        )}
      </For>
    </Column>
  );
}

function MetricsView(): Element {
  const activeClisCount = () =>
    cliUsageState.value.filter((c) => c.sessions > 0).length;
  const totalSessions = () =>
    cliUsageState.value.reduce((acc, c) => acc + c.sessions, 0);

  const kpiCard = (label: string, value: () => string, sub: () => string): Element => (
    <Column
      style={{ grow: 1, padding: [14, 16, 14, 16], gap: 4 }}
      paint={() => ({
        radius: radii.xl,
        background: palette().cardBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        ...accordeeFadeUpMotion(),
      })}
    >
      <Text
        font={{
          family: fontFamilies.sans,
          size: typeScale.micro,
          weight: fontWeights.semibold,
        }}
        paint={() => ({ color: palette().textMuted })}
      >
        {label}
      </Text>
      <Text
        font={{
          family: fontFamilies.sans,
          size: typeScale.metric,
          weight: fontWeights.semibold,
        }}
        paint={() => ({ color: palette().textPrimary })}
      >
        {value}
      </Text>
      <Text
        font={{ family: fontFamilies.mono, size: typeScale.micro }}
        paint={() => ({ color: palette().textSecondary })}
      >
        {sub}
      </Text>
    </Column>
  );

  return (
    <Column
      style={{
        grow: 1,
        minWidth: 0,
        minHeight: 0,
        scroll: true,
        padding: [16, 20, 20, 20],
        gap: 16,
      }}
    >
      <Row style={{ align: "center", justify: "between" }}>
        <Column style={{ gap: 2 }}>
          <Text
            font={{
              family: fontFamilies.sans,
              size: typeScale.headingSm,
              weight: fontWeights.semibold,
            }}
            paint={() => ({ color: palette().textPrimary })}
          >
            Agent Usage
          </Text>
          <Text
            font={{ family: fontFamilies.sans, size: typeScale.caption }}
            paint={() => ({ color: palette().textMuted })}
          >
            Live local telemetry scanned from Claude Code, Antigravity, Codex, Grok, Junie & OpenCode.
          </Text>
        </Column>
        <CountPill count={() => `${tilingState.value.mode} tiling`} />
      </Row>

      {/* 4 KPI Summary Cards */}
      <Row style={{ gap: 10, shrink: 0 }}>
        {kpiCard(
          "TOTAL CLI SESSIONS",
          () => String(totalSessions()),
          () => `${activeClisCount()} active providers`
        )}
        {kpiCard(
          "LIVE TERMINALS",
          () => String(sessionsState.value.items.length),
          () => `${tilingState.value.totalCols} tiled columns`
        )}
        {kpiCard(
          "OPEN TASKS",
          () => String(openTasksCount()),
          () => `${tasksState.value.length} total tracked`
        )}
        {kpiCard(
          "ARCHIVED RUNS",
          () => String(sessionsState.value.archivedCount),
          () => (githubState.value.branch ? `branch ${githubState.value.branch}` : "local workspace")
        )}
      </Row>

      {/* Per-Agent Breakdown with official SVGL logos + animated usage bars */}
      <Column
        style={{ padding: [16, 18, 16, 18], gap: 12, shrink: 0 }}
        paint={() => ({
          radius: radii.xl,
          background: palette().cardBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
        })}
      >
        <Row style={{ align: "center", justify: "between" }}>
          <Text
            font={{
              family: fontFamilies.sans,
              size: typeScale.caption,
              weight: fontWeights.semibold,
            }}
            paint={() => ({ color: palette().textSecondary })}
          >
            AGENTS
          </Text>
          <Text
            font={{ family: fontFamilies.mono, size: typeScale.micro }}
            paint={() => ({ color: palette().textMuted })}
          >
            click any CLI to launch in a tiled column
          </Text>
        </Row>

        <For each={() => cliUsageState.value} key={(cli) => cli.id} style={{ gap: 8 }}>
          {(cli) => (
            <Row
              style={{
                padding: [8, 10, 8, 10],
                align: "center",
                justify: "between",
                gap: 12,
              }}
              onPress={() => launchCli(cli().id)}
              paint={() => ({
                radius: radii.md,
                hover: { background: palette().sidebarActive },
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <Row style={{ width: 150, align: "center", gap: 8, shrink: 0 }}>
                <Image
                  source={() => "cli:" + cli().id}
                  style={{ width: 16, height: 16, shrink: 0 }}
                />
                <Text
                  font={{
                    family: fontFamilies.sans,
                    size: typeScale.label,
                    weight: fontWeights.medium,
                  }}
                  paint={() => ({ color: palette().textPrimary })}
                >
                  {() => cli().name}
                </Text>
              </Row>

              <Box
                style={{ grow: 1, height: 6, clip: true }}
                paint={() => ({
                  radius: 3,
                  background: palette().kbdBg,
                })}
              >
                <Box
                  style={() => ({
                    width: Math.max(4, Math.round((260 * cli().percent) / 100)),
                    height: 6,
                  })}
                  paint={() => ({
                    radius: 3,
                    background: cli().colorRgba,
                    transitions: [smoothTransition("width", motion.panel)],
                  })}
                />
              </Box>

              <Row style={{ width: 210, justify: "end", gap: 12, shrink: 0 }}>
                <Text
                  font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                  paint={() => ({ color: palette().textSecondary })}
                >
                  {() => `in ${cli().inputTokens}`}
                </Text>
                <Text
                  font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {() => `out ${cli().outputTokens}`}
                </Text>
                <Text
                  font={{
                    family: fontFamilies.mono,
                    size: typeScale.tiny,
                    weight: fontWeights.medium,
                  }}
                  paint={() => ({ color: cli().colorRgba })}
                >
                  {() => `${Math.round(cli().percent)}%`}
                </Text>
              </Row>
            </Row>
          )}
        </For>
      </Column>
    </Column>
  );
}
