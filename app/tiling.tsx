// Niri scrollable-column strip & Hyprland split tiling renderer. Geometry comes from
// icy-core::tiling (fractions of the area width); this file only maps it to pixels.

// ---- niri workspaces and overview ------------------------------------------------------------
// Every workspace is drawn live, stacked top to bottom with niri's gap (10% of the area height).
// A workspace switch moves the stack and the overview zooms the same stack out around the
// centred workspace (niri monitor.rs `workspaces_render_geo`), so no window is ever duplicated or
// cross-faded. A new window opens with a fade and slight scale; a closed one fades and shrinks
// in place (JSNIR's `paint.exiting` keeps its last frame).

// False until the restored layout has been drawn: that one is not something opening.
let openAnimation = false;
let tilingArea: [number, number] = [0, 0];
const committedResize = signal(false);

// Windows still open somewhere after the next tiling push: they moved, so they just go.
const liveWindows = signal<Set<number>>(new Set());

function overviewZoom(): number {
  return overviewOpen2.value ? tilingMotion.overviewZoom : 1;
}

// The workspace holding the overview selection (the trailing empty one for "Add workspace").
function overviewWorkspace(): number {
  const selected = overviewIndex.value;
  const list = workspaceList();
  if (selected === "addWorkspace") return list.length - 1;
  const index = list.findIndex((row) => row.panes.includes(selected));
  return index >= 0 ? index : tilingState.value.activeWorkspace;
}

// The workspace the stack centres: the overview follows its selection, and on close the
// stack heads straight for the picked workspace before the host has switched to it.
function centeredWorkspace(): number {
  if (overviewOpen2.value) return overviewWorkspace();
  return overviewExitTarget.value ?? tilingState.value.activeWorkspace;
}

// Distance between two workspace tops at zoom 1 (niri `workspace_size_with_gap`).
function workspacePitch(area: [number, number]): number {
  return area[1] * (1 + tilingMotion.workspaceGap);
}

// A workspace's layout: the active one is the live (or resize-previewed) view.
function workspaceTilingView(index: number, active: TilingView): TilingView {
  if (index === active.activeWorkspace) return active;
  const workspace = workspaceList()[index];
  return { ...active, columns: workspace?.columns ?? [], viewOffset: workspace?.viewOffset ?? 0,
    focusedSession: workspace?.focused ?? 0, maximized: false };
}

function tilingEasing(property: Property): Transition {
  return { property, duration: tilingMotion.duration, easing: "bezier", bezier: tilingMotion.ease };
}

function tilingPaneTransitions(): Transition[] {
  // Zero-duration transitions also update JSNIR's stored targets while the pointer moves.
  const direct = resizeDrag.value !== null || committedResize.value;
  const geometry: Transition[] = direct
    ? (["width", "x", "y", "height"] as const).map((property) => ({ property, duration: 0 }))
    : [springTransition("width"), springTransition("x"), workspaceTransition("y"), springTransition("height")];
  return [...geometry, tilingEasing("opacity"), tilingEasing("scale")];
}

function sameTilingGeometry(prev: TilingView, next: TilingView): boolean {
  return prev.activeWorkspace === next.activeWorkspace && prev.maximized === next.maximized &&
    (!next.maximized || prev.focusedSession === next.focusedSession) &&
    prev.viewOffset === next.viewOffset && prev.columns.length === next.columns.length &&
    prev.columns.every((column, index) => {
      const other = next.columns[index];
      return column.id === other.id && column.x === other.x && column.width === other.width &&
        column.panes.length === other.panes.length &&
        column.panes.every((id, row) => id === other.panes[row] && column.heights[row] === other.heights[row]);
    });
}

function paneExiting(id: number): Exiting {
  // Never undefined: JSNIR only keeps the last frame of nodes that had `exiting` when drawn, so
  // live panes carry an empty one (animates nothing) and the close's same-batch update plays it.
  return liveWindows.value.has(id) ? {} : { opacity: 0, scale: tilingMotion.closeScale };
}

// Called with the old and new layout before a tiling push is applied, so the exits set here
// reach the engine in the same batch as the removals.
function noteTilingChange(prev: TilingView, next: TilingView): void {
  // A host snapshot echo can arrive before the committed geometry has been drawn.
  if (committedResize.value && !sameTilingGeometry(prev, next)) committedResize.value = false;
  if (!workspaceReady.value) {
    afterView(32, () => { openAnimation = true; });
    return;
  }
  const live = new Set((next.workspaces ?? []).flatMap((workspace) => workspace.panes));
  for (const column of next.columns) for (const id of column.panes) live.add(id);
  liveWindows.value = live;
}

