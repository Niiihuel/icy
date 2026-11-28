// Clean, chrome-free terminal viewport. Raw keyboard input goes straight to the focused PTY;
// clicking an underlined file path or URL opens it directly.

// Silica / Niri window card shared by terminal panes and the other compositor windows (`TilePane`):
// rounded card with a hairline, the focused one gets the brighter ring and a soft shadow.
function paneCardPaint(focused: boolean, multiPane: boolean): Paint {
  return {
    radius: multiPane ? radii.popover : 0,
    background: windowUnderlay(palette().appBg),
    edge: multiPane ? (focused ? palette().ring : palette().border) : 0,
    // The width stays: fullscreen fades the border out with the tiling spring (niri interpolates
    // its border by `fullscreen_progress`) instead of dropping it in the first frame.
    edgeWidth: spacing.hairline,
    shadows: multiPane && focused ? [shadows.card] : [],
    // Corners spring with the tiling so fullscreen doesn't snap them square.
    transitions: [springTransition("edge"), springTransition("radius")],
  };
}

// Niri-style window frame (`border` + `clip-to-geometry`): the border sits outside the client
// surface (`padding: inset`) and the inner column clips all content (including opaque CEF browser
// images) to `radius - inset`, so no client buffer can paint over the window's border or corners.
function PaneFrame(props: {
  focused: () => boolean;
  multiPane: () => boolean;
  onHover?: (inside: boolean) => void;
  onPress?: () => void;
  children?: Children;
}): Element {
  // Reserve a full logical pixel so a 0.5px hairline never shares pixel 0 with an opaque client
  // surface (e.g. a CEF browser image) on 1x displays.
  const inset = workspaceTabLayout.inset;
  return (
    <Box
      style={() => ({ grow: 1, minWidth: 0, minHeight: 0, padding: props.multiPane() ? inset : 0 })}
      onHover={props.onHover}
      onPress={props.onPress}
      paint={() => paneCardPaint(props.focused(), props.multiPane())}
    >
      <Column
        style={{ grow: 1, minWidth: 0, minHeight: 0, clip: true }}
        paint={() => ({
          radius: props.multiPane() ? radii.popover - inset : 0,
          transitions: [springTransition("radius")],
        })}
      >
        {props.children}
      </Column>
    </Box>
  );
}

function terminalCellWidth(): number { return terminalFontSize.value * terminalGridLayout.advance; }
function terminalCellHeight(): number { return terminalFontSize.value * terminalGridLayout.lineHeight; }

function TerminalGrid(props: BoxProps): Element {
  const grid = Box(props);
  return {
    mount(parent, before) {
      // Scrollback replaces rows while their hover events can still be queued.
      // Use JSNIR's virtual-row callback guards so a recycled ID cannot open a link.
      const scope = childScope(true);
      (currentScope.children ??= []).push(scope);
      return inScope(scope, () => grid.mount(parent, before, null));
    },
  };
}

const terminalLinkKeys = new WeakMap<DetectedLink, string>();

function terminalLinkKey(link: DetectedLink | undefined): string {
  if (!link) return "";
  const known = terminalLinkKeys.get(link);
  if (known !== undefined) return known;
  const key = JSON.stringify(link);
  terminalLinkKeys.set(link, key);
  return key;
}

function TerminalCellRuns(props: {
  row: () => TerminalRow;
  onLinkHover?: (index: number, anchor: number, inside: boolean) => void;
  onLinkPress: (link: DetectedLink) => void;
}): Element {
  return <Dynamic value={() => terminalFontSize.value}>
    {(size) => {
      const font = { family: fontFamilies.mono, size, lineHeight: terminalGridLayout.lineHeight };
      return <For each={() => props.row().spans} key={(span) => {
        // A live row can replace a link between mouse-down and mouse-up.
        const index = span.linkIndex;
        const link = index === undefined ? undefined : props.row().links[index];
        return `${span.col}:${span.bold}:${index ?? ""}:${terminalLinkKey(link)}`;
      }}>
        {(span) => {
          const { bold, linkIndex } = span();
          const detected = linkIndex === undefined ? undefined : props.row().links[linkIndex];
          // A file path missing from the session cwd stays plain text: it could only open a "File not found" tab.
          // Its row is re-resolved on every frame, so the span becomes a link once the file exists.
          const link = detected?.kind === "file" && !detected.resolved ? undefined : detected;
          const text = () => span().text.trimEnd();
          const onLinkHover = props.onLinkHover;
          let spanId = 0;
          let hovered = false;
          const hover = linkIndex === undefined || !link || !onLinkHover ? undefined : (inside: boolean) => {
            hovered = inside;
            onLinkHover(linkIndex, spanId, inside);
          };
          if (hover) onCleanup(() => { if (hovered) hover(false); });
          return <Box ref={hover ? (id) => { spanId = id; } : undefined} onHover={hover}
            onPress={link ? () => props.onLinkPress(link) : undefined}
            style={() => ({ position: "absolute", top: 0, left: span().col * terminalCellWidth(),
            width: span().cells * terminalCellWidth(), height: terminalCellHeight() })}
            paint={() => ({ background: span().selected ? terminalSelectionBg() : span().image ? accent().brandSoft : span().bg })}>
            <Box style={() => ({ position: "absolute", top: 0, left: 0,
              width: span().cells * terminalCellWidth() + size * terminalGridLayout.textSlack })}>
              <Text font={bold ? { ...font, weight: terminalGridLayout.boldWeight } : font}
                paint={() => ({
                  color: span().image || (span().link && link) ? accent().brandText : span().fg,
                  underline: !span().image && (span().underline || (span().link && !!link)),
                })}>{text}</Text>
            </Box>
          </Box>;
        }}
      </For>;
    }}
  </Dynamic>;
}

