// Clean, chrome-free terminal viewport. Raw keyboard input goes straight to the focused PTY;
// clicking any underlined file path or localhost URL opens it directly.

function TerminalPane(props: {
  sessionId: () => number;
  focused: () => boolean;
  multiPane: () => boolean;
}): Element {
  const screen = () =>
    screensMapState.value[String(props.sessionId())] ?? screenState.value;

  const sessionMeta = () =>
    sessionsState.value.items.find((s) => s.id === props.sessionId());

  return (
    <Column
      style={{ grow: 1, minWidth: 0, minHeight: 0, clip: true }}
      onPress={() => {
        if (!props.focused()) {
          selectSession(props.sessionId());
        }
      }}
      paint={() => ({
        radius: props.multiPane() ? radii.lg : 0,
        background: palette().appBg,
        edge: props.multiPane()
          ? props.focused()
            ? accent().brand
            : palette().border
          : 0,
        edgeWidth: props.multiPane() ? (props.focused() ? 1 : spacing.hairline) : 0,
        transitions: [smoothTransition("edge", motion.fast)],
      })}
    >
      {/* Subtle Hyprland/Niri corner pill ONLY when multiple panes are tiled side-by-side */}
      <Show when={props.multiPane} style={{ shrink: 0 }}>
        {() => (
          <Row
            style={{
              height: 24,
              padding: [0, 10, 0, 10],
              align: "center",
              justify: "between",
              shrink: 0,
            }}
            paint={() => ({
              background: props.focused() ? palette().sidebarActive : palette().panelBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
              edgeSides: [0, 0, 1, 0],
            })}
          >
            <Row style={{ align: "center", gap: 6 }}>
              <Image
                source={() => "cli:" + (sessionMeta()?.cli ?? "terminal")}
                style={{ width: 12, height: 12, shrink: 0 }}
              />
              <Text
                lines={1}
                font={{
                  family: fontFamilies.sans,
                  size: typeScale.tiny,
                  weight: fontWeights.medium,
                }}
                paint={() => ({
                  color: props.focused() ? palette().textPrimary : palette().textMuted,
                })}
              >
                {() => sessionMeta()?.title ?? `pane-${props.sessionId()}`}
              </Text>
            </Row>
            <StatusDot alive={() => sessionMeta()?.alive ?? true} />
          </Row>
        )}
      </Show>

      {/* Pure terminal VT grid (Geist Mono, lineHeight multiplier 1.4) */}
      <For
        each={() => screen().rows}
        key={(row) => row.idx}
        style={{
          grow: 1,
          minHeight: 0,
          scroll: true,
          padding: [10, 14, 10, 14],
          gap: 0,
        }}
      >
        {(row) => {
          const hasLinks = () => row().links.length > 0;
          return (
            <Box
              style={() => ({
                height: Math.round(terminalFontSize.value * 1.4 * 10) / 10,
                padding: [0, 2, 0, 2],
                justify: "center",
              })}
              onPress={() => {
                if (!props.focused()) {
                  selectSession(props.sessionId());
                }
                const links = row().links;
                if (links.length > 0) openLink(links[0]);
              }}
              paint={() => ({
                radius: radii.xs,
                hover: hasLinks() ? { background: accent().brandSoft } : undefined,
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <Text
                lines={1}
                font={{
                  family: fontFamilies.mono,
                  size: terminalFontSize.value,
                  lineHeight: 1.4,
                }}
                spans={() =>
                  row().spans.map((s) => ({
                    text: s.text,
                    color: s.link ? accent().brandText : s.fg,
                    background: s.bg,
                    weight: s.bold ? 600 : 400,
                    underline: s.underline || s.link,
                  }))
                }
              >
                {() => row().plain}
              </Text>
            </Box>
          );
        }}
      </For>
    </Column>
  );
}
