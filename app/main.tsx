// Root shell combining Accordee's modern frame layout with terminal-first UX.

onHost(function (message: { view?: string; data?: any }) {
  if (message.view === "sessions") {
    sessionsState.value = message.data;
  } else if (message.view === "screen") {
    screenState.value = message.data;
  } else if (message.view === "viewer") {
    viewerState.value = message.data;
  } else if (message.view === "notice") {
    noticeState.value = message.data;
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

      {/* Accordee `html[data-layout=modern] .app-main`: inset main surface */}
      <Row
        style={{
          grow: 1,
          minWidth: 0,
          minHeight: 0,
          margin: [8, 8, 8, 0],
          clip: true,
        }}
        paint={() => ({
          radius: radii.lg,
          background: palette().appBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
          shadows: [shadows.panel],
        })}
      >
        <TerminalPane />
        <FileViewer />
      </Row>
    </Row>
  );
}

mount(IcyApp);