function TerminalPane(props: {
  sessionId: () => number;
  focused: () => boolean;
  multiPane: () => boolean;
  resizePty?: () => boolean;
}): Element {
  const snapshot = () => screensMapState.value[String(props.sessionId())];
  // The host only streams on-screen panes: keep the last frame so a pane sliding out of (or back
  // into) view during fullscreen or a workspace switch shows its content, not an empty grid.
  let lastScreen: ScreenView = emptyScreen;
  const screen = (): ScreenView => (lastScreen = snapshot() ?? lastScreen);

  const sessionMeta = () =>
    sessionsState.value.items.find((s) => s.id === props.sessionId());

  const hovered = signal(false);
  // The wheel over the grid scrolls this terminal (focused or not), never the tiling: the host
  // gets the grid's origin and cell size to turn the pointer into a cell for mouse reports.
  let gridId: number | undefined;
  const gridHovered = signal(false);
  let pointedRow = 0;
  let lastSize = "";
  let sizeTimerPending = false;
  let settleAt = 0;
  const gridHeight = signal(0);
  const reportSize = () => {
    if (gridId === undefined || sessionMeta()?.alive === false) return;
    const width = jsnir.columns.width[gridId] ?? 0;
    const height = jsnir.columns.height[gridId] ?? 0;
    gridHeight.value = height;
    if (props.resizePty?.() === false || width <= 2 * terminalGridLayout.left || height <= 2 * terminalGridLayout.top) return;
    const cols = Math.max(2, Math.min(400, Math.floor((width - 2 * terminalGridLayout.left) / terminalCellWidth())));
    const rows = Math.max(1, Math.min(200, Math.floor((height - 2 * terminalGridLayout.top) / terminalCellHeight())));
    const key = `${props.sessionId()}:${cols}:${rows}`;
    if (key !== lastSize && requestHost({ type: "terminalSize", id: props.sessionId(), cols, rows })) lastSize = key;
  };
  const reportSettledSize = () => {
    const remaining = settleAt - Date.now();
    if (remaining > 0) { afterView(remaining, reportSettledSize); return; }
    sizeTimerPending = false;
    reportSize();
  };
  const claimWheel = () => {
    if (gridId === undefined || !gridHovered.value) return;
    requestHost({
      type: "terminalWheel",
      id: props.sessionId(),
      x: (jsnir.columns.x[gridId] ?? 0) + terminalGridLayout.left,
      y: (jsnir.columns.y[gridId] ?? 0) + terminalGridLayout.top,
      cellW: terminalCellWidth(),
      cellH: terminalCellHeight(),
      row: pointedRow,
      // The host places a selection drag from this node's drawn rect (it includes paint offsets).
      grid: gridId,
      padX: terminalGridLayout.left,
      padY: terminalGridLayout.top,
    });
  };
  // Fullscreen is one discrete step whose final layout is known now: resize the PTY at once, as
  // a niri client does when it gets the new size, so the program redraws while the tiling
  // springs instead of after it settles. Fullscreen following focus to another tab is the same step.
  const fullscreenHere = () => tilingState.value.maximized && tilingState.value.focusedSession === props.sessionId();
  let wasFullscreen = fullscreenHere();
  effect(() => {
    const full = fullscreenHere();
    if (full === wasFullscreen) return;
    wasFullscreen = full;
    afterView(terminalGridLayout.settleDelay, reportSize);
  });
  // Windows move under a still pointer (tiling springs, resizes): refresh the origin after them.
  effect(() => {
    void tilingState.value;
    void viewportSize.value;
    void terminalFontSize.value;
    void props.multiPane();
    void noticeState.value;
    void sidebarCompact.value;
    settleAt = Date.now() + SPRING_MS + terminalGridLayout.settleDelay;
    if (!sizeTimerPending) {
      sizeTimerPending = true;
      afterView(SPRING_MS + terminalGridLayout.settleDelay, reportSettledSize);
    }
  });
  effect(() => {
    void tilingState.value;
    void viewportSize.value;
    void terminalFontSize.value;
    if (!gridHovered.value) return;
    claimWheel();
    afterView(SPRING_MS + terminalGridLayout.settleDelay, claimWheel);
  });
  onCleanup(() => {
    if (gridHovered.value) requestHost({ type: "terminalWheel", id: 0, from: props.sessionId() });
  });
  const scrolledBack = () => screen().displayOffset ?? 0;
  const paneAction = (icon: string, label: string, keys: () => string[], run: () => void) => (
    <IconButton
      size="sm"
      side="bottom"
      icon={icon}
      label={label}
      keys={keys}
      onPress={() => {
        if (!props.focused()) selectSession(props.sessionId());
        run();
      }}
    />
  );

  // Silica terminal card: rounded card per pane, 30px header naming what runs (CLI icon + title)
  // with maximize / close on hover (stacking lives in the ⋮ menu, its shortcut and drag); the
  // focused card gets a brighter ring instead of a coloured border.
  return (
    <PaneFrame
      focused={props.focused}
      multiPane={props.multiPane}
      onHover={(inside) => {
        hovered.value = inside;
      }}
      onPress={() => {
        if (!props.focused()) {
          selectSession(props.sessionId());
        }
      }}
    >
      {/* Always mounted: fullscreen folds the header away with the tiling spring instead of
          jumping the grid up 30px in one frame. */}
      <Row
        style={() => ({ height: props.multiPane() ? 30 : 0, padding: [0, 4, 0, 10], align: "center", gap: 6, shrink: 0, clip: true })}
        onDrag={windowDragHandler(() => props.sessionId())}
        paint={() => ({
          cursor: "grab",
          opacity: props.multiPane() ? 1 : 0,
          transitions: [springTransition("height"), smoothTransition("opacity", motion.fast)],
        })}
      >
        <Image
          source={() => "cli:" + (sessionMeta()?.cli ?? "terminal")}
          style={{ width: 14, height: 14, shrink: 0 }}
        />
        <FadeText
          text={() => cleanTitle(sessionMeta()?.title ?? `pane-${props.sessionId()}`)}
          font={{ family: fontFamilies.sans, size: typeScale.label }}
          color={() => props.focused() ? palette().textPrimary : palette().textSecondary}
          background={() => palette().appBg}
          layoutChanged={() => { void tilingState.value; void resizeDrag.value; }}
        />
        <Row
          style={{ align: "center", gap: 0, shrink: 0 }}
          paint={() => ({
            opacity: hovered.value || props.focused() ? 1 : 0,
            transitions: [smoothTransition("opacity", motion.fast)],
          })}
        >
          <Show when={() => tilingState.value.mode === "niri" && !tilingState.value.maximized} style={{ shrink: 0 }}>
            {() => paneAction("columns", "Maximize column", () => keysFor("layout.maximizeColumn"), () => tilingAction("maximizeColumn"))}
          </Show>
          {paneAction("maximize", "Fullscreen pane", () => keysFor("view.fullscreen"), toggleTiledMaximize)}
          {paneAction("close", "Close", () => keysFor("session.close"), () => closeSession(props.sessionId()))}
        </Row>
      </Row>

      <Show when={() => sessionMeta()?.alive === false} style={{ shrink: 0 }}>
        {() => (
          <Row style={{ align: "center", gap: spacing.unit * 2, padding: [spacing.unit, spacing.unit * 3, 0, spacing.unit * 3] }}>
            <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
              paint={() => ({ color: palette().textMuted })}>
              Terminal exited
            </Text>
            <GhostButton icon="plus" label="New session" onPress={() => showProjectDialog("session", sessionMeta()?.cwd)} />
          </Row>
        )}
      </Show>

      {/* Pure terminal VT grid (Geist Mono, lineHeight multiplier 1.4) + inline images */}
      <TerminalGrid
        ref={(id) => { gridId = id; afterView(terminalGridLayout.settleDelay, reportSize); }}
        style={{ grow: 1, minHeight: 0, clip: true }}
        onHover={(inside) => {
          gridHovered.value = inside;
          if (!inside) requestHost({ type: "terminalWheel", id: 0, from: props.sessionId() });
        }}
      >
      {/* No frame from the host yet (new session, or a pane just mounted): skeleton lines in the
          empty grid's place, fading out when the first frame lands. */}
      <Show when={() => screen() === emptyScreen && sessionMeta()?.alive !== false} style={{ shrink: 0 }}>
        {() => <TerminalSkeleton />}
      </Show>
      <For
        each={() => screen().rows}
        key={(row) => row.idx * 2 + Number(row.links.length > 0)}
        style={{
          grow: 1,
          minHeight: 0,
          padding: [terminalGridLayout.top, terminalGridLayout.left, terminalGridLayout.top, terminalGridLayout.left],
          gap: 0,
        }}
      >
        {(row) => {
          const hasLinks = row().links.length > 0;
          // The link under the pointer (its span is the popover anchor), not the row's first one.
          const pointed = signal<{ index: number; anchor: number; key: string } | null>(null);
          const pointedLink = () => {
            const at = pointed.value;
            if (!at) return undefined;
            const link = row().links[at.index];
            return terminalLinkKey(link) === at.key ? link : undefined;
          };
          const imageLink = () => {
            const link = pointedLink();
            return link?.image ? link : undefined;
          };
          const codeLink = () => {
            const link = pointedLink();
            return link && !link.image && (link.kind === "diff" || (link.kind === "file" && link.resolved)) ? link : undefined;
          };
          const urlLink = () => {
            const link = pointedLink();
            return link?.kind === "url" ? link : undefined;
          };
          const hovered = signal(false);
          let cardHovered = false;
          let hoverToken = 0;
          let previewUse: number | undefined;
          const releasePreview = () => {
            if (previewUse !== undefined) hideUrlPreview(previewUse);
            previewUse = undefined;
          };
          effect(() => { if (!hovered.value) releasePreview(); });
          onCleanup(releasePreview);
          const dismissPreview = () => {
            ++hoverToken;
            releasePreview();
            cardHovered = false;
            hovered.value = false;
          };
          // Spans under the pointer per link: enter/leave order between spans is not guaranteed.
          const inside = new Map<number, number>();
          const leavePreview = () => {
            const mine = ++hoverToken;
            const delay = hovered.value && urlLink() ? urlPreviewLayout.leaveDelay : codePreviewLayout.leaveDelay;
            afterView(delay, () => {
              const index = pointed.value?.index;
              if (mine === hoverToken && !cardHovered && index !== undefined && !inside.get(index)) {
                dismissPreview();
                pointed.value = null;
              }
            });
          };
          const linkHover = (index: number, anchor: number, entering: boolean) => {
            const count = (inside.get(index) ?? 0) + (entering ? 1 : -1);
            inside.set(index, Math.max(0, count));
            if (!entering) {
              // Still on another span of this link, or another link already took over.
              if (count <= 0 && pointed.value?.index === index) leavePreview();
              return;
            }
            const key = terminalLinkKey(row().links[index]);
            const previous = pointed.value;
            const same = previous?.index === index && previous.key === key;
            pointed.value = { index, anchor: same ? previous.anchor : anchor, key };
            if (same && hovered.value) {
              ++hoverToken;
              return;
            }
            dismissPreview();
            const mine = hoverToken;
            const image = imageLink();
            const code = codeLink();
            const url = urlLink();
            if (image) requestImagePreview(image.target, props.sessionId());
            else if (code) requestCodePreview(code, props.sessionId());
            else if (!url) return;
            afterView(codePreviewLayout.hoverDelay, () => {
              if (mine !== hoverToken) return;
              const page = urlLink();
              // The page only loads once the pointer has rested on the link.
              if (page && isLocalUrl(page.target)) previewUse = showUrlPreview(page.target);
              if (imageLink() || codeLink() || page) hovered.value = true;
            });
          };
          let rowId: number | undefined;
          const openRowLink = (link: DetectedLink) => {
            dismissPreview();
            const id = props.sessionId();
            // The first click on another pane only focuses it, so switching panes never opens a link.
            if (!props.focused()) {
              selectSession(id);
              return;
            }
            if (link.image) openScreenImages(screen(), link.target, id);
            else openLink(link, id);
          };
          return (
            <Box
              ref={(id) => {
                rowId = id;
              }}
              onHover={(inside) => {
                if (inside) {
                  pointedRow = row().idx;
                  claimWheel();
                  return;
                }
                if (hovered.value && urlLink()) leavePreview();
                else {
                  dismissPreview();
                  pointed.value = null;
                }
              }}
              style={() => ({
                height: terminalCellHeight(),
                shrink: 0,
              })}
              paint={() => ({
                radius: radii.xs,
                hover: hasLinks ? { background: accent().brandSoft } : undefined,
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <TerminalCellRuns row={row} onLinkHover={hasLinks ? linkHover : undefined} onLinkPress={openRowLink} />
              <Show when={() => row().spans.length === 0 && !!row().plain}>
                {() => <Text lines={1} font={{ family: fontFamilies.mono, size: terminalFontSize.value,
                  lineHeight: terminalGridLayout.lineHeight }}>{() => row().plain}</Text>}
              </Show>
              <Show when={() => props.focused() && sessionMeta()?.alive === true &&
                screen().cursorVisible && screen().cursorRow === row().idx}
                style={{ position: "absolute", left: 0, top: 0 }}>
                {() => <Box style={() => ({
                  position: "absolute", left: screen().cursorCol * terminalCellWidth(),
                  top: terminalFontSize.value * terminalGridLayout.caretInset, width: spacing.unit / 2, height: terminalFontSize.value,
                })} paint={() => ({ background: palette().textPrimary })} />}
              </Show>
              {hasLinks && <Popover open={hovered} anchor={() => pointed.value?.anchor ?? rowId} side="top" align="start" layer={codePreviewLayout.layer}>
                <Dynamic value={() => (imageLink() ? "image" : codeLink() ? "code" : urlLink() ? "url" : "")}>
                  {(kind) =>
                    kind === "image" ? (
                      <ImagePreviewCard path={() => imageLink()?.target ?? ""} />
                    ) : kind === "code" ? (
                      <CodePreviewCard
                        link={codeLink}
                        sessionId={props.sessionId}
                        onOpen={() => {
                          const code = codeLink();
                          dismissPreview();
                          if (code) openLink(code, props.sessionId());
                        }}
                      />
                    ) : kind === "url" ? (
                      <UrlPreviewCard url={() => urlLink()?.target ?? ""}
                        onHover={(inCard) => {
                          cardHovered = inCard;
                          if (!inCard && !inside.get(pointed.value?.index ?? -1)) leavePreview();
                        }}
                        onOpen={() => {
                          const url = urlLink();
                          if (url) openRowLink(url);
                        }} />
                    ) : (
                      <Box />
                    )
                  }
                </Dynamic>
              </Popover>}
            </Box>
          );
        }}
      </For>
      <TerminalImages images={() => screen().images ?? []} />

      {/* Reading the history: how far back, and the way back to the live output (typing works too). */}
      <Show when={() => scrolledBack() > 0} style={() => ({ position: "absolute", left: 0, top: Math.max(0, gridHeight.value - spacing.control) })}>
        {() => (
          <Row
            style={{ height: spacing.control, padding: [0, spacing.unit * 2, 0, spacing.unit * 3.5], align: "center", gap: spacing.unit * 2 }}
            paint={() => ({
              background: palette().appBg,
              edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [1, 0, 0, 0],
              starting: { opacity: 0, y: 6 },
              transitions: [smoothTransition("opacity", motion.fast), smoothTransition("y", motion.fast)],
            })}
          >
            <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.caption }}
              paint={() => ({ color: palette().textMuted })}>
              {() => `${scrolledBack()} ${scrolledBack() === 1 ? "line" : "lines"} up in the history`}
            </Text>
            <GhostButton icon="chevron-down" label="Jump to latest"
              onPress={() => requestHost({ type: "scrollToBottom", id: props.sessionId() })} />
          </Row>
        )}
      </Show>
      </TerminalGrid>
    </PaneFrame>
  );
}

function TerminalSkeleton(): Element {
  const layout = terminalSkeleton;
  return (
    <Column
      style={{
        gap: layout.gap,
        padding: [terminalGridLayout.top, terminalGridLayout.left, terminalGridLayout.top, terminalGridLayout.left],
      }}
      paint={{ opacity: 1, starting: { opacity: 0 }, exiting: { opacity: 0 }, transitions: [smoothTransition("opacity", motion.base)] }}
    >
      {layout.lines.map((share) => (
        <Row style={{ height: layout.lineHeight }}>
          <Box style={{ grow: share, basis: 0 }} paint={() => skeletonPaint(palette().kbdBg, radii.xs)} />
          <Box style={{ grow: 1 - share, basis: 0 }} />
        </Row>
      ))}
    </Column>
  );
}

// Kitty-graphics images over the cell grid. The grid origin is the rows' padding (14px + 2px row
// inset, 10px top); a cell is 0.6em wide (Geist Mono advance) and 1.4em tall. Images keep their
// aspect inside the c×r cell box the program asked for; clicking opens the image viewer.
function TerminalImages(props: { images: () => TermImageView[] }): Element {
  const cellW = terminalCellWidth;
  const cellH = terminalCellHeight;
  return (
    <For
      each={props.images}
      key={(img) => `${img.key}@${img.row}:${img.col}`}
      style={{ position: "absolute", left: 0, top: 0 }}
    >
      {(img) => {
        const box = () => {
          const bw = img().cols * cellW();
          const bh = img().rows * cellH();
          const w = img().width || bw;
          const h = img().height || bh;
          const scale = Math.min(bw / w, bh / h);
          return [Math.max(1, Math.round(w * scale)), Math.max(1, Math.round(h * scale))];
        };
        return (
          <Box
            style={() => ({
              position: "absolute",
              left: Math.round(terminalGridLayout.left + img().col * cellW()),
              top: Math.round(terminalGridLayout.top + img().row * cellH()),
              width: box()[0],
              height: box()[1],
            })}
            onPress={() => {
              const all = props.images();
              openImageViewer(
                all.map((i) => ({ key: i.key, width: i.width, height: i.height })),
                Math.max(0, all.findIndex((i) => i.key === img().key))
              );
            }}
            paint={() => ({
              radius: radii.sm,
              hover: { brightness: 1.06 },
              transitions: [smoothTransition("brightness", motion.fast)],
            })}
          >
            {/* Fills the sized box (stretch + grow): Image styles are static in JSNIR. */}
            <Image source={() => img().key} style={{ grow: 1 }} />
          </Box>
        );
      }}
    </For>
  );
}

// Every resolved image path on this screen, so the viewer can step through them.
function openScreenImages(screen: ScreenView, clicked: string, sessionId: number): void {
  const paths: string[] = [];
  for (const row of screen.rows) {
    for (const link of row.links) if (link.image && !paths.includes(link.target)) paths.push(link.target);
  }
  for (const p of paths) requestImagePreview(p, sessionId);
  openImageViewer(
    paths.map((path) => ({ key: "", width: 0, height: 0, path })),
    Math.max(0, paths.indexOf(clicked))
  );
}

// Hover preview for an image path: thumbnail (fit 280x180), file name and pixel size.
function ImagePreviewCard(props: { path: () => string }): Element {
  const entry = () => fileImages.value[props.path()];
  const fit = () => {
    const e = entry();
    if (typeof e !== "object") return [280, 140];
    const s = Math.min(1, 280 / Math.max(1, e.width), 180 / Math.max(1, e.height));
    return [Math.max(40, Math.round(e.width * s)), Math.max(24, Math.round(e.height * s))];
  };
  return (
    <Column
      style={{ padding: [6, 6, 8, 6], gap: 6 }}
      paint={() => ({
        radius: radii.lg,
        ...popupMaterial(palette().appBg),
        edge: palette().border,
        edgeWidth: spacing.hairline,
        shadows: [shadows.menu, shadows.menuTight],
        ...accordeeTooltipMotion("top"),
      })}
    >
      <Box
        style={() => ({ width: fit()[0], height: fit()[1], clip: true })}
        paint={() => ({ radius: radii.md, background: palette().panelBg })}
      >
        <Dynamic value={() => {
          const e = entry();
          return typeof e === "object" ? e.key : e === "error" ? "error" : "loading";
        }} style={{ grow: 1 }}>
          {(key) =>
            key === "loading" || key === "error" ? (
              <Column style={{ grow: 1, align: "center", justify: "center" }}>
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.caption }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {key === "error" ? "Preview unavailable" : "Loading preview…"}
                </Text>
              </Column>
            ) : (
              <Image source={key} style={{ grow: 1 }} />
            )
          }
        </Dynamic>
      </Box>
      <Row style={{ padding: [0, 2, 0, 2], align: "center", gap: 8 }}>
        <Glyph name="file" size={13} color={() => palette().textMuted} />
        <Text
          lines={1}
          style={{ grow: 1, minWidth: 0 }}
          font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
          paint={() => ({ color: palette().textPrimary })}
        >
          {() => basename(props.path())}
        </Text>
        <Text
          font={{ family: fontFamilies.mono, size: typeScale.tiny }}
          paint={() => ({ color: palette().textMuted })}
        >
          {() => {
            const e = entry();
            return typeof e === "object" ? `${e.width}×${e.height}` : "";
          }}
        </Text>
      </Row>
      <Text
        style={{ padding: [0, 2, 0, 2] }}
        font={{ family: fontFamilies.sans, size: typeScale.tiny }}
        paint={() => ({ color: palette().textMuted })}
      >
        Click to open in the viewer
      </Text>
    </Column>
  );
}

