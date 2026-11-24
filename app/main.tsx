// Root shell: Accordee modern frame (sidebar + inset main panel with header, tabs and the
// compositor tiling) or the full-window Accordee settings screen.

let lastAgentShortcutCount = -1;
effect(() => {
  const count = agentShortcutCount();
  if (count !== lastAgentShortcutCount && requestHost({ type: "agentShortcutCount", count })) lastAgentShortcutCount = count;
});

// Keyboard goes to a focused browser pane's page; any other focus releases it (id 0).
let lastBrowserFocus = -1;
effect(() => {
  const id = focusedTile()?.kind === "browser" ? tilingState.value.focusedSession : 0;
  if (id !== lastBrowserFocus && requestHost({ type: "browser", op: "focus", id })) lastBrowserFocus = id;
});

onHost(function (message: { view?: string; data?: any }) {
  if (message.view === "sessions") {
    sessionsState.value = message.data;
  } else if (message.view === "tiling") {
    noteTilingChange(tilingState.value, message.data);
    tilingState.value = message.data;
    workspaceReady.value = true;
    pruneTiles();
  } else if (message.view === "workspace") {
    setWorkspacePath(message.data);
  } else if (message.view === "terminalFrame") {
    const { sequence, screens, contextId, contextScreen } = message.data;
    const next = mergeScreens(screensMapState.value, screens);
    if (next !== screensMapState.value) screensMapState.value = next;
    const context = next[String(contextId)]
      ?? (contextScreen ? mergeScreen(screenState.value, contextScreen) : emptyScreen);
    if (context !== screenState.value) screenState.value = context;
    requestHost({ type: "terminalFrameReceived", sequence });
  } else if (message.view === "screen") {
    const next = message.data ? mergeScreen(screenState.value, message.data) : emptyScreen;
    if (next !== screenState.value) screenState.value = next;
  } else if (message.view === "screens") {
    const next = mergeScreens(screensMapState.value, message.data ?? {});
    if (next !== screensMapState.value) screensMapState.value = next;
  } else if (message.view === "browserState") {
    receiveBrowserState(message.data);
  } else if (message.view === "viewer") {
    receiveDoc(message.data);
  } else if (message.view === "editorSaved") {
    receiveEditorSaved(message.data);
  } else if (message.view === "editorSaveKey") {
    const spec = focusedTile();
    if (spec?.kind === "file") saveEditorDraft(editorDrafts.get(docKey(spec)));
  } else if (message.view === "gitBlame") {
    receiveGitBlame(message.data);
  } else if (message.view === "cliUsage") {
    cliUsageState.value = message.data;
  } else if (message.view === "horizontalScroll") {
    scrollHoveredStrip(message.data.delta, message.data.smooth);
  } else if (message.view === "metricsPointer") {
    updateMetricsPointer(message.data);
  } else if (message.view === "releaseGlowRect") {
    releaseGlowRect.value = message.data;
  } else if (message.view === "metricsUsage") {
    metricsUsageState.value = message.data;
    metricsScanState.value = "ready";
  } else if (message.view === "metricsUsageError") {
    metricsScanState.value = "error";
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
  } else if (message.view === "fileImage") {
    const d = message.data as { path: string; key?: string; width?: number; height?: number; error?: boolean };
    fileImages.value = {
      ...fileImages.value,
      [d.path]: d.error || !d.key ? "error" : { key: d.key, width: d.width ?? 0, height: d.height ?? 0 },
    };
  } else if (message.view === "codePreview") {
    receiveCodePreview(message.data as CodePreviewEntry);
  } else if (message.view === "keybinds") {
    keybindsState.value = message.data.items ?? [];
    const taken = (message.data.taken ?? []) as string[];
    keybindNotice.value = taken.length > 0 ? `Removed from ${taken.join(", ")}` : "";
  } else if (message.view === "capturedKey") {
    const id = keybindRecording.value;
    const chord = String(message.data ?? "");
    keybindRecording.value = null;
    // Same rule as the host: plain keys would steal typing from the terminal.
    if (id && chord && !/(^|\+)(Ctrl|Alt)\+|(^|\+)F\d+$/.test(chord)) {
      keybindNotice.value = `${chord} needs Ctrl or Alt`;
    } else if (id && chord) requestHost({ type: "setKeybind", id, chord });
  } else if (message.view === "gitGraph") {
    receiveGitGraph(message.data as GitGraphView);
  } else if (message.view === "fileSearch") {
    fileSearch.value = message.data;
  } else if (message.view === "agentDigitsHeld") {
    setAgentDigitsHeld(!!message.data);
  } else if (message.view === "agentDigit") {
    if (agentShortcutCount() > 0) handleAgentDigit(Number(message.data));
  } else if (message.view === "modalKey") {
    const action = String(message.data);
    if (projectDialog.value) {
      // The dialog owns Escape: its agent step goes back to the folder step.
      if (projectDialogKeyHandler) projectDialogKeyHandler(action);
      else if (action === "escape") closeProjectDialog();
    } else if (paletteRequest.value) {
      if (action === "escape") closePalette();
      else if (action === "panel.side") togglePalette("launcher");
      else if (action === "session.search") togglePalette("sessions");
      else paletteKeyHandler?.(action);
    } else if (imageViewer.value) {
      handleModalKey(action);
    } else if (action === "session.search") {
      togglePalette("sessions");
    } else if (action === "session.new") createSession();
    else if (action === "column.new") openTiledColumn();
    else if (action === "pane.split") stackTiledVertical();
    else if (action === "panel:overview" || action === "panel.overview") toggleOverview();
    else if (action === "panel:side" || action === "panel.side") togglePalette("launcher");
    else if (action === "panel.files") toggleFiles();
    else if (action === "view:overview" || action === "view.overview") toggleOverviewMode();
    else if (action === "editor.addToChat") addFocusedSelectionToChat();
    else handleOverviewKey(action);
  } else if (message.view === "avatar") {
    avatarReady.value = !!message.data?.ready;
  } else if (message.view === "changes") {
    const { cwd, changes } = message.data as { cwd: string; changes: ChangeSetView };
    receiveChanges(cwd, changes);
  } else if (message.view === "editors") {
    externalEditorsState.value = message.data ?? [];
  } else if (message.view === "github") {
    githubState.value = message.data;
  } else if (message.view === "files") {
    receiveFiles(message.data);
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
      if (paletteRequest.value) closePalette();
      restartConfirmOpen.value = true;
    }
  } else if (message.view === "notice") {
    noticeState.value = message.data;
  } else if (message.view === "viewport" && Array.isArray(message.data)) {
    viewportSize.value = [Number(message.data[0]), Number(message.data[1])];
  }
});

