// The window forwards horizontal wheel input to the strip under the pointer.
let horizontalWheelTarget: ((delta: number, smooth: boolean) => void) | undefined;

function scrollHoveredStrip(delta: number, smooth: boolean): void {
  horizontalWheelTarget?.(delta, smooth);
}

interface StripItemBounds {
  start: number;
  end: number;
}

function StripEdge(props: {
  side: "left" | "right";
  width: () => number;
  available: () => boolean;
  label: string;
  onPress: () => void;
}): Element {
  const hovered = signal(false);
  const layout = horizontalStripLayout;
  const left = props.side === "left";
  return (
    <Box style={() => ({
      width: props.width(), minWidth: 0, height: spacing.button, shrink: 0, clip: true,
      align: "center", justify: "center",
    })}>
      <Show when={() => props.width() > 0 && (props.available() || hovered.value)}>
        {() => (
          <HintTooltip label={props.label} side="top">
            <Box
              style={{
                width: layout.controlWidth, height: layout.controlHeight,
                align: "center", justify: "center",
              }}
              onHover={(inside) => { hovered.value = inside; }}
              disabled={() => !props.available()}
              onPress={props.onPress}
              paint={() => ({
                radius: radii.sm,
                opacity: props.available() ? 1 : layout.endOpacity,
                hover: { background: palette().sidebarActive },
                active: { scale: controlState.iconPressScale },
                starting: { opacity: 0, x: left ? spacing.unit : -spacing.unit },
                transitions: [
                  springTransition("x"),
                  springTransition("scale"),
                  smoothTransition("opacity", motion.fast),
                  smoothTransition("background", motion.fast),
                ],
              })}
            >
              <Glyph name={left ? "chevron-left" : "chevron-right"} size={layout.chevronSize}
                color={() => palette().textSecondary} hover={() => palette().textPrimary} />
            </Box>
          </HintTooltip>
        )}
      </Show>
    </Box>
  );
}

function HorizontalStrip(props: {
  contentWidth: () => number;
  label: string;
  stretch?: boolean;
  // Strip height; the header uses its full height so connected tabs can reach its bottom line.
  height?: number;
  activeItem?: () => StripItemBounds | undefined;
  resizing?: () => boolean;
  children: Children;
}): Element {
  const height = props.height ?? spacing.button;
  const viewport = signal(0);
  const offset = signal(0);
  const smooth = signal(false);
  const resizeGutter = signal<number | null>(null);
  let viewportId: number | undefined;
  let layoutGeneration = 0;

  const overflows = () => viewport.value > 0 && props.contentWidth() - viewport.value > spacing.hairline;
  // Keep the controls outside the clipping area, so neither an outdated measurement nor
  // an in-flight animation can paint over a tab title.
  const measuredGutter = () => overflows() ? Math.min(horizontalStripLayout.controlWidth, viewport.value / 3) : 0;
  const gutter = () => resizeGutter.value ?? measuredGutter();
  const visibleWidth = () => Math.max(0, viewport.value - gutter() * 2);
  const maxOffset = () => gutter() > 0 ? Math.max(0, props.contentWidth() - visibleWidth()) : 0;
  const scrollOffset = () => Math.min(offset.value, maxOffset());

  const measure = () => {
    const width = viewportId === undefined ? 0 : jsnir.columns.width[viewportId] ?? 0;
    if (width <= 0 || width === viewport.value) return false;
    viewport.value = width;
    return true;
  };

  const moveTo = (position: number, animate: boolean) => {
    const next = Math.max(0, Math.min(position, maxOffset()));
    if (next === offset.value) return;
    smooth.value = animate;
    offset.value = next;
  };

  const scrollBy = (delta: number, animate: boolean) => {
    measure();
    moveTo(scrollOffset() + delta, animate);
  };

  const page = (direction: number) => scrollBy(direction * visibleWidth(), true);

  function syncLayout(active: StripItemBounds | undefined): void {
    let next = scrollOffset();
    if (active && overflows()) {
      if (active.start < next) next = active.start;
      else if (active.end > next + visibleWidth()) next = Math.min(active.start, active.end - visibleWidth());
    }
    moveTo(next, true);
  }

  effect(() => {
    void viewportSize.value;
    void sidebarCompact.value;
    void props.contentWidth();
    const generation = ++layoutGeneration;
    if (props.resizing?.()) {
      // Keep the scroll controls in place while the tab edge follows the pointer.
      if (resizeGutter.value === null) resizeGutter.value = measuredGutter();
      return;
    }
    const active = props.activeItem?.();
    afterView(horizontalStripLayout.layoutDelay, () => {
      if (generation !== layoutGeneration) return;
      measure();
      resizeGutter.value = null;
      syncLayout(active);
    });
    // The sidebar width itself animates, so the first layout can precede the final clip.
    afterView(SPRING_MS + horizontalStripLayout.layoutDelay, () => {
      if (generation === layoutGeneration && measure()) syncLayout(active);
    });
  });

  const releaseWheel = () => {
    if (horizontalWheelTarget !== scrollBy) return;
    horizontalWheelTarget = undefined;
    requestHost({ type: "horizontalScrollCapture", on: false });
  };
  onCleanup(releaseWheel);

  return (
    <Row
      ref={(id) => { viewportId = id; }}
      style={() => ({
        width: props.stretch ? undefined : props.contentWidth(),
        shrink: props.stretch ? 0 : 1,
        minWidth: 0,
        height,
        align: "center",
      })}
      onHover={(inside) => {
        if (inside) {
          horizontalWheelTarget = scrollBy;
          requestHost({ type: "horizontalScrollCapture", on: true });
        } else releaseWheel();
      }}
    >
      <StripEdge side="left" width={gutter} available={() => scrollOffset() > 0}
        label={`Scroll ${props.label} left`} onPress={() => page(-1)} />
      <Box style={{ grow: 1, minWidth: 0, height, clip: true }}>
        <Row
          style={() => ({
            position: "absolute", left: 0, top: 0, height, align: "center",
            width: Math.max(props.contentWidth(), visibleWidth()),
          })}
          paint={() => ({ x: -scrollOffset(), transitions: smooth.value ? [springTransition("x")] : [] })}
        >
          {props.children}
        </Row>
      </Box>
      <StripEdge side="right" width={gutter} available={() => scrollOffset() < maxOffset()}
        label={`Scroll ${props.label} right`} onPress={() => page(1)} />
    </Row>
  );
}