const PREVIEW_CODE_FONT: Font = {
  family: fontFamilies.mono,
  size: codePreviewLayout.codeSize,
  lineHeight: codePreviewLayout.codeLineHeight,
};

// Hover preview for a CLI file or diff link: syntax-highlighted snippet around the target line
// or changed hunk; clicking opens the full tiling panel (`TilePane` / `DocView`).
function UrlPreviewCard(props: { url: () => string; onHover: (inside: boolean) => void; onOpen: () => void }): Element {
  const layout = urlPreviewLayout;
  const preview = () => urlPreview.value?.url === props.url() ? urlPreview.value : null;
  const state = () => browserStates.value[preview()?.id ?? 0];
  const view = () => state()?.error ? "error" : preview()?.loading ? "loading" : "page";
  return (
    <Column
      style={{ width: layout.width, padding: codePreviewLayout.padding, gap: codePreviewLayout.gap }}
      onHover={props.onHover}
      onPress={props.onOpen}
      paint={() => ({
        radius: radii.lg,
        cursor: "pointer",
        ...popupMaterial(palette().appBg),
        edge: palette().border,
        edgeWidth: spacing.hairline,
        shadows: [shadows.menu, shadows.menuTight],
        ...accordeeTooltipMotion("top"),
      })}
    >
      <Row style={{ gap: layout.headerGap, align: "center", padding: [0, spacing.unit, 0, spacing.unit] }}>
        <Glyph name="globe" size={layout.icon} color={() => palette().textMuted} />
        <Text lines={1} style={{ grow: 1, minWidth: 0 }} font={{ family: fontFamilies.mono, size: typeScale.caption }}
          paint={() => ({ color: palette().textSecondary })}>{props.url}</Text>
        <Text lines={1} style={{ shrink: 1, minWidth: 0 }} font={{ family: fontFamilies.sans, size: typeScale.tiny }}
          paint={() => ({ color: palette().textMuted })}>{() => view() === "page" ? state()?.title ?? "" : ""}</Text>
      </Row>
      <Show when={() => isLocalUrl(props.url())}>
        {() => (
          <Box style={{ width: layout.width - codePreviewLayout.padding[1] - codePreviewLayout.padding[3], height: layout.height, padding: workspaceTabLayout.inset }}
            paint={() => ({ radius: radii.md, edge: palette().border, edgeWidth: spacing.hairline })}>
            <Box style={{ grow: 1, minHeight: 0, clip: true }} paint={{ radius: radii.md - workspaceTabLayout.inset }}>
              <Dynamic value={view} style={{ grow: 1, minHeight: 0 }}>
                {(mode) => mode === "page"
                  ? <Image source={() => "browser:" + (preview()?.id ?? 0)} style={{ grow: 1, minWidth: 0, minHeight: 0 }} />
                  : mode === "error"
                    ? <Column style={{ grow: 1, align: "center", justify: "center" }}>
                        <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
                          paint={() => ({ color: palette().textMuted })}>{() => state()?.error ?? ""}</Text>
                      </Column>
                    : <Box style={{ grow: 1 }} paint={() => skeletonPaint(palette().panelBg, radii.md - workspaceTabLayout.inset)} />}
              </Dynamic>
            </Box>
          </Box>
        )}
      </Show>
      <Text font={{ family: fontFamilies.sans, size: typeScale.tiny }} style={{ padding: [0, spacing.unit, 0, spacing.unit] }}
        paint={() => ({ color: palette().textMuted })}>Click to open in a browser pane</Text>
    </Column>
  );
}

