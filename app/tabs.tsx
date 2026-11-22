// Paginated session tab bar with 21st.dev 1113 spring-animated sliding active pill.

const TABS_PER_PAGE = 5;

function TabBar(): Element {
  const currentPage = signal(0);
  const pillLeft = signal(0);
  const pillWidth = signal(0);
  const tabNodes = new Map<number, number>();
  let stripNodeId: number | undefined;

  const totalPages = () =>
    Math.max(1, Math.ceil(sessionsState.value.items.length / TABS_PER_PAGE));

  const visibleTabs = () => {
    const items = sessionsState.value.items;
    const page = Math.min(currentPage.value, Math.max(0, totalPages() - 1));
    const start = page * TABS_PER_PAGE;
    return items.slice(start, start + TABS_PER_PAGE);
  };

  function syncPill(): void {
    const activeId = sessionsState.value.activeId;
    const tabId = tabNodes.get(activeId);
    if (stripNodeId === undefined || tabId === undefined) {
      pillWidth.value = 0;
      return;
    }
    const cols = jsnir.columns;
    const w = cols.width[tabId] ?? 0;
    const relX = (cols.x[tabId] ?? 0) - (cols.x[stripNodeId] ?? 0);
    if (w > 0) {
      pillLeft.value = relX;
      pillWidth.value = w;
    }
  }

  // Keep currentPage aligned with activeId and re-measure the spring pill after layout updates.
  effect(() => {
    const items = sessionsState.value.items;
    const activeId = sessionsState.value.activeId;
    const _vp = viewportSize.value;
    void _vp;
    const idx = items.findIndex((item) => item.id === activeId);
    if (idx >= 0) {
      const targetPage = Math.floor(idx / TABS_PER_PAGE);
      if (currentPage.value !== targetPage) {
        currentPage.value = targetPage;
      }
    }
    syncPill();
    after(24, syncPill);
    after(80, syncPill);
  });

  return (
    <Row
      style={{
        height: spacing.tabbar,
        shrink: 0,
        padding: [6, 10, 6, 10],
        align: "center",
        justify: "between",
        gap: 8,
      }}
      paint={() => ({
        background: palette().appBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        edgeSides: [0, 0, 1, 0],
      })}
    >
      <Row style={{ align: "center", gap: 6, grow: 1, minWidth: 0 }}>
        {/* Restore sidebar button when collapsed */}
        <Show when={() => !sidebarOpen.value}>
          {() => (
            <Box
              style={{
                width: 28,
                height: 28,
                align: "center",
                justify: "center",
                shrink: 0,
              }}
              onPress={() => {
                sidebarOpen.value = true;
                syncLayout();
              }}
              paint={() => ({
                radius: radii.md,
                hover: { background: palette().panelBg },
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
          )}
        </Show>

        {/* Spring-animated tab strip (21st.dev 1113 layoutId="bubble" adaptation) */}
        <Row
          ref={(id) => {
            stripNodeId = id;
            after(24, syncPill);
          }}
          style={{
            height: 30,
            align: "center",
            gap: 4,
            grow: 1,
            minWidth: 0,
          }}
        >
          {/* Animated spring background bubble */}
          <Box
            style={() => ({
              position: "absolute",
              left: pillLeft.value,
              top: 0,
              width: pillWidth.value,
              height: 30,
            })}
            paint={() => ({
              radius: radii.md,
              background: palette().sidebarActive,
              edge: palette().border,
              edgeWidth: spacing.hairline,
              opacity: pillWidth.value > 0 ? 1 : 0,
              transitions: [
                { property: "left", duration: 520, easing: "spring" },
                { property: "width", duration: 520, easing: "spring" },
                smoothTransition("opacity", motion.fast),
              ],
            })}
          />

          <For
            each={visibleTabs}
            key={(item) => item.id}
            style={{ direction: "row", align: "center", gap: 4, height: 30 }}
          >
            {(item) => {
              const isActive = () => sessionsState.value.activeId === item().id;
              return (
                <Row
                  ref={(id) => {
                    tabNodes.set(item().id, id);
                    onCleanup(() => {
                      tabNodes.delete(item().id);
                    });
                    after(16, syncPill);
                  }}
                  style={{
                    height: 30,
                    padding: [0, 10, 0, 10],
                    align: "center",
                    gap: 7,
                    shrink: 0,
                  }}
                  onPress={() => selectSession(item().id)}
                  paint={() => ({
                    radius: radii.md,
                    hover: isActive() ? undefined : { background: palette().panelBg },
                    transitions: [smoothTransition("background", motion.fast)],
                  })}
                >
                  <Image
                    source={() => "cli:" + item().cli}
                    style={{ width: 14, height: 14, shrink: 0 }}
                  />
                  <Text
                    lines={1}
                    style={{ maxWidth: 168 }}
                    font={{
                      family: fontFamilies.sans,
                      size: typeScale.label,
                      weight: fontWeights.medium,
                    }}
                    paint={() => ({
                      color: isActive() ? palette().textPrimary : palette().textSecondary,
                      hover: { color: palette().textPrimary },
                      transitions: [smoothTransition("color", motion.fast)],
                    })}
                  >
                    {() => item().title}
                  </Text>
                  <Show when={() => sessionsState.value.items.length > 1}>
                    {() => (
                      <Box
                        style={{
                          width: 16,
                          height: 16,
                          align: "center",
                          justify: "center",
                          shrink: 0,
                        }}
                        onPress={() => closeSession(item().id)}
                        paint={() => ({
                          radius: radii.sm,
                          hover: { background: palette().kbdBg },
                          transitions: [smoothTransition("background", motion.fast)],
                        })}
                      >
                        <Glyph
                          name="close"
                          size={10}
                          color={() => palette().textMuted}
                          hover={() => palette().textPrimary}
                        />
                      </Box>
                    )}
                  </Show>
                </Row>
              );
            }}
          </For>

          {/* New session tab button (+) */}
          <Box
            style={{
              width: 28,
              height: 28,
              align: "center",
              justify: "center",
              shrink: 0,
            }}
            onPress={createSession}
            paint={() => ({
              radius: radii.md,
              hover: { background: palette().panelBg },
              active: { scale: 0.94 },
              transitions: [
                smoothTransition("background", motion.fast),
                smoothTransition("scale", motion.fast),
              ],
            })}
          >
            <Glyph
              name="plus"
              size={14}
              color={() => palette().textMuted}
              hover={() => palette().textPrimary}
            />
          </Box>
        </Row>
      </Row>

      {/* Pagination controls (visible cleanly on the right) */}
      <Row style={{ align: "center", gap: 4, shrink: 0 }}>
        <Box
          style={{
            width: 24,
            height: 24,
            align: "center",
            justify: "center",
            shrink: 0,
          }}
          disabled={() => currentPage.value <= 0}
          onPress={() => {
            if (currentPage.value > 0) {
              const next = currentPage.value - 1;
              currentPage.value = next;
              const firstOnPage = sessionsState.value.items[next * TABS_PER_PAGE];
              if (firstOnPage) selectSession(firstOnPage.id);
            }
          }}
          paint={() => ({
            radius: radii.md,
            opacity: currentPage.value <= 0 ? 0.35 : 1,
            hover: { background: palette().panelBg },
            transitions: [smoothTransition("background", motion.fast)],
          })}
        >
          <Glyph name="chevron-left" size={13} color={() => palette().textSecondary} />
        </Box>

        <Text
          font={{ family: fontFamilies.mono, size: typeScale.tiny, weight: fontWeights.normal }}
          paint={() => ({ color: palette().textMuted })}
        >
          {() => `${Math.min(currentPage.value + 1, totalPages())} / ${totalPages()}`}
        </Text>

        <Box
          style={{
            width: 24,
            height: 24,
            align: "center",
            justify: "center",
            shrink: 0,
          }}
          disabled={() => currentPage.value + 1 >= totalPages()}
          onPress={() => {
            if (currentPage.value + 1 < totalPages()) {
              const next = currentPage.value + 1;
              currentPage.value = next;
              const firstOnPage = sessionsState.value.items[next * TABS_PER_PAGE];
              if (firstOnPage) selectSession(firstOnPage.id);
            }
          }}
          paint={() => ({
            radius: radii.md,
            opacity: currentPage.value + 1 >= totalPages() ? 0.35 : 1,
            hover: { background: palette().panelBg },
            transitions: [smoothTransition("background", motion.fast)],
          })}
        >
          <Glyph name="chevron-right" size={13} color={() => palette().textSecondary} />
        </Box>
      </Row>
    </Row>
  );
}
