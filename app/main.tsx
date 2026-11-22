// Root shell: Accordee modern frame with sidebar, left download progress ring,
// spring-animated session tab bar, Niri/Hyprland tiled PTY panes, functional sections,
// on-demand file viewer split pane, and Accordee-animated updater modals.

onHost(function (message: { view?: string; data?: any }) {
  if (message.view === "sessions") {
    sessionsState.value = message.data;
  } else if (message.view === "tiling") {
    tilingState.value = message.data;
  } else if (message.view === "screen") {
    screenState.value = message.data;
  } else if (message.view === "screens") {
    screensMapState.value = message.data ?? {};
  } else if (message.view === "viewer") {
    viewerState.value = message.data;
  } else if (message.view === "cliUsage") {
    cliUsageState.value = message.data;
  } else if (message.view === "config") {
    const cfg = message.data as ConfigView;
    configState.value = cfg;
    activeThemeId.value = cfg.theme;
    themeMode.value = cfg.mode === "light" ? "light" : "dark";
    themeAccent.value = cfg.accent;
    densityMode.value = cfg.density === "compact" ? "compact" : "comfortable";
    terminalFontSize.value = cfg.fontSize || 13;
  } else if (message.view === "themes") {
    themesDirPath.value = message.data?.themesDir ?? "~/.config/icy/themes";
    themesList.value = message.data?.items ?? [];
  } else if (message.view === "tasks") {
    tasksState.value = message.data?.items ?? [];
  } else if (message.view === "github") {
    githubState.value = message.data;
  } else if (message.view === "files") {
    filesState.value = message.data;
  } else if (message.view === "update") {
    const prevStage = updateState.value.stage;
    const next = message.data as UpdateView;
    updateState.value = next;
    // Automatically show the restart confirmation modal once a download finishes staging.
    if (
      prevStage !== "ready" &&
      next.stage === "ready" &&
      configState.value.autoRestartUpdate
    ) {
      installPopoverOpen.value = false;
      restartConfirmOpen.value = true;
    }
  } else if (message.view === "notice") {
    noticeState.value = message.data;
  } else if (message.view === "viewport" && Array.isArray(message.data)) {
    viewportSize.value = [Number(message.data[0]), Number(message.data[1])];
  }
});

function IcyApp(): Element {
  sendHello();

  return (
    <Box style={{ grow: 1 }} paint={() => ({ background: palette().sidebarBg })}>
      <Row style={{ grow: 1 }}>
        <Sidebar />

        {/* Accordee `html[data-layout=modern] .app-main`: inset main panel */}
        <Column
          style={() => ({
            grow: 1,
            minWidth: 0,
            minHeight: 0,
            margin: sidebarOpen.value ? [8, 8, 8, 0] : [8, 8, 8, 8],
            clip: true,
          })}
          paint={() => ({
            radius: radii.lg,
            background: palette().appBg,
            edge: palette().border,
            edgeWidth: spacing.hairline,
            shadows: [shadows.panel],
          })}
        >
          <TabBar />

          <Row style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
            <Dynamic value={() => activeNav.value} style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
              {(nav) => {
                if (nav === "files") return <FilesView />;
                if (nav === "tasks") return <TasksView />;
                if (nav === "metrics") return <MetricsView />;
                if (nav === "settings") return <SettingsView />;
                return <TilingWorkspaceView />;
              }}
            </Dynamic>
            <FileViewer />
          </Row>
        </Column>
      </Row>

      {/* Global Accordee-animated modal overlays */}
      <ReleaseNotesModal />
      <RestartConfirmModal />
    </Box>
  );
}

mount(IcyApp);