function TilingWorkspaceView(): Element {
  const measured = signal<[number, number]>([0, 0]);
  // Width is the *target* width (window − sidebar − frame margin), not the measured one: when the
  // sidebar springs, columns spring to their final geometry in the same frame instead of lagging
  // behind a re-measure. Height falls back the same way until measured.
  const area = {
    get value(): [number, number] {
      const [vw, vh] = viewportSize.value;
      const side = sidebarCompact.value ? spacing.sidebarRail : spacing.sidebarWidth;
      return [
        Math.max(0, vw - side - spacing.workspaceFrame - 2 * workspaceTabLayout.stroke),
        measured.value[1] > 0
          ? measured.value[1]
          : Math.max(0, vh - 2 * (spacing.workspaceFrame + workspaceTabLayout.stroke) - spacing.headerHeight),
      ];
    },
  };
  let areaId: number | undefined;
  effect(() => {
    const next = area.value;
    if (tilingArea[0] !== next[0] || tilingArea[1] !== next[1]) committedResize.value = false;
    tilingArea = next;
  });
  effect(() => {
    const target = overviewExitTarget.value;
    if (target === null) return;
    afterView(SPRING_MS, () => {
      if (overviewExitTarget.value === target) overviewExitTarget.value = null;
    });
  });
  const layout = () => resizeDrag.value?.preview ?? tilingState.value;
  const multiPane = () => hasTilingGaps(layout());
  function measure(): void {
    if (areaId === undefined) return;
    const w = jsnir.columns.width[areaId] ?? 0;
    const h = jsnir.columns.height[areaId] ?? 0;
    if (w > 0 && (w !== measured.value[0] || h !== measured.value[1])) measured.value = [w, h];
  }
  // Re-measure when the window or the sidebar change the tiling area.
  effect(() => {
    void viewportSize.value;
    void sidebarCompact.value;
    void noticeState.value;
    afterView(16, measure);
    afterView(120, measure);
  });

  // Test the pointer against the current layout, then render the destination from the layout
  // that would actually result from dropping the window there.
  let dropKey = "";
  let dropBase: TilingView | undefined;
  let dropLayout: TilingView | null = null;
  effect(() => {
    const drag = windowDrag.value;
    const current = tilingState.value;
    const dimensions = area.value;
    const request = drag && areaId !== undefined
      ? computeDrop(current, drag.x - (jsnir.columns.x[areaId] ?? 0),
          drag.y - (jsnir.columns.y[areaId] ?? 0), dimensions)
      : null;
    if (!drag || !request) {
      dropKey = "";
      dropTarget.value = null;
      return;
    }
    const key = JSON.stringify([drag.id, request]);
    if (key !== dropKey || dropBase !== current) {
      dropKey = key;
      dropBase = current;
      dropLayout = previewRequest<TilingView>({ type: "moveWindow", preview: true, id: drag.id, ...request });
    }
    const rect = dropLayout && findPaneRect(dropLayout, drag.id, dimensions);
    dropTarget.value = rect ? { request, rect } : null;
  });

  // Overview drag: the workspace under the pointer takes the window; the gap above a workspace
  // inserts a new one there (niri's drop between workspaces).
  effect(() => {
    const drag = overviewDrag.value;
    if (!drag || areaId === undefined) return;
    const height = area.value[1];
    const zoom = tilingMotion.overviewZoom;
    const pitch = workspacePitch(area.value) * zoom;
    const gap = pitch - height * zoom;
    const firstTop = height * (1 - zoom) / 2 - centeredWorkspace() * pitch;
    const y = drag.y - (jsnir.columns.y[areaId] ?? 0);
    const list = workspaceList();
    const index = Math.max(0, Math.min(list.length - 1, Math.floor((y - firstTop + gap) / pitch)));
    const source = list.findIndex((row) => row.panes.includes(drag.id));
    const insert = y < firstTop + index * pitch && !(index === source && list[index].panes.length === 1);
    const next = insert || index !== source ? { index, insert } : null;
    const before = overviewDrop.value;
    if (next?.index !== before?.index || next?.insert !== before?.insert) overviewDrop.value = next;
  });
  onCleanup(() => {
    overviewDrag.value = null;
    overviewDrop.value = null;
    overviewDragged = false;
  });

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

      {/* niri monitor: the backdrop, then every workspace stacked and zoomed around the centred
          one. Columns are absolutely placed from the core layout so width, height and scroll
          changes animate (niri-style smooth view movement). */}
      <Box
        ref={(id) => {
          areaId = id;
          afterView(16, measure);
        }}
        style={{ grow: 1, minWidth: 0, minHeight: 0, clip: true }}
      >
        {/* Shows between workspaces while they move and around them in the overview; a click
            on it leaves the overview. */}
        <Box style={() => ({ position: "absolute", left: 0, top: 0, width: area.value[0], height: area.value[1] })}
          onPress={() => { if (overviewOpen2.value && !overviewDragged) closeOverviewMode(); }}
          paint={() => ({ background: palette().sidebarBg })}
        />
        {/* JSNIR scales a box about its centre, which is niri's zoom about the output centre. */}
        <Box style={() => ({ position: "absolute", left: 0, top: 0, width: area.value[0], height: area.value[1] })}
          paint={() => ({ scale: overviewZoom(), transitions: [springTransition("scale")] })}
        >
          <For each={() => workspaceList().map((_, index) => index)} key={(index) => index}
            style={() => ({ position: "absolute", left: 0, top: 0, width: area.value[0], height: area.value[1] })}
          >
            {(index) => <WorkspaceLayer index={index} area={() => area.value} active={layout} />}
          </For>
        </Box>

        <For
          each={() => {
            const view = tilingState.value;
            if (view.maximized || overviewOpen2.value) return [];
            const count = view.columns.length;
            return view.columns.flatMap((_, index) =>
              view.mode === "niri" ? (index === 0 ? [0, 1] : [index + 1]) :
              index + 1 < count ? [index + 1] : []);
          }}
          key={(boundary) => boundary}
          style={{ position: "absolute", left: 0, top: 0 }}
        >
          {(boundary) => (
            <ResizeHandle vertical={true}
              rect={() => columnBoundary(layout(), boundary(), area.value)}
              fraction={() => area.value[0] - (multiPane() ? spacing.paneGap : 0)}
              target={() => {
                const view = tilingState.value;
                const i = boundary();
                const right = view.columns[i];
                // Only niri's leading edge resizes from the left; shared borders trade width.
                const left = view.mode === "niri" && !!right && i === 0;
                return { column: (left ? right : view.columns[i - 1])?.id ?? 0,
                  left, workspace: view.activeWorkspace };
              }}
            />
          )}
        </For>

        <WindowDragOverlay area={() => area.value} />

        {/* An empty workspace (niri keeps one at the end): what to open here. */}
        <Show when={isEmptyWorkspace} style={{ grow: 1, minHeight: 0 }}>
          {() => <EmptyWorkspace width={() => area.value[0]} />}
        </Show>

        {/* Overview drag: a card of the window follows the pointer. */}
        <Show when={() => overviewDrag.value !== null} style={{ position: "absolute", left: 0, top: 0 }}>
          {() => (
            <Box
              style={() => {
                const [left, top] = dragPreviewPosition(overviewDrag.value, areaId, area.value);
                return { position: "absolute", left, top, layer: 50 };
              }}
            >
              <WindowGhost id={() => overviewDrag.value?.id ?? 0} />
            </Box>
          )}
        </Show>
      </Box>
    </Column>
  );
}

