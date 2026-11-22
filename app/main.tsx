// Root shell: Accordee modern frame with sidebar, spring-animated session tab bar,
// chrome-free PTY terminal viewport, and on-demand file viewer split pane.

onHost(function (message: { view?: string; data?: any }) {
  if (message.view === "sessions") {
    sessionsState.value = message.data;
  } else if (message.view === "screen") {
    screenState.value = message.data;
  } else if (message.view === "viewer") {
    viewerState.value = message.data;
  } else if (message.view === "cliUsage") {
    cliUsageState.value = message.data;
  } else if (message.view === "notice") {
    noticeState.value = message.data;
  } else if (message.view === "viewport" && Array.isArray(message.data)) {
    viewportSize.value = [Number(message.data[0]), Number(message.data[1])];
  }
});

function IcyApp(): Element {
  sendHello();

  return (
    <Row
      style={{ grow: 1 }}
      paint={() => ({ background: palette().sidebarBg })}
    >
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
          <TerminalPane />
          <FileViewer />
        </Row>
      </Column>
    </Row>
  );
}

mount(IcyApp);
