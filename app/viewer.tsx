// Read-only file viewer pane opened when clicking any file reference in terminal output.

function FileViewer(): Element {
  return (
    <Show when={() => viewerState.value.open}>
      {() => (
        <Column
          style={{
            width: 460,
            shrink: 0,
            minHeight: 0,
          }}
          paint={() => ({
            background: palette().panelBg,
            edge: palette().border,
            edgeWidth: spacing.hairline,
            edgeSides: [0, 0, 0, 1],
          })}
        >
          {/* Viewer header */}
          <Row
            style={{
              height: spacing.tabbar,
              shrink: 0,
              padding: [0, 10, 0, 14],
              align: "center",
              justify: "between",
              gap: 10,
            }}
            paint={() => ({
              edge: palette().border,
              edgeWidth: spacing.hairline,
              edgeSides: [0, 0, 1, 0],
            })}
          >
            <Row style={{ align: "center", gap: 8, grow: 1, minWidth: 0 }}>
              <Text
                lines={1}
                font={{
                  family: fontFamilies.mono,
                  size: typeScale.caption,
                  weight: fontWeights.medium,
                }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() => viewerState.value.displayPath || viewerState.value.path}
              </Text>
              <Show
                when={() =>
                  viewerState.value.targetLine !== null &&
                  viewerState.value.targetLine !== undefined
                }
              >
                {() => (
                  <CountPill count={() => `L${viewerState.value.targetLine ?? 1}`} />
                )}
              </Show>
            </Row>
            <Box
              style={{ width: 24, height: 24, align: "center", justify: "center", shrink: 0 }}
              onPress={closeViewer}
              paint={() => ({
                radius: radii.md,
                hover: { background: palette().sidebarActive },
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <Glyph
                name="close"
                size={13}
                color={() => palette().textMuted}
                hover={() => palette().textPrimary}
              />
            </Box>
          </Row>

          {/* Error state if missing or unreadable */}
          <Show
            when={() =>
              viewerState.value.error !== null && viewerState.value.error !== undefined
            }
          >
            {() => (
              <Column style={{ padding: [16, 16, 16, 16], gap: 6 }}>
                <Text
                  font={{
                    family: fontFamilies.sans,
                    size: typeScale.label,
                    weight: fontWeights.medium,
                  }}
                  paint={() => ({ color: palette().destructive })}
                >
                  Cannot open file
                </Text>
                <Text
                  font={{
                    family: fontFamilies.mono,
                    size: typeScale.caption,
                    weight: fontWeights.normal,
                  }}
                  paint={() => ({ color: palette().textSecondary })}
                >
                  {() => viewerState.value.error ?? ""}
                </Text>
              </Column>
            )}
          </Show>

          {/* Code lines */}
          <For
            each={() => viewerState.value.lines}
            key={(line) => line.num}
            style={{
              grow: 1,
              minHeight: 0,
              scroll: true,
              padding: [10, 0, 14, 0],
              gap: 0,
            }}
          >
            {(line) => (
              <Row
                style={{
                  height: 18.2,
                  padding: [0, 12, 0, 12],
                  gap: 12,
                  align: "center",
                }}
                paint={() => ({
                  background: line().highlighted ? accent().brandSoft : undefined,
                })}
              >
                <Text
                  style={{ width: 38, shrink: 0 }}
                  align="end"
                  font={{ family: fontFamilies.mono, size: typeScale.caption, lineHeight: 1.4 }}
                  paint={() => ({
                    color: line().highlighted ? accent().brandText : palette().textMuted,
                  })}
                >
                  {() => String(line().num)}
                </Text>
                <Text
                  lines={1}
                  style={{ grow: 1, minWidth: 0 }}
                  font={{ family: fontFamilies.mono, size: 12.5, lineHeight: 1.4 }}
                  paint={() => ({ color: palette().textPrimary })}
                >
                  {() => (line().text.length > 0 ? line().text : " ")}
                </Text>
              </Row>
            )}
          </For>
        </Column>
      )}
    </Show>
  );
}