// One workspace of the stack, drawn live at full size: the stack's zoom shrinks it for the overview.
function WorkspaceLayer(props: { index: () => number; area: () => [number, number]; active: () => TilingView }): Element {
  const view = () => workspaceTilingView(props.index(), props.active());
  const isActive = () => props.index() === props.active().activeWorkspace;
  const multiPane = () => hasTilingGaps(view());
  const empty = () => view().columns.length === 0;
  const addWorkspace = () => empty() && props.index() === workspaceList().length - 1;
  const dropHere = () => overviewDrop.value?.index === props.index();
  const selected = () => overviewOpen2.value && addWorkspace() && overviewIndex.value === "addWorkspace";
  // Sizes that must read the same on screen at the overview zoom.
  const zoomed = (size: number) => size / tilingMotion.overviewZoom;
  return (
    <Box
      style={() => ({ position: "absolute", left: 0, top: 0, width: props.area()[0], height: props.area()[1] })}
      onPress={() => { if (overviewOpen2.value && !overviewDragged) openWorkspaceFromOverview(props.index()); }}
      paint={() => ({
        y: (props.index() - centeredWorkspace()) * workspacePitch(props.area()),
        radius: overviewOpen2.value ? zoomed(radii.panel) : 0,
        background: dropHere() && !overviewDrop.value?.insert ? accent().brandSoft : palette().appBg,
        edge: dropHere() || selected() ? accent().brand : 0,
        edgeWidth: zoomed(workspaceTiling.guideWidth),
        edgeSides: dropHere() && overviewDrop.value?.insert ? [1, 0, 0, 0] : [1, 1, 1, 1],
        cursor: overviewOpen2.value && addWorkspace() ? "pointer" : undefined,
        a11y: overviewOpen2.value && addWorkspace() ? { role: "button", label: "Add workspace" } : undefined,
        transitions: [workspaceTransition("y"), springTransition("radius"),
          smoothTransition("background", motion.fast), smoothTransition("edge", motion.fast)],
      })}
    >
      {/* Always mounted and shown by opacity (see ARCHITECTURE.md, JSNIR gap 8). At the top: the
          trailing workspace only peeks in below the centred one. */}
      <Column style={() => ({ position: "absolute", left: 0, top: 0, width: props.area()[0], height: props.area()[1],
        align: "center", padding: zoomed(emptyStateLayout.padding) })}>
        <Text lines={1} font={{ family: fontFamilies.sans, size: zoomed(typeScale.caption) }}
          paint={() => ({
            color: selected() || dropHere() ? palette().textPrimary : palette().textMuted,
            opacity: overviewOpen2.value && empty() ? 1 : 0,
            transitions: [smoothTransition("color", motion.fast), smoothTransition("opacity", motion.fast)],
          })}>
          {() => addWorkspace()
            ? overviewDrag.value ? "Drop here for a new workspace" : "Add workspace"
            : "No windows yet"}
        </Text>
      </Column>
      <For each={() => view().columns.map((column, index) => ({ ...column, index }))}
        key={(column) => column.id}
        style={() => ({ position: "absolute", left: 0, top: 0, width: props.area()[0], height: props.area()[1] })}
      >
        {(column) => {
          const rect = () => columnRect(view(), column().index, props.area());
          // Decided once, when the column appears: see `noteTilingChange`.
          const starting: Starting | undefined = openAnimation
            ? { opacity: 0, scale: tilingMotion.openScale } : undefined;
          // Panes that arrive later in this column open on their own.
          let columnSettled = false;
          afterView(32, () => { columnSettled = true; });
          return (
            <Box
              style={() => {
                const [, , width, height] = rect();
                return { position: "absolute", left: 0, top: 0, width, height };
              }}
              // Position goes through `x`/`y`: JSNIR's `left` transition moves only the box's own
              // paint, while `x` carries the panes, so niri's view scroll and close reflow slide.
              paint={() => ({ x: rect()[0], y: rect()[1], opacity: 1, scale: 1, resize: "crossfade", starting,
                transitions: tilingPaneTransitions() })}
            >
              <For each={() => column().panes.map((sid, index) => ({ sid, index }))} key={(pane) => pane.sid}
                style={{ grow: 1, minHeight: 0 }}
              >
                {(pane) => {
                  const geometry = () => paneSlot(view(), column().heights, pane().index, pane().sid, rect()[3]);
                  const opening = columnSettled && openAnimation;
                  const focused = () => isActive() && view().focusedSession === pane().sid;
                  return (
                    <Box
                      style={() => ({ position: "absolute", left: 0, top: 0, width: rect()[2], height: geometry()[1] })}
                      paint={() => ({
                        x: 0,
                        y: geometry()[0],
                        opacity: windowDrag.value?.id === pane().sid || overviewDrag.value?.id === pane().sid
                          ? workspaceTiling.dragSourceOpacity : 1,
                        scale: 1,
                        resize: "crossfade",
                        starting: opening ? { opacity: 0, scale: tilingMotion.openScale } : undefined,
                        exiting: paneExiting(pane().sid),
                        transitions: tilingPaneTransitions(),
                      })}
                    >
                      <Dynamic value={() => isTerminalId(pane().sid)} style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
                        {(terminal) => terminal ? (
                          <TerminalPane sessionId={() => pane().sid} focused={focused} multiPane={multiPane} />
                        ) : (
                          <TilePane id={() => pane().sid} focused={focused} multiPane={multiPane}
                            paintOffset={() => [rect()[0], rect()[1] + geometry()[0]]} />
                        )}
                      </Dynamic>
                      {/* Overview: the window itself is the target (click opens it, drag moves it
                          to another workspace). Zero-sized outside the overview so it never
                          takes the pane's own input. */}
                      <Box
                        style={() => ({ position: "absolute", left: 0, top: 0,
                          width: overviewOpen2.value ? rect()[2] : 0, height: overviewOpen2.value ? geometry()[1] : 0 })}
                        onPress={() => { if (!overviewDragged) pickFromOverview(pane().sid); }}
                        onDrag={overviewCardDrag(() => pane().sid)}
                        paint={() => ({
                          radius: multiPane() ? radii.popover : 0,
                          edge: overviewIndex.value === pane().sid ? accent().brand : 0,
                          edgeWidth: zoomed(workspaceTiling.guideWidth),
                          cursor: "grab",
                          transitions: [smoothTransition("edge", motion.fast)],
                        })}
                      />
                    </Box>
                  );
                }}
              </For>
              <For each={() => view().maximized || !isActive() || overviewOpen2.value ? []
                : column().panes.slice(0, -1).map((_, index) => index)}
                key={(row) => row} style={{ position: "absolute", left: 0, top: 0 }}
              >
                {(row) => (
                  <ResizeHandle vertical={false}
                    rect={() => {
                      const [top, height] = panePosition(column().heights, row(), rect()[3]);
                      return [0, top + height, rect()[2], spacing.paneGap];
                    }}
                    fraction={() => rect()[3] + spacing.paneGap}
                    target={() => ({ column: column().id, row: row(), workspace: tilingState.value.activeWorkspace })}
                  />
                )}
              </For>
            </Box>
          );
        }}
      </For>
    </Box>
  );
}