function CodePreviewCard(props: {
  link: () => DetectedLink | undefined;
  sessionId: () => number;
  onOpen?: () => void;
}): Element {
  const layout = codePreviewLayout;
  const entry = (): CodePreviewEntry | "loading" | undefined => {
    const link = props.link();
    return link ? codePreviews.value[codePreviewKey(link, props.sessionId())] : undefined;
  };
  const loaded = (): CodePreviewEntry | undefined => {
    const current = entry();
    return typeof current === "object" ? current : undefined;
  };
  const isDiff = () => {
    const data = loaded();
    return data ? data.mode === "diff" : props.link()?.kind === "diff";
  };
  const viewState = () => {
    const data = loaded();
    if (!data) return "loading";
    if (data.error) return "error";
    if (data.mode === "diff") return (data.diff?.length ?? 0) === 0 ? "empty" : "diff";
    return (data.lines?.length ?? 0) === 0 ? "empty" : "file";
  };
  const hiddenLines = () => {
    const data = loaded();
    if (!data || !data.truncated) return 0;
    const shown = data.mode === "diff" ? (data.diff?.length ?? 0) : (data.lines?.length ?? 0);
    return Math.max(0, data.totalLines - shown);
  };
  const headerPath = () => {
    const data = loaded();
    const link = props.link();
    const path = data?.displayPath || link?.target || "";
    const line = data?.targetLine ?? link?.line;
    return !isDiff() && line ? `${path}:${line}` : path;
  };

  return (
    <Column
      style={{ width: layout.width, padding: layout.padding, gap: layout.gap }}
      onPress={props.onOpen}
      paint={() => ({
        radius: radii.lg,
        background: palette().appBg,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        shadows: [shadows.menu, shadows.menuTight],
        ...accordeeTooltipMotion("top"),
      })}
    >
      <Column
        style={{ clip: true, padding: layout.codePadding }}
        paint={() => ({
          radius: radii.md,
          background: palette().panelBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
        })}
      >
        <Dynamic value={viewState}>
          {(state) =>
            state === "loading" ? (
              <Column style={{ height: layout.loadingHeight, align: "center", justify: "center" }}>
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.caption }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  Loading preview…
                </Text>
              </Column>
            ) : state === "error" ? (
              <Column
                style={{
                  minHeight: layout.loadingHeight,
                  padding: [spacing.unit * 3, spacing.unit * 3, spacing.unit * 3, spacing.unit * 3],
                  align: "center",
                  justify: "center",
                  gap: spacing.unit,
                }}
              >
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
                  paint={() => ({ color: palette().textSecondary })}
                >
                  {() => (loaded()?.singleEdit ? "Edit unavailable" : "Preview unavailable")}
                </Text>
                <Text
                  lines={2}
                  font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {() => loaded()?.error ?? ""}
                </Text>
              </Column>
            ) : state === "empty" ? (
              <Column style={{ height: layout.loadingHeight, align: "center", justify: "center" }}>
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.caption }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {() => (isDiff() ? "No text changes to display." : "Empty file")}
                </Text>
              </Column>
            ) : state === "diff" ? (
              <For
                each={() => (loaded()?.diff ?? []).map((line, index) => ({ line, index }))}
                key={(row) => `${row.index}:${row.line.kind}:${row.line.oldNum ?? ""}:${row.line.newNum ?? ""}`}
              >
                {(row) => {
                  const kind = () => row().line.kind;
                  const focused = () => loaded()?.diffFocus === row().index;
                  const tint = () =>
                    kind() === "add" ? palette().diffAddText : kind() === "del" ? palette().diffDelText : 0;
                  const sign = () => (kind() === "add" ? "+" : kind() === "del" ? "-" : " ");
                  return (
                    <Row
                      style={{
                        height: layout.rowHeight,
                        padding: [0, layout.lineEndPadding, 0, 0],
                        gap: layout.gutterGap,
                        align: "center",
                        shrink: 0,
                      }}
                      paint={() => ({
                        background:
                          kind() === "add" ? palette().diffAddBg : kind() === "del" ? palette().diffDelBg : 0,
                        edge: focused() ? accent().brand : tint(),
                        edgeWidth: focused() || tint() ? editorLayout.diffMarker : 0,
                        edgeSides: [0, 0, 0, 1],
                      })}
                    >
                      <Text
                        lines={1}
                        style={{ width: layout.gutterWidth, shrink: 0 }}
                        align="end"
                        font={PREVIEW_CODE_FONT}
                        paint={() => ({ color: focused() ? accent().brandText : palette().textMuted })}
                      >
                        {() => String(row().line.newNum ?? row().line.oldNum ?? "")}
                      </Text>
                      <Text
                        lines={1}
                        style={{ width: layout.signWidth, shrink: 0 }}
                        font={PREVIEW_CODE_FONT}
                        paint={() => ({ color: tint() || palette().textMuted })}
                      >
                        {sign}
                      </Text>
                      <Text
                        lines={1}
                        style={{ grow: 1, minWidth: 0 }}
                        font={PREVIEW_CODE_FONT}
                        spans={() => codeSpans(row().line)}
                      >
                        {() => (row().line.text.length > 0 ? row().line.text : " ")}
                      </Text>
                    </Row>
                  );
                }}
              </For>
            ) : (
              <For each={() => loaded()?.lines ?? []} key={(line) => String(line.num)}>
                {(line) => (
                  <Row
                    style={{
                      height: layout.rowHeight,
                      padding: [0, layout.lineEndPadding, 0, 0],
                      gap: layout.gutterGap,
                      align: "center",
                      shrink: 0,
                    }}
                    paint={() => ({
                      background: line().highlighted ? accent().brandSoft : 0,
                      edge: line().highlighted ? accent().brand : 0,
                      edgeWidth: line().highlighted ? editorLayout.diffMarker : 0,
                      edgeSides: [0, 0, 0, 1],
                    })}
                  >
                    <Text
                      lines={1}
                      style={{ width: layout.gutterWidth, shrink: 0 }}
                      align="end"
                      font={PREVIEW_CODE_FONT}
                      paint={() => ({ color: line().highlighted ? accent().brandText : palette().textMuted })}
                    >
                      {() => String(line().num)}
                    </Text>
                    <Text
                      lines={1}
                      style={{ grow: 1, minWidth: 0 }}
                      font={PREVIEW_CODE_FONT}
                      spans={() => codeSpans(line())}
                    >
                      {() => (line().text.length > 0 ? line().text : " ")}
                    </Text>
                  </Row>
                )}
              </For>
            )
          }
        </Dynamic>
        <Show when={() => hiddenLines() > 0}>
          {() => (
            <Row
              style={{
                padding: [spacing.unit, layout.lineEndPadding, 0, layout.gutterWidth + layout.gutterGap],
                margin: [spacing.unit, 0, 0, 0],
                align: "center",
              }}
              paint={() => ({
                edge: palette().border,
                edgeWidth: spacing.hairline,
                edgeSides: [1, 0, 0, 0],
              })}
            >
              <Text
                lines={1}
                font={{ family: fontFamilies.mono, size: typeScale.micro }}
                paint={() => ({ color: palette().textMuted })}
              >
                {() => `+${hiddenLines()} more ${hiddenLines() === 1 ? "line" : "lines"}`}
              </Text>
            </Row>
          )}
        </Show>
      </Column>
      <Row style={{ padding: [0, 2, 0, 2], align: "center", gap: spacing.unit * 2 }}>
        <Glyph name={() => (isDiff() ? "file-diff" : "file-code")} size={layout.headerIcon} color={() => palette().textMuted} />
        <Text
          lines={1}
          style={{ grow: 1, minWidth: 0 }}
          font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
          paint={() => ({ color: palette().textPrimary })}
        >
          {headerPath}
        </Text>
        <Show when={() => isDiff() && loaded() !== undefined && !loaded()?.error} style={{ shrink: 0 }}>
          {() => (
            <Row style={{ align: "center", gap: spacing.unit * 1.5, shrink: 0 }}>
              <DiffStat additions={() => loaded()?.additions ?? 0} deletions={() => loaded()?.deletions ?? 0} />
              <Show when={() => !!loaded()?.singleEdit} style={{ shrink: 0 }}>
                {() => (
                  <Text
                    lines={1}
                    font={{ family: fontFamilies.sans, size: typeScale.micro }}
                    paint={() => ({ color: palette().textMuted })}
                  >
                    single edit
                  </Text>
                )}
              </Show>
            </Row>
          )}
        </Show>
        <Show when={() => !isDiff() && (loaded()?.totalLines ?? 0) > 0} style={{ shrink: 0 }}>
          {() => (
            <Text
              lines={1}
              font={{ family: fontFamilies.mono, size: typeScale.tiny }}
              paint={() => ({ color: palette().textMuted })}
            >
              {() => {
                const count = loaded()?.totalLines ?? 0;
                return `${count} ${count === 1 ? "line" : "lines"}`;
              }}
            </Text>
          )}
        </Show>
      </Row>
      <Text
        style={{ padding: [0, 2, 0, 2] }}
        font={{ family: fontFamilies.sans, size: typeScale.tiny }}
        paint={() => ({ color: palette().textMuted })}
      >
        {() => (isDiff() ? "Click to open diff panel" : "Click to open file panel")}
      </Text>
    </Column>
  );
}
