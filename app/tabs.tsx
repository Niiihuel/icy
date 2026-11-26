// The active workspace's tabs, overview control, and launcher.

// Accordee ghost button (`Memory`, `Share`, `Start from scratch`): 30px, icon 16 + 13px label.
function GhostButton(props: {
  icon: Source<string>;
  label?: Source<string>;
  active?: Source<boolean>;
  tooltip?: string;
  tooltipDetail?: string;
  keys?: Source<string[]>;
  onPress: () => void;
  trailing?: Element;
}): Element {
  const active = () => read(props.active ?? false);
  const button = (
    <Row
      style={{
        height: spacing.button,
        padding: props.label !== undefined ? [0, 10, 0, 8] : [0, 7, 0, 7],
        align: "center",
        gap: 6,
        shrink: 0,
      }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: active() ? palette().panelBg : 0,
        hover: { background: palette().panelBg },
        active: { scale: controlState.pressScale },
        transitions: [
          smoothTransition("background", motion.fast),
          smoothTransition("scale", motion.fast),
        ],
      })}
    >
      <Glyph
        name={props.icon}
        size={16}
        color={() => (active() ? palette().textPrimary : palette().textSecondary)}
        hover={() => palette().textPrimary}
      />
      {props.label !== undefined ? (
        <Text
          lines={1}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          paint={() => ({ color: active() ? palette().textPrimary : palette().textSubtle })}
        >
          {props.label}
        </Text>
      ) : null}
      {props.trailing ?? null}
    </Row>
  );
  return props.tooltip ? (
    <HintTooltip label={props.tooltip} detail={props.tooltipDetail} keys={props.keys} side="bottom">
      {button}
    </HintTooltip>
  ) : (
    button
  );
}