function isEmptyWorkspace(): boolean {
  return workspaceReady.value && !overviewOpen2.value && tilingState.value.columns.length === 0;
}

const ASCII_RAMP = " .·:-=+*";

// Draw the grid at its actual glyph advance and row pitch. The main panel clips the
// extra columns during the sidebar spring, so the pattern reaches the edge throughout.
// The field drifts over time and ripples out from the pointer while it moves.
function EmptyWorkspaceBackdrop(): Element {
  const layout = emptyWorkspaceLayout;
  const inset = workspaceTabLayout.stroke;
  const panelWidth = () => {
    const side = sidebarCompact.value ? spacing.sidebarRail : spacing.sidebarWidth;
    return Math.max(1, viewportSize.value[0] - side - spacing.workspaceFrame - 2 * inset);
  };
  const gridWidth = () => Math.max(1, viewportSize.value[0] - spacing.sidebarRail - spacing.workspaceFrame - 2 * inset);
  const height = () => Math.max(1, viewportSize.value[1] - 2 * (spacing.workspaceFrame + inset));
  const rowCount = () => Math.ceil(height() / layout.asciiCellH);
  const colCount = () => Math.ceil(gridWidth() / layout.asciiCellW) + 1;
  // One frame of the field: seconds of drift, and the eased pointer (grid pixels, presence 0..1).
  const field = signal({ t: 0, x: 0, y: 0, presence: 0 });
  // A modal over the backdrop freezes it: nothing to see, nothing to spend.
  const paused = () => projectDialog.value !== null || paletteRequest.value !== null;

  return (
    <Show when={isEmptyWorkspace} style={() => ({
      position: "absolute", left: inset, top: inset, width: gridWidth(), height: height(),
    })}>
      {() => {
        // The host reports the pointer relative to one node at a time (the metrics channel);
        // a hovered chart may borrow it, so each tick claims it back while it is free.
        const pointer = signal<MetricsPointer | null>(null);
        let node: number | undefined;
        let alive = true;
        let target: [number, number] | null = null;
        let movedAt = 0;
        // Plain copy of the field's time, so the pointer effect below does not subscribe to every frame.
        let clock = 0;
        const claim = () => {
          if (node === undefined || metricsPointerTarget) return;
          metricsPointerTarget = { id: node, position: pointer, clear: () => { pointer.value = null; } };
          requestHost({ type: "metricsPointer", id: node });
        };
        effect(() => {
          const point = pointer.value;
          if (!point) return;
          target = [point[1], point[2]];
          movedAt = clock;
        });
        let running = false;
        const tick = () => {
          if (!alive || paused()) {
            running = false;
            return;
          }
          claim();
          // Read outside any effect: the tick itself subscribes to nothing.
          const { t, x, y, presence } = field.value;
          const now = t + layout.asciiFrameMs / 1000;
          clock = now;
          const active = target !== null && now - movedAt < layout.asciiPointerIdle;
          const ease = layout.asciiPointerEase;
          const [tx, ty] = target ?? [x, y];
          // A ripple that has faded restarts at the pointer instead of sliding in from the old spot.
          field.value = {
            t: now,
            x: presence > 0.01 ? x + (tx - x) * ease : tx,
            y: presence > 0.01 ? y + (ty - y) * ease : ty,
            presence: presence + ((active ? 1 : 0) - presence) * ease,
          };
          afterView(layout.asciiFrameMs, tick);
        };
        // Deferred so the first tick runs outside this effect and subscribes it to nothing.
        effect(() => {
          if (paused() || running) return;
          running = true;
          afterView(0, tick);
        });
        onCleanup(() => {
          alive = false;
          if (node !== undefined && metricsPointerTarget?.id === node) {
            metricsPointerTarget = undefined;
            requestHost({ type: "metricsPointer", id: 0 });
          }
        });
        return (
          <Column ref={(id) => { node = id; }} style={{ grow: 1 }}>
            <For each={() => Array.from({ length: rowCount() }, (_, row) => row)} key={(row) => row}>
              {(row) => {
                const spans = (): Span[] => {
                  const r = row();
                  const cols = colCount();
                  const { t, x: px, y: py, presence } = field.value;
                  const dark = themeMode.value === "dark";
                  const base = withAlpha(palette().textMuted, dark ? layout.asciiBaseAlphaDark : layout.asciiBaseAlphaLight);
                  const peak = withAlpha(accent().brandText, dark ? layout.asciiAccentAlphaDark : layout.asciiAccentAlphaLight);
                  const ny = (r + 0.5) * layout.asciiCellH / height();
                  const width = panelWidth();
                  const radius = layout.asciiPointerRadius;
                  const dy = (r + 0.5) * layout.asciiCellH - py;
                  // Rows out of the pointer's reach skip the ripple math.
                  const lit = presence > 0.01 && Math.abs(dy) < radius * 2.5;
                  const out: Span[] = [];
                  let text = "";
                  let color = base;
                  for (let c = 0; c < cols; c++) {
                    const nx = (c + 0.5) * layout.asciiCellW / width;
                    const dist = Math.hypot((nx - 0.5) / 0.34, (ny - 0.52) / 0.30);
                    const quiet = Math.max(0, Math.min(1, (dist - 0.55) / 0.85));
                    const wave = (Math.sin(c * 0.22 + r * 0.18 + t * layout.asciiDrift)
                      + Math.cos(r * 0.34 + c * 0.09 - t * layout.asciiDrift * 0.7)) * 0.25 + 0.5;
                    let intensity = quiet * wave;
                    if (lit) {
                      const d = Math.hypot((c + 0.5) * layout.asciiCellW - px, dy);
                      const falloff = Math.exp(-(d * d) / (radius * radius));
                      const ripple = 0.6 + 0.4 * Math.sin(d * layout.asciiRippleScale - t * layout.asciiRippleSpeed);
                      intensity = Math.min(1, intensity + presence * falloff * ripple * (1 - intensity * 0.5));
                    }
                    const index = Math.min(ASCII_RAMP.length - 1, Math.floor(intensity * ASCII_RAMP.length));
                    const nextColor = intensity > 0.52 ? peak : base;
                    if (nextColor !== color && text.length > 0) {
                      out.push({ text, color });
                      text = "";
                    }
                    color = nextColor;
                    text += ASCII_RAMP[index] + " ";
                  }
                  out.push({ text, color });
                  return out;
                };
                return (
                  <Text
                    lines={1}
                    style={{ height: layout.asciiCellH, shrink: 0 }}
                    font={{ family: fontFamilies.mono, size: typeScale.caption, lineHeight: 1 }}
                    spans={spans}
                  >
                    {" "}
                  </Text>
                );
              }}
            </For>
          </Column>
        );
      }}
    </Show>
  );
}