function WorkspaceShell(): Element {
  const stroke = workspaceTabLayout.stroke;
  const framed = () => !overviewOpen2.value && !isEmptyWorkspace();
  const radius = radii.lg;
  return (
    <Row style={{ grow: 1, minHeight: 0 }}>
      <Sidebar />

      {/* Accordee `html[data-layout=modern] .app-main`: inset main panel */}
      <Column
        style={{ grow: 1, minWidth: 0, minHeight: 0,
          margin: [spacing.workspaceFrame, spacing.workspaceFrame, spacing.workspaceFrame, 0],
          padding: [stroke, 0, 0, 0], clip: true }}
      >
        {/* ASCII backdrop covering the full panel including the tab row when empty. */}
        <EmptyWorkspaceBackdrop />
        {/* The header and tab row sit above the workspace tiling. */}
        <TabBar />
        {/* All four corners of the frame curve evenly; TabBar overlaps the top stroke so the
            active tab connects cleanly into the frame. Inset content to keep every border arc. */}
        <Box style={{ grow: 1, minWidth: 0, minHeight: 0, margin: [-stroke, 0, 0, 0], padding: stroke }}
          paint={() => framed() ? ({
            radius,
            background: palette().appBg,
            edge: palette().border,
            edgeWidth: stroke,
          }) : {}}
        >
          <Box style={{ grow: 1, minWidth: 0, minHeight: 0, clip: true }}
            paint={() => ({
              radius: framed() ? radius - stroke : 0,
            })}
          >
            <TilingWorkspaceView />
          </Box>
        </Box>
      </Column>
    </Row>
  );
}

function IcyApp(): Element {
  sendHello();
  effect(() => {
    if (activeNav.value === "settings" && overviewOpen2.value) closeOverviewMode();
  });

  return (
    <Box style={{ grow: 1 }} paint={() => ({ background: palette().sidebarBg })}>
      {/* minHeight: 0 keeps every shell pinned to the window so inner lists scroll. */}
      <Dynamic value={() => activeNav.value === "settings"} style={{ grow: 1, minHeight: 0 }}>
        {(settings) => (settings ? <SettingsShell /> : <WorkspaceShell />)}
      </Dynamic>

      {/* Global Accordee-animated modal overlays */}
      <SearchPalette />
      <ProjectDialog />
      <ReleaseNotesModal />
      <ImageViewerModal />
      <RestartConfirmModal />
    </Box>
  );
}

mount(IcyApp);