function TabBar(): Element {
  const layout = workspaceTabLayout;
  const outlineWidth = layout.width + layout.overhang * 2;
  const tabStride = layout.width + layout.stripGap;
  const tabs = () => overviewOpen2.value ? [] : paneOrder();
  const contentWidth = () => {
    const count = tabs().length;
    return count === 0 ? 0 : (count - 1) * tabStride + outlineWidth;
  };
  const activeItem = () => {
    if (activeNav.value !== "agents") return undefined;
    const index = tabs().indexOf(tilingState.value.focusedSession);
    if (index < 0) return undefined;
    // Scroll the entire outline into view, not just the tab's contents.
    const start = index * tabStride;
    return { start, end: start + outlineWidth };
  };

  return (
    <Column style={{ height: spacing.headerHeight, shrink: 0, layer: layout.layer }}>
      {/* The header covers previews, with room for the workspace stroke along its bottom. */}
      <Box
        style={{ height: spacing.headerHeight - layout.stroke, shrink: 0,
          margin: [0, 0, layout.stroke - spacing.headerHeight, 0] }}
        paint={() => ({ background: tabs().length > 0 ? windowUnderlay(palette().sidebarBg) : 0 })}
      />
      <Row
        style={{
          height: spacing.headerHeight,
          shrink: 0,
          padding: [0, layout.gap + layout.stroke, 0, layout.gap + layout.stroke],
          align: "center",
          gap: spacing.unit,
        }}
      >
        {/* Niri workspaces: the active one's number; opens (or closes) the overview. */}
        <GhostButton icon="workspaces" label={() => String(tilingState.value.activeWorkspace + 1)}
          tooltip="Workspaces" keys={() => keysFor("view.overview")} active={() => overviewOpen2.value}
          onPress={toggleOverviewMode} />
        <HorizontalStrip
          contentWidth={contentWidth}
          label="tabs"
          height={spacing.headerHeight}
          activeItem={activeItem}
        >
          {/* Replace this span of the divider with a single stroke that starts and ends
              on the divider, so the curves never overlap it or the tab sides. */}
          <Box
            style={() => ({
              position: "absolute", left: activeItem()?.start ?? 0, top: spacing.headerHeight - layout.height,
              width: outlineWidth, height: layout.height,
            })}
            paint={() => ({
              opacity: activeItem() ? 1 : 0,
              transitions: [springTransition("left"), smoothTransition("opacity", motion.fast)],
            })}
          >
            <Image source="glyph:workspace-tab-fill" style={{ width: outlineWidth, height: layout.height }}
              paint={() => ({ color: palette().appBg })} />
            <Box style={{ position: "absolute", left: 0, top: layout.height - layout.stroke, width: outlineWidth, height: layout.stroke }}
              paint={() => ({ background: palette().appBg })} />
            <Box style={{ position: "absolute", left: 0, top: 0, width: outlineWidth, height: layout.height }}>
              <Image source="glyph:workspace-tab-outline" style={{ width: outlineWidth, height: layout.height }}
                paint={() => ({ color: palette().border })} />
            </Box>
          </Box>
          {/* Tabs follow the active workspace's tiling order. */}
          <For
            each={tabs}
            key={(id) => id}
            style={() => ({ direction: "row", align: "end", gap: workspaceTabLayout.stripGap, height: spacing.headerHeight,
              padding: tabs().length > 0 ? [0, 0, 0, layout.overhang] : 0 })}
          >
            {(tabId) => {
              const id = tabId();
              const session = () => sessionsState.value.items.find((s) => s.id === id);
              const tile = () => tileOf(id);
              const isActive = () => activeNav.value === "agents" && tilingState.value.focusedSession === id;
              const hovered = signal(false);
              return (
                <Row
                  style={{ width: workspaceTabLayout.width, height: workspaceTabLayout.height,
                    padding: [0, spacing.unit * 1.5, 0, workspaceTabLayout.gap * 1.5],
                    align: "center", gap: workspaceTabLayout.gap, shrink: 0 }}
                  onHover={(inside) => {
                    hovered.value = inside;
                  }}
                  onPress={() => focusTile(id)}
                  onDrag={windowDragHandler(() => id)}
                >
                  {/* Inactive hover: a floating pill inside the tab, not the connected shape. */}
                  <Box style={{ position: "absolute", left: spacing.unit / 2, top: spacing.unit,
                    width: workspaceTabLayout.width - spacing.unit, height: spacing.button }}
                    paint={() => ({
                      radius: radii.md,
                      background: hovered.value && !isActive() ? palette().sidebarActive : 0,
                      transitions: [smoothTransition("background", motion.fast)],
                    })} />
                  <Dynamic value={() => (tile() ? tileGlyph(tile() as TileSpec) : "")} style={{ shrink: 0 }}>
                    {(glyph) =>
                      glyph ? (
                        <Glyph name={glyph} size={workspaceTabLayout.icon}
                          color={() => (isActive() || hovered.value ? palette().textSecondary : palette().textMuted)} />
                      ) : (
                        <Image source={() => "cli:" + (session()?.cli ?? "terminal")}
                          style={{ width: workspaceTabLayout.icon, height: workspaceTabLayout.icon }}
                          paint={() => ({ opacity: isActive() || hovered.value ? 1 : workspaceTabLayout.dimOpacity,
                            transitions: [smoothTransition("opacity", motion.fast)] })} />
                      )
                    }
                  </Dynamic>
                  <FadeText
                    text={() => {
                      const spec = tile();
                      const s = session();
                      if (spec) return tileTitle(spec, id);
                      if (s?.cli === "terminal") return basename(s.cwd) || "Terminal";
                      return cleanTitle(s?.title ?? "");
                    }}
                    font={{ family: fontFamilies.sans, size: typeScale.label }}
                    color={() => isActive() ? palette().textPrimary
                      : hovered.value ? palette().textSecondary : palette().textMuted}
                    background={() => isActive() ? palette().appBg
                      : hovered.value ? palette().sidebarActive : palette().sidebarBg}
                  />
                  {/* Close: always on the active tab, on hover for the rest (Silica). */}
                  <Box
                    style={{ width: workspaceTabLayout.closeSize, height: workspaceTabLayout.closeSize, align: "center", justify: "center", shrink: 0 }}
                    onPress={() => closeSession(id)}
                    paint={() => ({
                      radius: radii.sm,
                      opacity: isActive() || hovered.value ? 1 : 0,
                      hover: { background: palette().kbdBg },
                      transitions: [smoothTransition("opacity", motion.fast), smoothTransition("background", motion.fast)],
                    })}
                  >
                    <Glyph name="close" size={10} color={() => palette().textMuted} hover={() => palette().textPrimary} />
                  </Box>
                </Row>
              );
            }}
          </For>
        </HorizontalStrip>

        {/* An empty workspace keeps a "New session" tab where the session tabs would be. */}
        <Show when={() => !overviewOpen2.value} style={{ shrink: 0 }}>
          {() => (
            <Dynamic value={() => tabs().length === 0}>
              {(empty) => empty ? (
                <GhostButton icon="plus" label="New session" tooltip="New session" keys={() => keysFor("session.new")}
                  onPress={createSession} />
              ) : (
                <GhostButton icon="plus" tooltip="Open launcher" tooltipDetail="Start an agent, open a file, Git or a URL"
                  keys={() => keysFor("panel.side")}
                  onPress={() => togglePalette("launcher")} />
              )}
            </Dynamic>
          )}
        </Show>
      </Row>
    </Column>
  );
}