function EmptyWorkspace(props: { width: () => number }): Element {
  const layout = emptyWorkspaceLayout;
  const inset = emptyStateLayout.padding;
  const width = () => Math.min(layout.contentWidth, Math.max(0, props.width() - inset * 2));
  const shortcuts = (id: string) => () => width() >= layout.shortcutMinWidth ? keysFor(id) : [];
  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0, padding: inset, align: "center", scroll: true }}>
      <Box style={{ grow: 1 }} />
      <Column style={() => ({ width: width(), shrink: 0 })}>
        <EmptyState title="Empty workspace">
          <MenuItem icon="square-terminal" label="New session" keys={shortcuts("session.new")}
            onPress={createSession} />
          <MenuItem icon="folder-open" label="Add project folder" onPress={() => showProjectDialog("add")} />
          <MenuItem icon="search" label="Open launcher" keys={shortcuts("panel.side")}
            onPress={() => togglePalette("launcher")} />
          <MenuItem icon="workspaces" label="Workspace overview" keys={shortcuts("view.overview")}
            onPress={() => setViewMode("overview")} />
        </EmptyState>
      </Column>
      <Box style={{ grow: 1 }} />
    </Column>
  );
}

// Resize and drop previews share the same geometry as the rendered layout.
type TilingRect = [number, number, number, number];

function hasTilingGaps(view: TilingView): boolean {
  return !view.maximized && (view.columns.length > 1 || (view.columns[0]?.panes.length ?? 0) > 1);
}

// Niri fullscreen: the focused column grows to the whole area and pushes its neighbours out of
// the sides at their tiled widths, so entering and leaving springs between the two layouts.
function fullscreenColumnRect(view: TilingView, index: number, area: [number, number]): TilingRect {
  const focused = view.columns.findIndex((column) => column.panes.includes(view.focusedSession));
  const width = (i: number) => Math.round(view.columns[i].width * area[0]);
  let left = 0;
  if (index < focused) for (let i = index; i < focused; i++) left -= width(i);
  if (index > focused) {
    left = area[0];
    for (let i = focused + 1; i < index; i++) left += width(i);
  }
  return [left, 0, index === focused ? area[0] : width(index), area[1]];
}

// Pane slot inside its column; in fullscreen the focused pane takes the whole column and the
// panes stacked above / below slide out of the top / bottom edge.
function paneSlot(view: TilingView, heights: number[], index: number, id: number, height: number): [number, number] {
  if (!view.maximized) return panePosition(heights, index, height);
  const own = Math.round((heights[index] ?? 0) * height);
  if (id === view.focusedSession) return [0, height];
  const focused = view.columns.find((column) => column.panes.includes(view.focusedSession));
  const row = focused?.panes.indexOf(id) ?? -1;
  if (!focused || row < 0) return panePosition(heights, index, height);
  const above = row < focused.panes.indexOf(view.focusedSession);
  return [above ? -own : height, own];
}

function columnRect(view: TilingView, index: number, area: [number, number]): TilingRect {
  const column = view.columns[index];
  if (!column) return [0, 0, 0, 0];
  if (view.maximized) return fullscreenColumnRect(view, index, area);
  const gap = hasTilingGaps(view) ? spacing.paneGap : 0;
  const usable = Math.max(0, area[0] - gap);
  const x = (column.x - view.viewOffset) * usable;
  const left = Math.round(x + gap);
  return [left, gap, Math.max(0, Math.round(x + column.width * usable) - left), Math.max(0, area[1] - gap * 2)];
}

