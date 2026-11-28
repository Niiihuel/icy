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
        margin: [spacing.headerHeight - workspaceTabLayout.height, 0, 0, 0],
        padding: props.label !== undefined ? [0, 10, 0, 8] : [0, 7, 0, 7],
        align: "center",
        gap: 6,
        shrink: 0,
      }}
      onPress={props.onPress}
      paint={() => ({
        radius: radii.md,
        background: active() ? chromeHighlight(true, palette().panelBg) : 0,
        hover: { background: chromeHighlight(active(), palette().panelBg) },
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

const tabWidths = signal(new Map<number, number>());

effect(() => {
  const widths = tabWidths.value;
  if (widths.size === 0) return;
  const live = new Set(allWindowIds());
  const kept = new Map([...widths].filter(([id]) => live.has(id)));
  if (kept.size !== widths.size) tabWidths.value = kept;
});

function TabShape(props: { width: () => number; outline?: boolean }): Element {
  const layout = workspaceTabLayout;
  const originalWidth = layout.width + layout.overhang * 2;
  const source = props.outline ? "glyph:workspace-tab-outline" : "glyph:workspace-tab-fill";
  const color = () => props.outline ? palette().border : palette().appBg;
  return (
    <Dynamic value={() => props.width() === originalWidth}
      style={() => ({ position: "absolute", left: 0, top: 0, width: props.width(), height: layout.height })}>
      {(original) => original ? (
        <Image source={source} style={{ width: originalWidth, height: layout.height }} paint={() => ({ color: color() })} />
      ) : (
        <Box style={{ grow: 1 }}>
          {/* Preserve both curve masks at their original size and extend the straight middle. */}
          <Box style={() => ({
            position: "absolute", left: layout.capWidth, top: 0, width: props.width() - layout.capWidth * 2,
            height: props.outline ? layout.stroke : layout.height,
          })} paint={() => ({ background: color() })} />
          {[false, true].map((right) => (
            <Box style={() => ({
              position: "absolute", left: right ? props.width() - layout.capWidth : 0, top: 0,
              width: layout.capWidth, height: layout.height, clip: true,
            })}>
              <Image source={source} style={{
                position: "absolute", left: right ? layout.capWidth - originalWidth : 0, top: 0,
                width: originalWidth, height: layout.height,
              }} paint={() => ({ color: color() })} />
            </Box>
          ))}
        </Box>
      )}
    </Dynamic>
  );
}

function TabBar(): Element {
  const layout = workspaceTabLayout;
  const resize = signal<{ id: number; width: number } | null>(null);
  const tabs = () => overviewOpen2.value ? [] : paneOrder();
  const widthFor = (id: number) => {
    const drag = resize.value;
    return drag?.id === id ? drag.width : tabWidths.value.get(id) ?? layout.width;
  };
  const overhang = layout.overhang * 2;
  const contentWidth = () => {
    const ids = tabs();
    return ids.length === 0 ? 0
      : ids.reduce((width, id) => width + widthFor(id) + layout.stripGap, overhang - layout.stripGap);
  };
  const activeItem = () => {
    if (activeNav.value !== "agents") return undefined;
    // Scroll the entire outline into view, not just the tab's contents.
    let start = 0;
    for (const id of tabs()) {
      const width = widthFor(id);
      if (id === tilingState.value.focusedSession) return { start, end: start + width + overhang };
      start += width + layout.stripGap;
    }
    return undefined;
  };
  const outlineWidth = () => {
    const item = activeItem();
    return item ? item.end - item.start : layout.width + overhang;
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
          resizing={() => resize.value !== null}
        >
          {/* Replace this span of the divider with a single stroke that starts and ends
              on the divider, so the curves never overlap it or the tab sides. */}
          <Box
            style={() => ({
              position: "absolute", left: activeItem()?.start ?? 0, top: spacing.headerHeight - layout.height,
              width: outlineWidth(), height: layout.height,
            })}
            paint={() => ({
              opacity: activeItem() && windowTransparency() === 0 ? 1 : 0,
              transitions: resize.value === null
                ? [springTransition("left"), smoothTransition("opacity", motion.fast)] : [],
            })}
          >
            <TabShape width={outlineWidth} />
            <Box style={() => ({
              position: "absolute", left: 0, top: layout.height - layout.stroke, width: outlineWidth(), height: layout.stroke,
            })}
              paint={() => ({ background: palette().appBg })} />
            <TabShape width={outlineWidth} outline />
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
              const resizeHovered = signal(false);
              const width = () => widthFor(id);
              let originalWidth = layout.width;
              onCleanup(() => {
                if (resize.value?.id === id) resize.value = null;
              });
              return (
                <Row
                  style={() => ({
                    width: width(), height: layout.height,
                    padding: [0, layout.resizeHitWidth, 0, layout.gap * 1.5],
                    align: "center", gap: layout.gap, shrink: 0,
                  })}
                  onHover={(inside) => {
                    hovered.value = inside;
                  }}
                  onPress={() => focusTile(id)}
                  onDrag={windowDragHandler(() => id)}
                >
                  {/* Inactive hover: a floating pill inside the tab, not the connected shape. */}
                  <Box style={() => ({
                    position: "absolute", left: spacing.unit / 2, top: spacing.unit,
                    width: width() - spacing.unit, height: spacing.button,
                  })}
                    paint={() => ({
                      radius: radii.md,
                      background: isActive()
                        ? windowTransparency() > 0 ? chromeHighlight(true) : 0
                        : hovered.value ? chromeHighlight() : 0,
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
                    layoutChanged={width}
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
                      hover: { background: chromeHighlight(false, palette().kbdBg) },
                      transitions: [smoothTransition("opacity", motion.fast), smoothTransition("background", motion.fast)],
                    })}
                  >
                    <Glyph name="close" size={10} color={() => palette().textMuted} hover={() => palette().textPrimary} />
                  </Box>
                  <Box
                    style={() => ({
                      position: "absolute", left: width() - layout.resizeHitWidth, top: spacing.unit,
                      width: layout.resizeHitWidth, height: spacing.button, align: "center", justify: "center",
                    })}
                    onHover={(inside) => { resizeHovered.value = inside; }}
                    onDrag={(event) => {
                      if (event.phase === "start") {
                        originalWidth = width();
                        resize.value = { id, width: originalWidth };
                        return;
                      }
                      const drag = resize.value;
                      if (drag?.id !== id) return;
                      const next = Math.max(layout.minWidth, Math.min(layout.maxWidth, Math.round(originalWidth + event.dx)));
                      if (event.phase === "move") {
                        if (next !== drag.width) resize.value = { id, width: next };
                        return;
                      }
                      if (next !== (tabWidths.value.get(id) ?? layout.width)) {
                        const widths = new Map(tabWidths.value);
                        if (next === layout.width) widths.delete(id);
                        else widths.set(id, next);
                        tabWidths.value = widths;
                      }
                      resize.value = null;
                    }}
                    paint={{ cursor: "colResize" }}
                  >
                    <Box style={{ width: workspaceTiling.guideWidth, height: spacing.button - spacing.unit * 2 }}
                      paint={() => ({
                        radius: radii.xs, background: accent().brand,
                        opacity: resize.value?.id === id ? 1 : resizeHovered.value ? workspaceTiling.resizeHoverOpacity : 0,
                        transitions: [smoothTransition("opacity", motion.fast)],
                      })} />
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
