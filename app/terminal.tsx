// Clean, chrome-free terminal viewport. Raw keyboard input goes straight to the active PTY;
// clicking any underlined file path or localhost URL opens it directly.

function TerminalPane(): Element {
  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      {/* Optional URL preview notice when clicking an http://localhost link */}
      <Show when={() => noticeState.value !== null}>
        {() => (
          <Row
            style={{
              height: 30,
              shrink: 0,
              padding: [0, 12, 0, 14],
              align: "center",
              justify: "between",
              gap: 10,
            }}
            paint={() => ({
              background: accent().brandSoft,
              edge: palette().border,
              edgeWidth: spacing.hairline,
              edgeSides: [0, 0, 1, 0],
            })}
          >
            <Text
              lines={1}
              font={{ family: fontFamilies.mono, size: typeScale.caption, weight: fontWeights.normal }}
              paint={() => ({ color: accent().brandText })}
            >
              {() =>
                noticeState.value
                  ? `Preview URL (embedded browser in Phase 4): ${noticeState.value.text}`
                  : ""
              }
            </Text>
            <Box
              style={{ width: 20, height: 20, align: "center", justify: "center", shrink: 0 }}
              onPress={() => {
                noticeState.value = null;
              }}
              paint={() => ({
                radius: radii.sm,
                hover: { background: palette().kbdBg },
              })}
            >
              <Glyph name="close" size={11} color={() => palette().textSecondary} />
            </Box>
          </Row>
        )}
      </Show>

      {/* Pure terminal VT grid (Geist Mono 13px, lineHeight multiplier 1.4 = 18.2px) */}
      <For
        each={() => screenState.value.rows}
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
              style={{
                height: 18.2,
                padding: [0, 2, 0, 2],
                justify: "center",
              }}
              onPress={() => {
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
                font={{ family: fontFamilies.mono, size: 13, lineHeight: 1.4 }}
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