function panePosition(heights: number[], index: number, height: number): [number, number] {
  const gap = heights.length > 1 ? spacing.paneGap : 0;
  const total = height + gap;
  const start = heights.slice(0, index).reduce((sum, share) => sum + share, 0);
  const top = Math.round(start * total);
  return [top, Math.max(0, Math.round((start + (heights[index] ?? 0)) * total - gap) - top)];
}

function columnBoundary(view: TilingView, index: number, area: [number, number]): TilingRect {
  const [x, y, w, h] = columnRect(view, Math.max(0, index - 1), area);
  return [index === 0 ? x : x + w, y, hasTilingGaps(view) ? spacing.paneGap : 0, h];
}

interface ResizeTarget {
  column: number;
  row?: number;
  left?: boolean;
  workspace: number;
}

interface ResizeDrag {
  target: ResizeTarget;
  fraction: number;
  preview: TilingView | null;
}

const resizeDrag = signal<ResizeDrag | null>(null);

function previewRequest<T>(payload: Record<string, unknown>): T | null {
  try {
    return JSON.parse(jsnirRequest(JSON.stringify(payload))) as T | null;
  } catch {
    return null;
  }
}

function ResizeHandle(props: {
  vertical: boolean;
  rect: () => TilingRect;
  fraction: () => number;
  target: () => ResizeTarget;
}): Element {
  const hovered = signal(false);
  const dragging = signal(false);
  const layout = workspaceTiling;
  return (
    <Box
      style={() => {
        const [x, y, w, h] = props.rect();
        const grab = layout.resizeHitSlop;
        return props.vertical
          ? { position: "absolute", left: x - grab, top: y, width: w + grab * 2, height: h, align: "center", justify: "center" }
          : { position: "absolute", left: x, top: y - grab, width: w, height: h + grab * 2, align: "center", justify: "center" };
      }}
      onHover={(inside) => { hovered.value = inside; }}
      onDrag={(event) => {
        if (event.phase === "start") {
          dragging.value = true;
          resizeDrag.value = { target: props.target(), fraction: Math.max(1, props.fraction()), preview: null };
          return;
        }
        const drag = resizeDrag.value;
        if (!dragging.value || !drag) return;
        const delta = (props.vertical ? event.dx : event.dy) / drag.fraction;
        const preview = event.phase === "move";
        const view = previewRequest<TilingView>({ type: "tilingResize", ...drag.target, delta, preview });
        if (preview) {
          resizeDrag.value = { ...drag, preview: view };
        } else {
          committedResize.value = true;
          if (view) tilingState.value = view;
          resizeDrag.value = null;
          dragging.value = false;
        }
      }}
      paint={{ cursor: props.vertical ? "colResize" : "rowResize" }}
    >
      <Box
        style={props.vertical
          ? { width: layout.guideWidth, grow: 1, margin: [layout.guideInset, 0, layout.guideInset, 0] }
          : { height: layout.guideWidth, grow: 1, margin: [0, layout.guideInset, 0, layout.guideInset] }}
        paint={() => ({
          radius: radii.xs, background: accent().brand,
          opacity: dragging.value ? 1 : hovered.value ? layout.resizeHoverOpacity : 0,
          transitions: [smoothTransition("opacity", motion.fast)],
        })}
      />
    </Box>
  );
}

// ---- Overview (niri overview): the workspace stack zoomed out (see `WorkspaceLayer`). Arrows
// select, click or Enter opens, and Esc returns. Drag a window onto another workspace to move it
// there, or onto the gap above a workspace to put it in a new workspace there (niri's drop
// "between workspaces"); the last, empty workspace takes a window as a new one.

interface OverviewDrop {
  index: number;
  insert: boolean;
}

const overviewDrag = signal<WindowDrag | null>(null);
const overviewDrop = signal<OverviewDrop | null>(null);
// True from a window drag's first move until just after release, so the release isn't a click.
let overviewDragged = false;

function overviewCardDrag(id: () => number): (event: DragEvent) => void {
  let dragging = false;
  return (event) => {
    if (event.phase === "start") {
      dragging = false;
      return;
    }
    if (event.phase === "move") {
      if (!dragging && Math.abs(event.dx) + Math.abs(event.dy) < workspaceTiling.dragThreshold) return;
      dragging = true;
      overviewDragged = true;
      overviewDrag.value = { id: id(), x: event.x, y: event.y };
      return;
    }
    const drop = overviewDrop.value;
    overviewDrag.value = null;
    overviewDrop.value = null;
    if (dragging && drop && event.phase === "end") {
      overviewIndex.value = id();
      requestHost({ type: "moveToWorkspace", id: id(), index: drop.index, insert: drop.insert });
    }
    dragging = false;
    afterView(motion.fast, () => { overviewDragged = false; });
  };
}

