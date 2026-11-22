// Niri scrollable-column & Hyprland split tiling workspace renderer.

function TilingWorkspaceView(): Element {
  const columns = () => tilingState.value.visibleColumns;
  const multiPane = () => {
    const cols = columns();
    return cols.length > 1 || (cols.length === 1 && cols[0].panes.length > 1);
  };
  const canScrollLeft = () =>
    tilingState.value.mode === "niri" && tilingState.value.scrollCol > 0;
  const canScrollRight = () =>
    tilingState.value.mode === "niri" &&
    tilingState.value.scrollCol + columns().length < tilingState.value.totalCols;

  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      {/* Optional URL / info notice banner */}
      <Show when={() => noticeState.value !== null} style={{ shrink: 0 }}>
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
              font={{ family: fontFamilies.mono, size: typeScale.caption }}
              paint={() => ({ color: accent().brandText })}
            >
              {() => (noticeState.value ? noticeState.value.text : "")}
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

      {/* Niri horizontal column viewport bar (only shown when > visible columns) */}
      <Show when={() => canScrollLeft() || canScrollRight()} style={{ shrink: 0 }}>
        {() => (
          <Row
            style={{
              height: 26,
              padding: [0, 10, 0, 10],
              align: "center",
              justify: "between",
              shrink: 0,
            }}
            paint={() => ({
              background: palette().panelBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
              edgeSides: [0, 0, 1, 0],
            })}
          >
            <Text
              font={{ family: fontFamilies.mono, size: typeScale.micro }}
              paint={() => ({ color: palette().textMuted })}
            >
              {() =>
                `Niri strip · col ${tilingState.value.scrollCol + 1}–${
                  tilingState.value.scrollCol + columns().length
                } of ${tilingState.value.totalCols}`
              }
            </Text>
            <Row style={{ gap: 4 }}>
              <Box
                style={{ width: 22, height: 20, align: "center", justify: "center" }}
                disabled={() => !canScrollLeft()}
                onPress={() => scrollTiledColumns(-1)}
                paint={() => ({
                  radius: radii.sm,
                  opacity: canScrollLeft() ? 1 : 0.35,
                  hover: { background: palette().sidebarActive },
                })}
              >
                <Glyph name="chevron-left" size={12} color={() => palette().textSecondary} />
              </Box>
              <Box
                style={{ width: 22, height: 20, align: "center", justify: "center" }}
                disabled={() => !canScrollRight()}
                onPress={() => scrollTiledColumns(1)}
                paint={() => ({
                  radius: radii.sm,
                  opacity: canScrollRight() ? 1 : 0.35,
                  hover: { background: palette().sidebarActive },
                })}
              >
                <Glyph name="chevron-right" size={12} color={() => palette().textSecondary} />
              </Box>
            </Row>
          </Row>
        )}
      </Show>

      {/* Tiled columns & vertical pane stacks */}
      <For
        each={columns}
        key={(col) => col.panes.join("-")}
        style={() => ({
          direction: "row",
          grow: 1,
          minWidth: 0,
          minHeight: 0,
          gap: multiPane() ? spacing.paneGap : 0,
          padding: multiPane() ? [8, 8, 8, 8] : [0, 0, 0, 0],
        })}
      >
        {(col) => (
          <For
            each={() => col().panes}
            key={(sid) => sid}
            style={() => ({
              direction: "column",
              grow: 1,
              minWidth: 0,
              minHeight: 0,
              gap: col().panes.length > 1 ? spacing.paneGap : 0,
            })}
          >
            {(sid) => (
              <TerminalPane
                sessionId={sid}
                focused={() => tilingState.value.focusedSession === sid()}
                multiPane={multiPane}
              />
            )}
          </For>
        )}
      </For>
    </Column>
  );
}