function WindowThumbnail(props: { id: () => number; height: () => number; screen?: () => ScreenView | null }): Element {
  const terminalScreen = () => tileOf(props.id()) ? null : (screensMapState.value[String(props.id())] ?? props.screen?.() ?? null);
  const width = signal(0);
  let boxId: number | undefined;
  const measure = () => {
    const w = boxId === undefined ? 0 : jsnir.columns.width[boxId] ?? 0;
    if (w !== width.value) width.value = w;
  };
  effect(() => {
    void props.height();
    void viewportSize.value;
    afterView(16, measure);
    afterView(SPRING_MS + 40, measure);
  });
  // The font that fits the terminal's full cols × rows grid inside the card, never above the
  // pane's own size; other windows keep the fixed preview size.
  const gridFont = () => {
    const screen = terminalScreen();
    if (!screen || width.value <= 0) return workspaceTiling.previewTypeSize;
    const layout = workspaceTiling;
    const w = width.value - layout.previewPadding * 2;
    const h = props.height() - layout.previewHeaderHeight - spacing.unit * 2;
    const size = Math.min(w / (Math.max(1, screen.cols) * terminalGridLayout.advance),
      h / (Math.max(1, screen.rowsCount) * terminalGridLayout.lineHeight));
    return Math.max(layout.previewMinTypeSize, Math.min(terminalFontSize.value, Math.floor(size * 4) / 4));
  };
  // Thumbnail text: the terminal's whole grid, or a window's first lines.
  const rows = () => {
    const layout = workspaceTiling;
    const available = props.height() - layout.previewHeaderHeight - layout.previewPadding * 2;
    const lines = Math.max(1, Math.floor(available / layout.previewLineHeight));
    const spec = tileOf(props.id());
    if (spec) {
      const doc = docCache.value[docKey(spec)];
      if (spec.kind === "files") {
        return (treeDirectory(projectTree(spec.cwd ?? workspacePath.value), "").value ?? []).slice(0, lines)
          .map((entry, idx) => ({ idx, plain: entry.isDir ? `${entry.name}/` : entry.name, spans: [] as Span[] }));
      }
      if (spec.kind === "review") {
        return projectChanges(spec.cwd).files.slice(0, lines)
          .map((file, idx) => ({ idx, plain: `${file.status} ${file.path}  +${file.additions} -${file.deletions}`, spans: [] as Span[] }));
      }
      const code = spec.kind === "diff" ? doc?.diff : doc?.lines;
      const start = spec.kind === "diff" ? Math.max(0, (doc?.diffFocus ?? 0) - Math.floor(lines / 3)) : 0;
      return (code ?? []).slice(start, start + lines).map((line, idx) => ({ idx, plain: line.text, spans: codeSpans(line) }));
    }
    // A terminal shows its whole grid, scaled to the card (see `gridFont`), like niri's overview.
    const all = terminalScreen()?.rows ?? [];
    return all.map((row) => ({
      ...row,
      spans: row.spans.map((span) => ({ text: span.text, color: span.fg, background: span.bg })),
    }));
  };
  return (
    <Column
      ref={(id) => { boxId = id; afterView(16, measure); }}
      style={{
        grow: 1, minHeight: 0, clip: true,
        padding: [spacing.unit, workspaceTiling.previewPadding, spacing.unit, workspaceTiling.previewPadding],
      }}
      paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [1, 0, 0, 0] })}
    >
      {/* A browser pane's preview is its page frame (the same image as the pane). */}
      <Show when={() => tileOf(props.id())?.kind === "browser"} style={{ grow: 1, minHeight: 0 }}>
        {() => <Image source={() => "browser:" + props.id()} style={{ grow: 1, minWidth: 0, minHeight: 0 }} />}
      </Show>
      <Dynamic value={gridFont} style={{ minHeight: 0 }}>
        {(size) => {
          const lineHeight = terminalScreen() ? terminalGridLayout.lineHeight : 1.25;
          return (
            <For each={rows} key={(row) => row.idx}>
              {(row) => (
                <Text lines={1}
                  font={{ family: fontFamilies.mono, size, lineHeight }}
                  paint={() => ({ color: palette().textSecondary })}
                  spans={() => row().spans}
                >
                  {() => row().plain || " "}
                </Text>
              )}
            </For>
          );
        }}
      </Dynamic>
    </Column>
  );
}

// A window's icon: the CLI logo for terminals, the kind glyph for other windows.
function WindowIcon(props: { id: () => number; size: number; color?: () => number }): Element {
  return (
    <Dynamic value={() => { const t = tileOf(props.id()); return t ? tileGlyph(t) : ""; }} style={{ shrink: 0 }}>
      {(glyph) =>
        glyph ? (
          <Glyph name={glyph} size={props.size} color={props.color ?? (() => palette().textMuted)} />
        ) : (
          <Image
            source={() => "cli:" + (sessionsState.value.items.find((s) => s.id === props.id())?.cli ?? "terminal")}
            style={{ width: props.size, height: props.size }}
          />
        )
      }
    </Dynamic>
  );
}

// A thumbnail of the dragged window with the same content as its Overview card.
function WindowGhost(props: { id: () => number; screen?: () => ScreenView | null }): Element {
  const layout = workspaceTiling;
  const title = () => {
    const spec = tileOf(props.id());
    if (spec) return tileTitle(spec, props.id());
    return cleanTitle(sessionsState.value.items.find((s) => s.id === props.id())?.title ?? "");
  };
  return (
    <Column
      style={{ width: layout.dragPreviewWidth, height: layout.dragPreviewHeight, clip: true }}
      paint={() => ({
        background: palette().appBg, radius: radii.xl, edge: accent().brand,
        edgeWidth: spacing.hairline, shadows: [shadows.dragGhost],
        starting: { opacity: 0, scale: 0.95 },
        transitions: [smoothTransition("opacity", motion.fast), smoothTransition("scale", motion.fast)],
      })}
    >
      <Row style={{
        height: layout.previewHeaderHeight, align: "center", gap: spacing.unit * 2, shrink: 0,
        padding: [0, layout.previewPadding, 0, layout.previewPadding],
      }}>
        <WindowIcon id={props.id} size={workspaceTabLayout.icon} />
        <Text lines={1} style={{ grow: 1, minWidth: 0 }}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          paint={() => ({ color: palette().textPrimary })}
        >
          {title}
        </Text>
      </Row>
      <WindowThumbnail id={props.id} height={() => layout.dragPreviewHeight} screen={props.screen} />
    </Column>
  );
}

function dragPreviewPosition(drag: WindowDrag | null, origin: number | undefined, area: [number, number]): [number, number] {
  if (!drag || origin === undefined) return [0, 0];
  const layout = workspaceTiling;
  const x = drag.x - (jsnir.columns.x[origin] ?? 0);
  const y = drag.y - (jsnir.columns.y[origin] ?? 0);
  const gap = spacing.paneGap;
  const fitsRight = x + gap + layout.dragPreviewWidth <= area[0];
  const fitsBelow = y + gap + layout.dragPreviewHeight <= area[1];
  return [
    Math.round(Math.max(0, Math.min(area[0] - layout.dragPreviewWidth, fitsRight ? x + gap : x - gap - layout.dragPreviewWidth))),
    Math.round(Math.max(0, Math.min(area[1] - layout.dragPreviewHeight, fitsBelow ? y + gap : y - gap - layout.dragPreviewHeight))),
  ];
}

// ---- Window drag and drop (niri interactive move / Hyprland `movewindow`): drag a window's
// header (or its tab) and drop it between columns (new column) or above / below a pane (stacked
// into that column). While dragging, a ghost follows the pointer and an insert hint shows where
// the window will land; the drop goes to icy-core (`move_pane`).

interface WindowDrag {
  id: number;
  x: number;
  y: number;
  screen?: ScreenView | null;
}

type DropRequest = { newColumn: number } | { col: number; row: number };

interface DropTarget {
  request: DropRequest;
  rect: TilingRect;
}

const windowDrag = signal<WindowDrag | null>(null);
const dropTarget = signal<DropTarget | null>(null);

function findPaneRect(view: TilingView, id: number, area: [number, number]): TilingRect | null {
  const index = view.columns.findIndex((column) => column.panes.includes(id));
  if (index < 0) return null;
  const column = view.columns[index];
  const [x, top, width, height] = columnRect(view, index, area);
  const [y, paneHeight] = panePosition(column.heights, column.panes.indexOf(id), height);
  return [x, top + y, width, paneHeight];
}

function computeDrop(view: TilingView, x: number, y: number, area: [number, number]): DropRequest | null {
  const cols = view.columns;
  if (cols.length === 0 || x < 0 || x > area[0] || y > area[1] || y < -spacing.headerHeight) return null;
  const gap = hasTilingGaps(view) ? spacing.paneGap : 0;
  const rects = cols.map((_, index) => columnRect(view, index, area));
  const col = rects.findIndex(([left, , width]) => x >= left - gap / 2 && x <= left + width + gap / 2);
  if (col < 0) return { newColumn: x < rects[0][0] ? 0 : cols.length };
  const [left, top, width, height] = rects[col];
  const edge = Math.min(workspaceTiling.insertionZoneMax, width * workspaceTiling.insertionZoneFraction);
  if (y < 0) return { newColumn: x < left + width / 2 ? col : col + 1 };
  if (x < left + edge) return { newColumn: col };
  if (x > left + width - edge) return { newColumn: col + 1 };
  const heights = cols[col].heights;
  const row = heights.findIndex((_, index) => {
    const [offset, paneHeight] = panePosition(heights, index, height);
    return y < top + offset + paneHeight / 2;
  });
  return { col, row: row < 0 ? heights.length : row };
}

// onDrag for a window header or tab: a press that moves past the threshold starts the drag.
function windowDragHandler(id: () => number): (e: DragEvent) => void {
  let dragging = false;
  return (e) => {
    if (e.phase === "start") {
      dragging = false;
      return;
    }
    if (e.phase === "move") {
      if (!dragging) {
        if (Math.abs(e.dx) + Math.abs(e.dy) < workspaceTiling.dragThreshold) return;
        if (tilingState.value.maximized || paneOrder().length < 2 || activeNav.value !== "agents") return;
        dragging = true;
      }
      const windowId = id();
      const previous = windowDrag.value;
      const screen = previous?.id === windowId ? previous.screen : isTerminalId(windowId)
        ? screensMapState.value[String(windowId)] ?? previewRequest<ScreenView>({ type: "previewScreen", id: windowId })
        : null;
      windowDrag.value = { id: windowId, x: e.x, y: e.y, screen };
      return;
    }
    const target = dropTarget.value;
    windowDrag.value = null;
    dropTarget.value = null;
    if (dragging && target) requestHost({ type: "moveWindow", id: id(), ...target.request });
    dragging = false;
  };
}

function WindowDragOverlay(props: { area: () => [number, number] }): Element {
  let overlayId: number | undefined;
  const ghost = () => dragPreviewPosition(windowDrag.value, overlayId, props.area());
  return (
    <Box
      ref={(id) => {
        overlayId = id;
      }}
      style={{ position: "absolute", left: 0, top: 0, width: 0, height: 0 }}
    >
      {/* Insert hint: where the window lands (niri's insert hint), springing between targets. */}
      <Show when={() => windowDrag.value !== null && dropTarget.value !== null} style={{ position: "absolute", left: 0, top: 0 }}>
        {() => (
          <Box
            style={() => {
              const [x, , w, h] = dropTarget.value?.rect ?? [0, 0, 0, 0];
              return { position: "absolute", left: Math.round(x), top: 0, width: Math.round(w), height: Math.round(h) };
            }}
            paint={() => ({
              // Vertical position as `y` (springs; `top` does not animate).
              y: Math.round(dropTarget.value?.rect[1] ?? 0),
              radius: radii.popover,
              background: accent().brandSoft,
              edge: accent().brand,
              edgeWidth: workspaceTiling.insertionEdgeWidth,
              starting: { opacity: 0 },
              transitions: [
                springTransition("left"),
                springTransition("y"),
                springTransition("width"),
                springTransition("height"),
                smoothTransition("opacity", motion.fast),
              ],
            })}
          />
        )}
      </Show>
      {/* Ghost: a small card of the dragged window following the pointer. */}
      <Show when={() => windowDrag.value !== null} style={{ position: "absolute", left: 0, top: 0 }}>
        {() => (
          <Box style={() => ({ position: "absolute", left: Math.round(ghost()[0]), top: Math.round(ghost()[1]), layer: 50 })}>
            <WindowGhost id={() => windowDrag.value?.id ?? 0} screen={() => windowDrag.value?.screen ?? null} />
          </Box>
        )}
      </Show>
    </Box>
  );
}
