// Native JSNIR dashboard for local agent usage. It is a compositor window (`TileKind` "metrics"),
// so it lays itself out from its own measured width, not from the window's.

function MetricsText(props: {
  children: Source<string>;
  size?: number;
  weight?: number;
  color?: Source<number>;
  style?: Style;
  align?: TextAlign;
}): Element {
  return (
    <Text
      lines={1}
      style={props.style}
      align={props.align}
      font={{
        family: fontFamilies.sans,
        size: props.size ?? typeScale.caption,
        weight: props.weight ?? fontWeights.normal,
      }}
      paint={() => ({ color: props.color === undefined ? palette().textPrimary : read(props.color) })}
    >
      {props.children}
    </Text>
  );
}

// shortcut: one width for every Metrics window; two open at once (one per workspace) share it.
const metricsViewWidth = signal(0);

function metricsWidth(): number {
  const measured = metricsViewWidth.value;
  const available = measured > 0
    ? measured
    : viewportSize.value[0] - (sidebarCompact.value ? spacing.sidebarRail : spacing.sidebarWidth) - spacing.paneGap;
  return Math.max(0, Math.min(metricsLayout.maxContentWidth, available - 2 * metricsLayout.padding));
}

function metricsCardsFit(count: number): boolean {
  return metricsWidth() >= count * metricsLayout.cardMinWidth + (count - 1) * metricsLayout.gap;
}

function metricsPanelsFit(): boolean {
  return metricsWidth() >= 2 * metricsLayout.panelMinWidth + metricsLayout.gap;
}

function metricsPanelWidth(): number {
  return metricsPanelsFit() ? (metricsWidth() - metricsLayout.gap) / 2 : metricsWidth();
}

function metricsCardPaint(): Paint {
  return {
    radius: radii.lg,
    background: palette().cardBg,
    edge: palette().border,
    edgeWidth: spacing.hairline,
    hover: { edge: hoverBorder() },
    transitions: [smoothTransition("edge", motion.fast)],
  };
}

function scanMetrics(refresh: boolean): void {
  if (metricsScanState.value === "loading") return;
  metricsScanState.value = "loading";
  if (!requestHost({ type: "scanUsage", refresh })) metricsScanState.value = "error";
}

// ---- Shared chart pieces ---------------------------------------------------------------------

interface MetricsTipRow {
  key: string;
  label: string;
  value: string;
  color?: number;
}

type MetricsPointer = [id: number, x: number, y: number, width: number, height: number];
let metricsPointerTarget: { id: number; position: Signal<MetricsPointer | null>; clear: () => void } | undefined;

function updateMetricsPointer(pointer: MetricsPointer): void {
  if (metricsPointerTarget?.id === pointer[0]) metricsPointerTarget.position.value = pointer;
}

function metricsTooltipHeight(rows: number): number {
  const { tooltipPadding, tooltipRow } = metricsLayout;
  return 2 * tooltipPadding + (rows + 1) * tooltipRow + rows * spacing.unit;
}

function metricsTooltipOffset(pointer: number, size: number, extent: number, afterPointer: boolean): number {
  const gap = metricsLayout.tooltipOffset;
  const position = afterPointer ? pointer + gap : pointer - gap - size;
  return Math.round(Math.max(0, Math.min(position, extent - size)));
}

// ECharts-style tooltip: one card per chart that follows the pointer. It has no handlers and
// no layer of its own, so it never steals the hover from the points under it.
function MetricsTooltip(props: {
  hover: ReturnType<typeof metricsHover>;
  title: () => string;
  rows: () => MetricsTipRow[];
  preferAbove?: boolean;
}): Element {
  const { tooltipWidth, tooltipPadding, tooltipRow, tooltipOffset, mark } = metricsLayout;
  const pointer = () => props.hover.pointer.value;
  const rows = signal<MetricsTipRow[]>([]);
  // Pointer moves only change the paint offset; the width lives apart so the layout is untouched.
  const placement = signal<[number, number]>([0, 0]);
  const width = signal(tooltipWidth);
  let side: [boolean, boolean] | undefined;
  effect(() => { rows.value = props.rows(); });
  effect(() => {
    const point = pointer();
    if (!point) { side = undefined; return; }
    const [, x, y, extentX, extentY] = point;
    const tipWidth = Math.min(tooltipWidth, extentX);
    const height = metricsTooltipHeight(rows.value.length);
    // Choose the side once per hover. Clamp subsequent moves instead of jumping across the pointer.
    side ??= [x + tooltipOffset + tipWidth <= extentX, !props.preferAbove && y + tooltipOffset + height <= extentY];
    width.value = tipWidth;
    placement.value = [
      metricsTooltipOffset(x, tipWidth, extentX, side[0]),
      metricsTooltipOffset(y, height, extentY, side[1]),
    ];
  });
  return (
    <Column
      style={() => ({
        position: "absolute", left: 0, top: 0, width: width.value, padding: tooltipPadding, gap: spacing.unit,
      })}
      paint={() => ({
        x: placement.value[0],
        y: placement.value[1],
        opacity: pointer() ? 1 : 0,
        radius: radii.lg,
        background: metricsColors().tooltip,
        edge: palette().border,
        edgeWidth: spacing.hairline,
        shadows: [shadows.popover],
      })}
    >
      <MetricsText size={typeScale.tiny} weight={fontWeights.medium} style={{ height: tooltipRow }}
        color={() => palette().textSecondary}>{props.title}</MetricsText>
      <For each={rows} key={(row) => row.key} style={{ gap: spacing.unit }}>
        {(row) => (
          <Row style={{ height: tooltipRow, gap: spacing.unit * 2, align: "center" }}>
            <Box
              style={{ width: mark, height: mark, shrink: 0 }}
              paint={() => ({ radius: mark / 2, background: row().color ?? metricsColors().track })}
            />
            <MetricsText style={{ grow: 1, minWidth: 0 }} color={() => palette().textSecondary}>{() => row().label}</MetricsText>
            <MetricsText weight={fontWeights.medium}>{() => row().value}</MetricsText>
          </Row>
        )}
      </For>
    </Column>
  );
}

// Lazy memo: callers still subscribe to whatever `deps` reads, and `compute` (which must read
// nothing beyond `deps`) reruns only when one of them changes. Unlike an effect it never runs ahead
// of its readers, so a data change re-evaluates each bar once.
function metricsMemo<T>(deps: () => unknown[], compute: () => T): () => T {
  let last: unknown[] | undefined;
  let value!: T;
  return () => {
    const next = deps();
    if (!last || next.some((dep, i) => dep !== last![i])) {
      last = next;
      value = compute();
    }
    return value;
  };
}

// Keep one active point across the visual gutters between cells or segments.
function metricsHover(data: () => MetricsDashboard) {
  const hovered = signal(-1);
  const shown = signal(0);
  const pointer = signal<MetricsPointer | null>(null);
  // ECharts' hover layer: `any` flips when the pointer enters or leaves the chart, and one flag per
  // mark flips when the pointed mark changes, so moving between marks repaints two of them.
  const any = new LocalSignal(false);
  const marks: LocalSignal<boolean>[] = [];
  let lit = -1;
  effect(() => {
    const index = hovered.value;
    any.value = index >= 0;
    if (index === lit) return;
    if (marks[lit]) marks[lit].value = false;
    lit = index;
    if (marks[index]) marks[index].value = true;
  });
  let chartId: number | undefined;
  let leaveToken = 0;
  const clear = () => {
    ++leaveToken;
    hovered.value = -1;
    pointer.value = null;
    if (metricsPointerTarget && metricsPointerTarget.id === chartId) {
      metricsPointerTarget = undefined;
      requestHost({ type: "metricsPointer", id: 0 });
    }
  };
  effect(() => { data(); clear(); });
  onCleanup(clear);
  return {
    hovered,
    shown,
    pointer,
    any,
    /** Whether mark `index` is the pointed one. */
    active(index: number): boolean {
      return (marks[index] ??= new LocalSignal(index === lit)).value;
    },
    ref(id: number) { chartId = id; },
    enter(index: number) {
      if (chartId === undefined || index < 0) return;
      ++leaveToken;
      if (metricsPointerTarget?.id !== chartId) {
        const previous = metricsPointerTarget;
        metricsPointerTarget = { id: chartId, position: pointer, clear };
        previous?.clear();
        requestHost({ type: "metricsPointer", id: chartId });
      }
      hovered.value = index;
      shown.value = index;
    },
    leave(index: number) {
      // Moving between points can report the new enter before the old leave.
      if (hovered.value !== index) return;
      const token = ++leaveToken;
      afterView(metricsLayout.tooltipLeaveDelay, () => {
        if (token === leaveToken) clear();
      });
    },
  };
}

function MetricsLegendToggle(props: {
  color: () => number;
  on: Signal<boolean>;
  other: Signal<boolean>;
  children: string;
}): Element {
  return (
    <Row
      style={{ height: metricsLayout.toolSize, gap: spacing.unit * 1.5, align: "center", padding: [0, spacing.unit * 2, 0, spacing.unit * 2] }}
      // ECharts legend: a click hides the series; the last visible one stays.
      onPress={() => { if (!props.on.value || props.other.value) props.on.value = !props.on.value; }}
      paint={() => ({
        radius: radii.md,
        hover: { background: rowHoverBackground() },
        transitions: [smoothTransition("background", motion.fast)],
      })}
    >
      <Box
        style={{ width: metricsLayout.mark, height: metricsLayout.mark, shrink: 0 }}
        paint={() => ({
          radius: radii.xs,
          background: props.on.value ? props.color() : metricsColors().track,
          transitions: [smoothTransition("background", motion.fast)],
        })}
      />
      <MetricsText size={typeScale.tiny} color={() => props.on.value ? palette().textSecondary : palette().textMuted}>
        {props.children}
      </MetricsText>
    </Row>
  );
}

function MetricsLegend(props: { color: Source<number>; children: Source<string> }): Element {
  return (
    <Row style={{ gap: spacing.unit * 1.5, align: "center", shrink: 0 }}>
      <Box
        style={{ width: metricsLayout.mark, height: metricsLayout.mark, shrink: 0 }}
        paint={() => ({ background: read(props.color), radius: radii.xs })}
      />
      <MetricsText size={typeScale.tiny} color={() => metricsColors().axis}>{props.children}</MetricsText>
    </Row>
  );
}

function MetricsAxisText(props: { children: Source<string>; style?: Style; align?: TextAlign; color?: Source<number> }): Element {
  return (
    <Text
      lines={1}
      style={props.style}
      align={props.align}
      font={{ family: fontFamilies.sans, size: typeScale.tiny }}
      paint={() => ({ color: props.color === undefined ? metricsColors().axis : read(props.color) })}
    >
      {props.children}
    </Text>
  );
}

function MetricsPanel(props: { title: string; summary: Source<string>; style?: () => Style; ref?: (id: number) => void; children: Children }): Element {
  return (
    <Column
      ref={props.ref}
      style={() => ({ minWidth: 0, shrink: 0, ...props.style?.() })}
      paint={metricsCardPaint}
    >
      <Row
        style={{
          height: spacing.toolbar, padding: [0, metricsLayout.cardPadding, 0, metricsLayout.cardPadding],
          gap: metricsLayout.gap, align: "center", justify: "between", shrink: 0,
        }}
        paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [0, 0, 1, 0] })}
      >
        <MetricsText size={typeScale.label} weight={fontWeights.medium} style={{ shrink: 0 }}>{props.title}</MetricsText>
        <MetricsText size={typeScale.tiny} color={() => palette().textMuted}>{props.summary}</MetricsText>
      </Row>
      {props.children}
    </Column>
  );
}

// Half-width beside a sibling when both fit, full width otherwise.
function metricsHalfStyle(): Style {
  return metricsPanelsFit() ? { grow: 1, basis: 0 } : {};
}

// ---- Header ----------------------------------------------------------------------------------

function MetricsHeader(props: { data: () => MetricsDashboard }): Element {
  const filterLabel = () =>
    configState.value.clis.find((cli) => cli.id === metricsAgent.value)?.name ?? "";
  const subtitle = () => {
    const status = metricsScanState.value;
    if (!metricsUsageState.value) return status === "error" ? "Couldn’t read local usage" : "Reading local usage…";
    if (status === "loading") return "Refreshing local usage…";
    if (status === "error") return "Refresh failed · showing last scan";
    const { start, today } = props.data();
    return `${metricDateRange(start, today)} · Local history · UTC`;
  };
  return (
    <Box
      style={() => ({
        direction: metricsCardsFit(2) ? "row" : "column",
        align: metricsCardsFit(2) ? "center" : "start",
        gap: metricsLayout.gap,
        shrink: 0,
      })}
    >
      <Column style={{ grow: 1, minWidth: 0, gap: spacing.unit }}>
        <Row style={{ align: "center", gap: metricsLayout.gap }}>
          <MetricsText size={typeScale.heading} weight={fontWeights.medium}>Agent usage</MetricsText>
          <Show when={() => !!filterLabel()} style={{ minWidth: 0 }}>
            {() => (
              <HintTooltip label="Clear agent filter" side="bottom" style={{ minWidth: 0 }}>
                <Row
                  style={{
                    height: metricsLayout.toolSize, align: "center", gap: spacing.unit, minWidth: 0,
                    padding: [0, spacing.unit * 2, 0, spacing.unit * 2],
                  }}
                  onPress={() => { metricsAgent.value = ""; }}
                  paint={() => ({
                    radius: radii.md,
                    background: accent().brandSoft,
                    hover: { background: rowHoverBackground() },
                    transitions: [smoothTransition("background", motion.fast)],
                  })}
                >
                  <Image source={() => "cli:" + metricsAgent.value}
                    style={{ width: metricsLayout.iconSize, height: metricsLayout.iconSize, shrink: 0 }} />
                  <MetricsText style={{ minWidth: 0 }} color={() => accent().brandText}>{filterLabel}</MetricsText>
                  <Glyph name="close" size={metricsLayout.iconSize} color={() => palette().textMuted} />
                </Row>
              </HintTooltip>
            )}
          </Show>
        </Row>
        <MetricsText color={() => palette().textMuted}>{subtitle}</MetricsText>
      </Column>
      <Row style={{ gap: spacing.unit, align: "center", shrink: 0 }}>
        <SegmentedControl value={metricsPeriod}
          choices={metricsPeriods.map((period, index) => ({ id: index, label: period.label }))} />
        <IconButton size="sm"
          icon="refresh"
          label="Refresh local usage"
          disabled={() => metricsScanState.value === "loading"}
          onPress={() => scanMetrics(true)}
        />
      </Row>
    </Box>
  );
}

// ---- KPI cards -------------------------------------------------------------------------------

type MetricCardField = "input" | "output" | "sessions" | "duration";

function metricsCardStyle(): Style {
  const { cardHeight, cardVerticalPadding, cardPadding } = metricsLayout;
  return {
    grow: metricsCardsFit(2) ? 1 : 0,
    basis: metricsCardsFit(2) ? 0 : undefined,
    minWidth: 0, height: cardHeight, shrink: 0,
    padding: [cardVerticalPadding, cardPadding, cardVerticalPadding, cardPadding],
    gap: spacing.unit,
  };
}

function metricsCardGridStyle(): Style {
  return { direction: metricsCardsFit(4) ? "row" : "column", gap: metricsLayout.gap, shrink: 0 };
}

function metricsCardPairStyle(): Style {
  return {
    direction: metricsCardsFit(2) ? "row" : "column",
    grow: metricsCardsFit(4) ? 1 : 0,
    basis: metricsCardsFit(4) ? 0 : undefined,
    minWidth: 0, gap: metricsLayout.gap,
  };
}

function MetricCard(props: {
  label: string;
  field: MetricCardField;
  data: () => MetricsDashboard;
  available: (totals: MetricTotals) => boolean;
  format: (value: number) => string;
  detail: (data: MetricsDashboard) => string;
}): Element {
  const values = () => props.data().totals;
  const delta = () => props.available(values()) && props.available(props.data().previous)
    ? metricDelta(values()[props.field], props.data().previous[props.field]) : "";
  const peak = metricsMemo(() => [props.data().buckets],
    () => Math.max(1, ...props.data().buckets.map((bucket) => bucket[props.field])));
  const color = () => metricsColors()[props.field];
  return (
    <Column style={metricsCardStyle} paint={metricsCardPaint}>
      <Row style={{ align: "center", justify: "between", gap: spacing.unit, height: metricsLayout.toolSize }}>
        <MetricsText size={typeScale.label} color={() => palette().textSecondary}>
          {props.label}
        </MetricsText>
        <Show when={() => !!delta()} style={{ shrink: 0 }}>
          {() => (
            <HintTooltip label="Compared with the previous period" side="top">
              <Box
                style={{ padding: [spacing.unit / 2, spacing.unit * 1.5, spacing.unit / 2, spacing.unit * 1.5] }}
                paint={() => ({ radius: radii.chip, background: palette().kbdBg })}
              >
                <MetricsText size={typeScale.tiny} color={() => palette().textSecondary}>{delta}</MetricsText>
              </Box>
            </HintTooltip>
          )}
        </Show>
      </Row>
      <MetricsText size={typeScale.display} weight={fontWeights.medium}>
        {() => props.available(values()) ? props.format(values()[props.field]) : metricNoValue}
      </MetricsText>
      <MetricsText color={() => palette().textMuted}>{() => props.detail(props.data())}</MetricsText>
      <Box style={{ grow: 1 }} />
      <Column style={{ height: metricsLayout.sparkHeight, justify: "end", shrink: 0 }}>
        <For
          each={() => props.data().buckets}
          key={(bucket) => bucket.start}
          style={{
            direction: "row", height: metricsLayout.sparkHeight, align: "end",
            gap: spacing.unit / 2, minWidth: 0,
          }}
        >
          {(bucket) => (
            <Box
              style={() => ({
                grow: 1, basis: 0, minWidth: 0,
                height: Math.round(metricsLayout.sparkHeight * bucket()[props.field] / peak()),
              })}
              paint={() => ({
                background: color(),
                radius: [radii.xs, radii.xs, 0, 0],
                transitions: [smoothTransition("height", motion.panel)],
              })}
            />
          )}
        </For>
        <Box style={{ height: spacing.hairline, shrink: 0 }} paint={() => ({ background: metricsColors().grid })} />
      </Column>
    </Column>
  );
}

function MetricsCards(props: { data: () => MetricsDashboard }): Element {
  const tokensReported = (totals: MetricTotals) => totals.calls > 0;
  return (
    <Box style={metricsCardGridStyle}>
      <Box style={metricsCardPairStyle}>
        <MetricCard
          label="Input tokens" field="input" data={props.data} format={metricCompact} available={tokensReported}
          detail={({ totals }) => totals.input > 0
            ? `${Number((100 * totals.cached / totals.input).toFixed(1))}% from cache` : "Cached input included"}
        />
        <MetricCard
          label="Output tokens" field="output" data={props.data} format={metricCompact} available={tokensReported}
          detail={({ totals }) => totals.calls > 0 ? `${metricCompact(totals.output / totals.calls)} per model call` : "Model responses"}
        />
      </Box>
      <Box style={metricsCardPairStyle}>
        <MetricCard
          label="Sessions" field="sessions" data={props.data} format={metricCount}
          available={() => !!metricsUsageState.value &&
            (!metricsAgent.value || props.data().agentTotals.has(metricsAgent.value))}
          detail={({ activeDays }) => `${metricCount(activeDays)} active ${activeDays === 1 ? "day" : "days"}`}
        />
        <MetricCard
          label="Agent time" field="duration" data={props.data} format={metricDuration}
          available={(totals) => totals.timedTurns > 0}
          detail={({ totals }) => totals.timedTurns > 0
            ? `${metricCount(totals.timedTurns)} timed ${totals.timedTurns === 1 ? "turn" : "turns"}` : "Reported turn duration"}
        />
      </Box>
    </Box>
  );
}

// ---- Token volume ----------------------------------------------------------------------------

type VolumeField = "input" | "output";

const metricsChartBodyStyle: Style = { padding: metricsLayout.cardPadding, gap: spacing.unit * 3 };
const metricsVolumePlotStyle: Style = { grow: 1, minWidth: 0, gap: spacing.unit * 2 };
const metricsVolumeAxisStyle: Style = { height: metricsLayout.axisLabelHeight + spacing.unit };

function MetricsVolume(props: { data: () => MetricsDashboard }): Element {
  const layout = metricsLayout;
  const hover = metricsHover(props.data);
  const visible: Record<VolumeField, Signal<boolean>> = { input: signal(true), output: signal(true) };
  const plotHeight = layout.plotInputHeight + layout.plotOutputHeight;
  const height = (field: VolumeField) => {
    if (!visible[field].value) return 0;
    const shared = visible[field === "input" ? "output" : "input"].value;
    return !shared ? plotHeight : field === "input" ? layout.plotInputHeight : layout.plotOutputHeight;
  };
  const buckets = () => props.data().buckets;
  // Each bar reads the scales; compute them once per data or legend change instead of per bar.
  const scale = (field: VolumeField) => metricsMemo(() => [buckets(), height(field)], () => metricNiceScale(
    Math.max(0, ...buckets().map((bucket) => bucket[field])),
    Math.floor(height(field) / layout.gridSpacing),
  ));
  const scales = { input: scale("input"), output: scale("output") };
  const ticks = (field: VolumeField) => scales[field]();
  const axisMax = (field: VolumeField) => { const all = ticks(field); return all[all.length - 1]; };
  // Input rises from the baseline, output hangs below it.
  const baseline = () => height("input");
  const yOf = (field: VolumeField, value: number) => field === "input"
    ? baseline() - height("input") * value / axisMax("input")
    : baseline() + height("output") * value / axisMax("output");
  const grid = metricsMemo(() => [ticks("input"), ticks("output"), visible.input.value, visible.output.value],
    () => (["input", "output"] as const).flatMap((field) => height(field) === 0 ? [] :
      ticks(field)
        // One baseline when both series share it.
        .filter((tick) => tick > 0 || field === "input" || !visible.input.value)
        .map((tick) => ({ key: `${field}:${tick}`, y: yOf(field, tick), label: metricCompact(tick), base: tick === 0 }))));
  const plotWidth = () => Math.max(1, metricsWidth() - 2 * layout.cardPadding - layout.plotScaleWidth - layout.gap);
  const slot = () => plotWidth() / Math.max(1, buckets().length);
  const barInset = () => Math.max(spacing.hairline, slot() * layout.barGapFraction);
  const xLabels = () => {
    const count = buckets().length;
    const wanted = Math.max(2, Math.min(layout.axisLabels, Math.floor(plotWidth() / (layout.axisLabelWidth + spacing.unit * 4))));
    const indexes = new Set<number>();
    for (let i = 0; i < wanted; i++) indexes.add(Math.round((i * (count - 1)) / (wanted - 1)));
    return [...indexes].filter((index) => index >= 0 && index < count);
  };
  const labelX = (index: number) => Math.max(0, Math.min(
    (index + 0.5) * slot() - layout.axisLabelWidth / 2, plotWidth() - layout.axisLabelWidth));
  const shownBucket = () => buckets()[Math.min(hover.shown.value, buckets().length - 1)];
  const bucketLabel = (bucket: MetricBucket) => metricDateRange(bucket.start, bucket.end);
  const tipRows = (): MetricsTipRow[] => {
    const bucket = shownBucket();
    if (!bucket) return [];
    const colors = metricsColors();
    return [
      { key: "input", label: "Input", value: metricCompact(bucket.input), color: colors.input },
      { key: "cached", label: "From cache", value: bucket.input ? `${Math.round((100 * bucket.cached) / bucket.input)}%` : metricNoValue },
      { key: "output", label: "Output", value: metricCompact(bucket.output), color: colors.output },
      { key: "calls", label: "Model calls", value: metricCount(bucket.calls) },
      { key: "time", label: "Agent time", value: bucket.timedTurns ? metricDuration(bucket.duration) : metricNoValue },
    ];
  };
  const summary = () => {
    const { activeDays, step } = props.data();
    return `${step > 1 ? `${step}-day buckets` : "Daily"} · ${activeDays} active ${activeDays === 1 ? "day" : "days"}`;
  };

  return (
    <MetricsPanel title="Token volume" summary={summary}>
      <Column ref={hover.ref} style={metricsChartBodyStyle}>
        <Row style={{ gap: spacing.unit, align: "center", margin: [0, 0, 0, -spacing.unit * 2] }}>
          <MetricsLegendToggle color={() => metricsColors().input} on={visible.input} other={visible.output}>Input</MetricsLegendToggle>
          <MetricsLegendToggle color={() => metricsColors().output} on={visible.output} other={visible.input}>Output</MetricsLegendToggle>
          <MetricsAxisText style={{ grow: 1, minWidth: 0 }} align="end">Independent scales</MetricsAxisText>
        </Row>
        <Row style={{ gap: layout.gap, align: "start" }}>
          {/* Y axis: one label per gridline. */}
          <For each={grid} key={(line) => line.key}
            style={{ width: layout.plotScaleWidth, height: plotHeight, shrink: 0 }}>
            {(line) => (
              <Box style={() => ({
                position: "absolute", left: 0, width: layout.plotScaleWidth,
                top: Math.round(Math.max(0, Math.min(plotHeight - layout.axisLabelHeight, line().y - layout.axisLabelHeight / 2))),
              })}>
                <MetricsAxisText align="end">{() => line().label}</MetricsAxisText>
              </Box>
            )}
          </For>
          <Column style={metricsVolumePlotStyle}>
            <Box style={{ height: plotHeight }}>
              {/* Axis pointer: a shaded band behind the pointed bucket. */}
              <Box
                style={() => ({ position: "absolute", left: 0, top: 0, width: slot(), height: plotHeight })}
                paint={() => ({
                  x: slot() * hover.shown.value,
                  opacity: hover.hovered.value >= 0 ? 1 : 0,
                  radius: radii.sm,
                  background: withAlpha(palette().textMuted, layout.pointerMix),
                  transitions: [smoothTransition("x", motion.base), smoothTransition("opacity", motion.fast)],
                })}
              />
              <For each={grid} key={(line) => line.key} style={() => ({ position: "absolute", left: 0, top: 0, width: plotWidth(), height: plotHeight })}>
                {(line) => (
                  <Box
                    style={() => ({ position: "absolute", left: 0, top: Math.round(line().y), width: plotWidth(), height: spacing.hairline })}
                    paint={() => ({ background: line().base ? metricsColors().axis : metricsColors().grid, opacity: line().base ? 0.5 : 1 })}
                  />
                )}
              </For>
              <For each={buckets} key={(bucket) => bucket.start}
                style={() => ({ direction: "row", position: "absolute", left: 0, top: 0, width: plotWidth(), height: plotHeight })}>
                {(bucket) => {
                  const index = () => buckets().indexOf(bucket());
                  const dimmed = () => hover.any.value && !hover.active(index());
                  const bar = (field: VolumeField) => {
                    const fade = (color: number) => withAlpha(color, layout.barFade);
                    return (
                      <Box
                        style={() => ({
                          height: Math.round(height(field) * bucket()[field] / axisMax(field)),
                          margin: [0, barInset(), 0, barInset()],
                        })}
                        paint={() => {
                          const color = metricsColors()[field];
                          return {
                            radius: field === "input" ? [radii.xs, radii.xs, 0, 0] : [0, 0, radii.xs, radii.xs],
                            // Strongest at the tip, fading toward the baseline.
                            gradient: field === "input" ? [color, fade(color)] : [fade(color), color],
                            opacity: dimmed() ? layout.blurOpacity : 1,
                            starting: { opacity: 0 },
                            transitions: [smoothTransition("height", motion.panel), smoothTransition("opacity", motion.fast)],
                          };
                        }}
                      />
                    );
                  };
                  return (
                    <Column
                      style={{ grow: 1, basis: 0, minWidth: 0, height: plotHeight }}
                      onHover={(inside) => (inside ? hover.enter(index()) : hover.leave(index()))}
                    >
                      <Column style={() => ({ height: height("input"), justify: "end" })}>{bar("input")}</Column>
                      <Column style={() => ({ height: height("output") })}>{bar("output")}</Column>
                    </Column>
                  );
                }}
              </For>
            </Box>
            {/* X axis, with the pointed bucket's date in a pill that follows the pointer. */}
            <Box style={metricsVolumeAxisStyle}>
              <For each={xLabels} key={(index) => index} style={() => ({ position: "absolute", left: 0, top: 0, width: plotWidth() })}>
                {(index) => (
                  <Box style={() => ({ position: "absolute", top: 0, left: labelX(index()), width: layout.axisLabelWidth })}>
                    <MetricsAxisText align="center">
                      {() => { const bucket = buckets()[index()]; return bucket ? metricDate(bucket.start) : ""; }}
                    </MetricsAxisText>
                  </Box>
                )}
              </For>
              <Box
                style={{ position: "absolute", left: 0, top: -spacing.unit / 2, width: layout.axisLabelWidth, height: layout.axisLabelHeight + spacing.unit, align: "center", justify: "center" }}
                paint={() => ({
                  x: labelX(hover.shown.value),
                  opacity: hover.hovered.value >= 0 ? 1 : 0,
                  radius: radii.sm,
                  background: palette().tooltipBg,
                  transitions: [smoothTransition("x", motion.base), smoothTransition("opacity", motion.fast)],
                })}
              >
                <MetricsAxisText align="center" color={() => palette().tooltipFg}>
                  {() => { const bucket = shownBucket(); return bucket ? metricDate(bucket.start) : ""; }}
                </MetricsAxisText>
              </Box>
            </Box>
          </Column>
        </Row>
        <MetricsTooltip
          hover={hover}
          title={() => { const bucket = shownBucket(); return bucket ? bucketLabel(bucket) : ""; }}
          rows={tipRows}
        />
      </Column>
    </MetricsPanel>
  );
}

// ---- Peak activity ---------------------------------------------------------------------------

function MetricsActivity(props: { data: () => MetricsDashboard }): Element {
  const layout = metricsLayout;
  const hover = metricsHover(props.data);
  const halfGap = layout.heatmapGap / 2;
  const count = (index: number) => props.data().hours[index];
  const total = () => props.data().hours.reduce((sum, value) => sum + value, 0);
  const hourLabel = (hour: number) => String(hour).padStart(2, "0");
  const intensity = (index: number) => Math.ceil(5 * count(index) / Math.max(1, count(props.data().peak)));
  const gridWidth = () => metricsPanelWidth() - 2 * layout.cardPadding;
  const cellWidth = () => (gridWidth() - layout.heatmapLabelWidth - 24 * layout.heatmapGap) / 24;
  const cellX = (hour: number) => layout.heatmapLabelWidth + layout.heatmapGap + hour * (cellWidth() + layout.heatmapGap);
  // Labels repaint when the pointed row or hour changes, not on every cell.
  const hoveredRow = new LocalSignal(-1);
  const hoveredHour = new LocalSignal(-1);
  effect(() => {
    const index = hover.hovered.value;
    hoveredRow.value = index < 0 ? -1 : Math.floor(index / 24);
    hoveredHour.value = index < 0 ? -1 : index % 24;
  });
  const caption = () => {
    const peak = props.data().peak;
    if (count(peak) === 0) return "No model calls";
    return `Peak ${metricWeekdays[Math.floor(peak / 24)]} ${hourLabel(peak % 24)}:00 · ${metricCount(count(peak))} calls`;
  };
  return (
    <MetricsPanel title="Peak activity" summary="Model calls · UTC" style={metricsHalfStyle}>
      <Column ref={hover.ref} style={metricsChartBodyStyle}>
        <Column>
          {metricWeekdays.map((weekday, row) => (
            <Row style={{ align: "center" }}>
              <MetricsAxisText style={{ width: layout.heatmapLabelWidth + layout.heatmapGap, shrink: 0 }}
                color={() => hoveredRow.value === row ? palette().textPrimary : metricsColors().axis}>{weekday}</MetricsAxisText>
              {metricHours.map((hour) => {
                const index = row * 24 + hour;
                return (
                  <Column
                    // Split each gutter across adjacent hit targets, leaving only the inner cells painted.
                    style={{ grow: 1, basis: 0, minWidth: 0, padding: [
                      row === 0 ? 0 : halfGap, hour === metricHours.length - 1 ? 0 : halfGap,
                      row === metricWeekdays.length - 1 ? 0 : halfGap, hour === 0 ? 0 : halfGap,
                    ] }}
                    onHover={(inside) => (inside ? hover.enter(index) : hover.leave(index))}
                  >
                    <Box
                      style={{ height: layout.heatmapCellHeight }}
                      paint={() => ({
                        radius: radii.xs,
                        background: metricsHeatColor(intensity(index)),
                        edge: hover.active(index) ? palette().textPrimary : 0,
                        edgeWidth: spacing.hairline * 2,
                        transitions: [smoothTransition("edge", motion.fast)],
                      })}
                    />
                  </Column>
                );
              })}
            </Row>
          ))}
          <Box style={{ height: layout.axisLabelHeight, margin: [spacing.unit + layout.heatmapGap, 0, 0, 0] }}>
            {[0, 6, 12, 18].map((hour) => (
              <Box style={() => ({ position: "absolute", top: 0, left: cellX(hour) })}>
                <MetricsAxisText
                  color={() => hoveredHour.value === hour ? palette().textPrimary : metricsColors().axis}>
                  {hourLabel(hour)}
                </MetricsAxisText>
              </Box>
            ))}
          </Box>
        </Column>
        <Row style={{ align: "center", justify: "between", gap: spacing.unit }}>
          <MetricsAxisText style={{ minWidth: 0 }}>{caption}</MetricsAxisText>
          <Row style={{ gap: spacing.unit / 2, align: "center", shrink: 0 }}>
            <MetricsAxisText>Less</MetricsAxisText>
            {[0, 1, 2, 3, 4, 5].map((level) => (
              <Box
                style={{ width: layout.mark, height: layout.mark }}
                paint={() => ({ radius: radii.xs, background: metricsHeatColor(level) })}
              />
            ))}
            <MetricsAxisText>More</MetricsAxisText>
          </Row>
        </Row>
        <MetricsTooltip
          hover={hover}
          title={() => {
            const index = hover.shown.value;
            const hour = index % 24;
            return `${metricWeekdays[Math.floor(index / 24)]} · ${hourLabel(hour)}:00–${hourLabel((hour + 1) % 24)}:00`;
          }}
          rows={() => {
            const calls = count(hover.shown.value);
            return [
              { key: "calls", label: "Model calls", value: metricCount(calls), color: metricsHeatColor(intensity(hover.shown.value)) },
              { key: "share", label: "Share of period", value: total() ? `${Number((100 * calls / total()).toFixed(1))}%` : metricNoValue },
            ];
          }}
        />
      </Column>
    </MetricsPanel>
  );
}

// ---- Tables ----------------------------------------------------------------------------------

function MetricsTableNumber(props: { children: Source<string>; heading?: boolean; color?: () => number }): Element {
  return (
    <MetricsText
      align="end" style={{ width: metricsLayout.tableNumber, shrink: 0 }}
      color={() => props.heading ? palette().textMuted : props.color?.() ?? palette().textPrimary}
      size={props.heading ? typeScale.tiny : typeScale.caption}
    >
      {props.children}
    </MetricsText>
  );
}

function metricsTableRowStyle(): Style {
  return {
    height: densityMode.value === "compact" ? metricsLayout.tableRowCompact : metricsLayout.tableRow,
    direction: "row", minWidth: 0, gap: spacing.unit * 2, align: "center", shrink: 0,
    padding: [0, metricsLayout.cardPadding, 0, metricsLayout.cardPadding],
  };
}

function metricsTableRowPaint(selected = false, hoverable = true): Paint {
  return {
    edge: palette().border,
    edgeWidth: spacing.hairline,
    edgeSides: [0, 0, 1, 0],
    background: selected ? accent().brandSoft : 0,
    hover: hoverable && !selected ? { background: rowHoverBackground() } : undefined,
    transitions: [smoothTransition("background", motion.fast)],
  };
}

function MetricsShare(props: { share: () => number; color: () => number }): Element {
  const width = metricsLayout.shareBarWidth;
  return (
    <Row style={{ gap: spacing.unit * 2, align: "center", shrink: 0 }}>
      <Box
        style={{ width, height: metricsLayout.inlineBarHeight }}
        paint={() => ({ radius: radii.sm, background: metricsColors().track })}
      >
        <Box
          style={() => ({ width: Math.max(spacing.hairline * 2, Math.round(width * props.share())), height: metricsLayout.inlineBarHeight })}
          paint={() => ({ radius: radii.sm, background: props.color(), transitions: [smoothTransition("width", motion.panel)] })}
        />
      </Box>
      <MetricsText align="end" style={{ width: spacing.unit * 9 }} color={() => palette().textSecondary}>
        {() => props.share() < 0.01 ? "<1%" : `${Math.round(100 * props.share())}%`}
      </MetricsText>
    </Row>
  );
}

function MetricsProviderShare(props: { data: () => MetricsDashboard; hover: ReturnType<typeof metricsHover> }): Element {
  const percent = (share: number) => share < 0.01 ? "<1%" : `${Math.round(100 * share)}%`;
  return (
    <Show when={() => props.data().providers.length > 0}>
      {() => (
        <Column
          style={{
            padding: [metricsLayout.cardVerticalPadding, metricsLayout.cardPadding, metricsLayout.cardVerticalPadding, metricsLayout.cardPadding],
            gap: spacing.unit * 3,
          }}
          paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [0, 0, 1, 0] })}
        >
          <For
            each={() => props.data().providers}
            key={(provider) => provider.id}
            style={{ direction: "row", height: metricsLayout.mark, gap: spacing.hairline * 2, minWidth: 0 }}
          >
            {(provider) => {
              const index = () => props.data().providers.indexOf(provider());
              return (
                <Box
                  style={() => ({ grow: provider().share, basis: 0, minWidth: spacing.unit })}
                  onHover={(inside) => inside ? props.hover.enter(index()) : props.hover.leave(index())}
                  paint={() => ({
                    radius: radii.xs,
                    background: metricsProviderColor(provider().id),
                    brightness: props.hover.active(index()) ? metricsLayout.providerHoverBrightness : 1,
                  })}
                />
              );
            }}
          </For>
          <For
            each={() => props.data().providers}
            key={(provider) => provider.id}
            style={{ direction: "row", gap: metricsLayout.gap * 1.5, align: "center" }}
          >
            {(provider) => (
              <MetricsLegend color={() => metricsProviderColor(provider().id)}>
                {() => `${metricProviderNames[provider().id]} ${percent(provider().share)}`}
              </MetricsLegend>
            )}
          </For>
        </Column>
      )}
    </Show>
  );
}

function MetricsModels(props: { data: () => MetricsDashboard }): Element {
  const hover = metricsHover(props.data);
  const shownProvider = () => props.data().providers[hover.shown.value];
  const expanded = signal(false);
  const visibleModels = metricsLayout.maxVisibleModels;
  const showShare = () => metricsWidth() >= metricsLayout.shareColumnMin;
  const totalInput = () => props.data().ranked.reduce((sum, model) => sum + model.tokens.input, 0);
  return (
    <MetricsPanel title="Models" summary={() => `${props.data().ranked.length} recorded`} ref={hover.ref}>
      <MetricsProviderShare data={props.data} hover={hover} />
      <Box style={metricsTableRowStyle} paint={() => metricsTableRowPaint(false, false)}>
        <MetricsText style={{ grow: 1, minWidth: 0 }} color={() => palette().textMuted} size={typeScale.tiny}>Model</MetricsText>
        <Show when={showShare} style={{ shrink: 0 }}>
          {() => <MetricsText style={{ width: metricsLayout.shareBarWidth + spacing.unit * 11 }} color={() => palette().textMuted} size={typeScale.tiny}>Input share</MetricsText>}
        </Show>
        <MetricsTableNumber heading>Input</MetricsTableNumber>
        <MetricsTableNumber heading>Cached</MetricsTableNumber>
        <MetricsTableNumber heading>Output</MetricsTableNumber>
        <MetricsTableNumber heading>Calls</MetricsTableNumber>
      </Box>
      <For
        each={() => expanded.value ? props.data().ranked : props.data().ranked.slice(0, visibleModels)}
        key={metricModelKey}
      >
        {(model) => (
          <Box style={metricsTableRowStyle} paint={() => metricsTableRowPaint()}>
            <Row style={{ grow: 1, minWidth: 0, gap: spacing.unit * 2, align: "center" }}>
              <Box
                style={{ width: metricsLayout.mark, height: metricsLayout.mark, shrink: 0 }}
                paint={() => ({ radius: radii.xs, background: metricsProviderColor(model().provider) })}
              />
              <MetricsText style={{ minWidth: 0 }}>{() => model().name}</MetricsText>
              <MetricsText style={{ minWidth: 0, shrink: 0 }} size={typeScale.tiny} color={() => palette().textMuted}>
                {() => metricProviderNames[model().provider]}
              </MetricsText>
            </Row>
            <Show when={showShare} style={{ shrink: 0 }}>
              {() => (
                <MetricsShare
                  share={() => totalInput() ? model().tokens.input / totalInput() : 0}
                  color={() => metricsProviderColor(model().provider)}
                />
              )}
            </Show>
            <MetricsTableNumber>{() => metricCompact(model().tokens.input)}</MetricsTableNumber>
            <MetricsTableNumber color={() => palette().textSecondary}>{() => metricCompact(model().tokens.cached)}</MetricsTableNumber>
            <MetricsTableNumber>{() => metricCompact(model().tokens.output)}</MetricsTableNumber>
            <MetricsTableNumber color={() => palette().textSecondary}>{() => metricCompact(model().tokens.calls)}</MetricsTableNumber>
          </Box>
        )}
      </For>
      <Show when={() => !props.data().ranked.length}>
        {() => <EmptyState compact title="No model records in this period." style={{ padding: metricsLayout.cardPadding }} />}
      </Show>
      <Show when={() => props.data().ranked.length > visibleModels}>
        {() => (
          <Row
            style={() => ({ ...metricsTableRowStyle(), gap: spacing.unit * 1.5 })}
            onPress={() => { expanded.value = !expanded.value; }}
            paint={() => ({
              radius: [0, 0, radii.lg, radii.lg],
              hover: { background: rowHoverBackground() },
              transitions: [smoothTransition("background", motion.fast)],
            })}
          >
            <Glyph name="chevrons-up-down" size={metricsLayout.iconSize} color={() => palette().textMuted} />
            <MetricsText color={() => palette().textSecondary}>
              {() => expanded.value ? "Show fewer models" : `Show ${props.data().ranked.length - visibleModels} more models`}
            </MetricsText>
          </Row>
        )}
      </Show>
      <MetricsTooltip hover={hover} preferAbove
        title={() => { const provider = shownProvider(); return provider ? metricProviderNames[provider.id] : ""; }}
        rows={() => {
          const provider = shownProvider();
          return provider ? [{
            key: "input", label: "Input share",
            value: `${Math.round(100 * provider.share)}% · ${metricCompact(provider.input)}`,
            color: metricsProviderColor(provider.id),
          }] : [];
        }}
      />
    </MetricsPanel>
  );
}

function MetricsAgents(props: { data: () => MetricsDashboard }): Element {
  const activeAgents = () => [...props.data().agentTotals.values()].filter((agent) => agent.sessions > 0).length;
  const peakInput = () => Math.max(1, ...[...props.data().agentTotals.values()].map((agent) => agent.input));
  return (
    <MetricsPanel title="Agents" summary={() => `${activeAgents()} active in this period`} style={metricsHalfStyle}>
      <Box style={metricsTableRowStyle} paint={() => metricsTableRowPaint(false, false)}>
        <MetricsText style={{ grow: 1, minWidth: 0 }} color={() => palette().textMuted} size={typeScale.tiny}>Agent</MetricsText>
        <MetricsTableNumber heading>Input</MetricsTableNumber>
        <MetricsTableNumber heading>Output</MetricsTableNumber>
        <MetricsTableNumber heading>Time</MetricsTableNumber>
        <Box style={{ width: metricsLayout.toolSize, shrink: 0 }} />
      </Box>
      <For each={() => configState.value.clis} key={(cli) => cli.id}>
        {(cli) => {
          const selected = () => metricsAgent.value === cli().id;
          const totals = () => props.data().agentTotals.get(cli().id);
          const input = () => totals()?.input ?? 0;
          const tokens = (field: "input" | "output") => {
            const row = totals();
            return row?.calls ? metricCompact(row[field]) : metricNoValue;
          };
          const time = () => {
            const row = totals();
            return row?.timedTurns ? metricDuration(row.duration) : metricNoValue;
          };
          const tracked = () => !!totals()?.calls || !!totals()?.sessions;
          return (
            <Box
                style={metricsTableRowStyle}
                // The whole row filters the dashboard; the launch button keeps its own press.
                onPress={() => { metricsAgent.value = selected() ? "" : cli().id; }}
                paint={() => metricsTableRowPaint(selected())}
              >
                <Row style={{ grow: 1, minWidth: 0, gap: spacing.unit * 2, align: "center" }}>
                  <Image source={() => "cli:" + cli().id}
                    style={{ width: spacing.unit * 4, height: spacing.unit * 4, shrink: 0 }}
                    paint={() => ({ opacity: tracked() ? 1 : controlState.disabledOpacity })} />
                  <MetricsText style={{ grow: 1, minWidth: 0 }}
                    color={() => selected() ? accent().brandText : tracked() ? palette().textPrimary : palette().textMuted}>
                    {() => cli().name}
                  </MetricsText>
                  <Show when={() => metricsPanelWidth() >= metricsLayout.inlineBarPanelMin} style={{ shrink: 0 }}>
                    {() => (
                      <Box
                        style={{ width: metricsLayout.inlineBarWidth, height: metricsLayout.inlineBarHeight }}
                        paint={() => ({ radius: radii.sm, background: input() ? metricsColors().track : 0 })}
                      >
                        <Box
                          style={() => ({
                            width: Math.round(metricsLayout.inlineBarWidth * input() / peakInput()),
                            height: metricsLayout.inlineBarHeight,
                          })}
                          paint={() => ({
                            radius: radii.sm,
                            background: metricsColors().input,
                            transitions: [smoothTransition("width", motion.panel)],
                          })}
                        />
                      </Box>
                    )}
                  </Show>
                </Row>
                <MetricsTableNumber>{() => tokens("input")}</MetricsTableNumber>
                <MetricsTableNumber>{() => tokens("output")}</MetricsTableNumber>
                <MetricsTableNumber>{time}</MetricsTableNumber>
                <IconButton size="sm"
                  icon="square-terminal" label={() => `Launch ${cli().name}`}
                  disabled={() => !cli().enabled} onPress={() => launchCli(cli().id)}
                />
              </Box>
          );
        }}
      </For>
    </MetricsPanel>
  );
}

// ---- Loading and empty states ----------------------------------------------------------------

function MetricsSkeletonBlock(props: { width?: number; height: number; grow?: boolean }): Element {
  return (
    <Box
      style={{ width: props.width, height: props.height, grow: props.grow ? 1 : 0, basis: props.grow ? 0 : undefined, minWidth: 0, shrink: 0 }}
      paint={() => ({ radius: radii.sm, background: metricsColors().track })}
    />
  );
}

function metricsSkeletonBars(height: number, growsDown = false): Element {
  return (
    <Row style={{ height, align: growsDown ? "start" : "end", gap: metricsLayout.gap, minWidth: 0 }}>
      {metricsSkeletonLayout.bars.map((fraction) => (
        <MetricsSkeletonBlock grow height={Math.round(height * fraction)} />
      ))}
    </Row>
  );
}

function MetricsSkeletonPanel(props: { title: string; style?: () => Style; children: Children }): Element {
  return (
    <MetricsPanel title={props.title} summary="" style={props.style}>
      <Column style={metricsChartBodyStyle}>{props.children}</Column>
    </MetricsPanel>
  );
}

function MetricsSkeletonTable(props: { agents?: boolean }): Element {
  const layout = metricsLayout;
  const count = () => 1 + (props.agents ? configState.value.clis.length : metricsSkeletonLayout.modelRows);
  return (
    <For each={() => Array.from({ length: count() }, (_, index) => index)} key={(index) => index}>
      {(index) => (
        <Box style={metricsTableRowStyle} paint={() => metricsTableRowPaint(false, false)}>
          <Row style={{ grow: 1, minWidth: 0, align: "center", gap: spacing.unit * 2 }}>
            {props.agents && index() > 0 ? (
              <MetricsSkeletonBlock width={spacing.unit * 4} height={spacing.unit * 4} />
            ) : null}
            <MetricsSkeletonBlock width={spacing.unit * 12} height={spacing.unit * 2} />
          </Row>
          {Array.from({ length: props.agents ? 3 : 4 }, () => (
            <Row style={{ width: layout.tableNumber, shrink: 0, justify: "end" }}>
              <MetricsSkeletonBlock width={layout.tableNumber / 2} height={spacing.unit * 2} />
            </Row>
          ))}
          {props.agents ? <Box style={{ width: layout.toolSize, shrink: 0 }} /> : null}
        </Box>
      )}
    </For>
  );
}

function MetricsSkeleton(): Element {
  const layout = metricsLayout;
  const card = () => (
    <Column style={metricsCardStyle} paint={metricsCardPaint}>
      <Row style={{ height: layout.toolSize, align: "center" }}>
        <MetricsSkeletonBlock width={spacing.unit * 20} height={spacing.unit * 3} />
      </Row>
      <MetricsSkeletonBlock width={spacing.unit * 24} height={spacing.unit * 7} />
      <MetricsSkeletonBlock width={spacing.unit * 28} height={spacing.unit * 2.5} />
      <Box style={{ grow: 1 }} />
      {metricsSkeletonBars(layout.sparkHeight)}
    </Column>
  );
  return (
    <Column style={{ gap: layout.gap, shrink: 0 }}>
      <Box style={metricsCardGridStyle}>
        <Box style={metricsCardPairStyle}>{card()}{card()}</Box>
        <Box style={metricsCardPairStyle}>{card()}{card()}</Box>
      </Box>
      <MetricsSkeletonPanel title="Token volume">
        <Row style={{ height: layout.toolSize, align: "center", gap: spacing.unit * 3 }}>
          <MetricsSkeletonBlock width={spacing.unit * 12} height={spacing.unit * 3} />
          <MetricsSkeletonBlock width={spacing.unit * 12} height={spacing.unit * 3} />
        </Row>
        <Row style={{ gap: layout.gap }}>
          <Box style={{ width: layout.plotScaleWidth, shrink: 0 }} />
          <Column style={metricsVolumePlotStyle}>
            <Column>
              {metricsSkeletonBars(layout.plotInputHeight)}
              {metricsSkeletonBars(layout.plotOutputHeight, true)}
            </Column>
            <Box style={metricsVolumeAxisStyle} />
          </Column>
        </Row>
      </MetricsSkeletonPanel>
      <Box style={() => ({ direction: metricsPanelsFit() ? "row" : "column", gap: layout.gap })}>
        <MetricsSkeletonPanel title="Peak activity" style={metricsHalfStyle}>
          <Column style={{ gap: layout.heatmapGap }}>
            {metricWeekdays.map((weekday) => (
              <Row style={{ gap: layout.heatmapGap, align: "center" }}>
                <MetricsAxisText style={{ width: layout.heatmapLabelWidth, shrink: 0 }}>{weekday}</MetricsAxisText>
                <MetricsSkeletonBlock grow height={layout.heatmapCellHeight} />
              </Row>
            ))}
            <Box style={{ height: layout.axisLabelHeight + spacing.unit, shrink: 0 }} />
          </Column>
          <MetricsSkeletonBlock width={layout.heatmapLabelWidth * 2} height={layout.axisLabelHeight} />
        </MetricsSkeletonPanel>
        <MetricsPanel title="Agents" summary="" style={metricsHalfStyle}>
          <MetricsSkeletonTable agents />
        </MetricsPanel>
      </Box>
      <MetricsPanel title="Models" summary="">
        <MetricsSkeletonTable />
      </MetricsPanel>
    </Column>
  );
}

function MetricsError(): Element {
  return (
    <Column
      style={{ padding: metricsLayout.cardPadding * 2, gap: spacing.unit * 3, align: "center", shrink: 0 }}
      paint={metricsCardPaint}
    >
      <Glyph name="dashboard" size={metricsLayout.errorIcon} color={() => palette().textMuted} />
      <MetricsText size={typeScale.body} weight={fontWeights.medium}>Couldn’t read local usage</MetricsText>
      <MetricsText color={() => palette().textMuted}>The agents’ history files could not be scanned.</MetricsText>
      <GhostButton icon="refresh" label="Try again" onPress={() => scanMetrics(true)} />
    </Column>
  );
}

function MetricsBody(props: { data: () => MetricsDashboard }): Element {
  const layout = metricsLayout;
  return (
    <Column
      style={{ gap: layout.gap, shrink: 0 }}
      // A refresh keeps the last scan on screen, dimmed until the new one lands.
      paint={() => ({
        opacity: metricsScanState.value === "loading" ? layout.refreshingOpacity : 1,
        transitions: [smoothTransition("opacity", motion.enter)],
      })}
    >
      <MetricsCards data={props.data} />
      <MetricsVolume data={props.data} />
      <Box style={() => ({ direction: metricsPanelsFit() ? "row" : "column", gap: layout.gap, shrink: 0 })}>
        <MetricsActivity data={props.data} />
        <MetricsAgents data={props.data} />
      </Box>
      <MetricsModels data={props.data} />
    </Column>
  );
}

function MetricsView(): Element {
  const data = metricsDashboardSource();
  if (metricsScanState.value === "idle") scanMetrics(false);
  const showSkeleton = signal(false);
  if (metricsUsageState.value === null) {
    afterView(metricsSkeletonLayout.delay, () => { showSkeleton.value = true; });
  }
  const viewWidth = signal(0);
  const fadeHeight = signal(0);
  let root: number | undefined;
  const measure = () => {
    const width = root === undefined ? 0 : jsnir.columns.width[root] ?? 0;
    if (width > 0) {
      viewWidth.value = width;
      metricsViewWidth.value = width;
    }
  };
  // The window resizes with the tiling (columns, splits, fullscreen) and the sidebar.
  effect(() => {
    void viewportSize.value;
    void sidebarCompact.value;
    void tilingState.value;
    afterView(16, measure);
    afterView(SPRING_MS + 40, measure);
  });
  const state = () => {
    if (viewWidth.value === 0) return "waiting";
    if (metricsUsageState.value !== null) return "ready";
    if (metricsScanState.value === "error") return "error";
    return showSkeleton.value ? "skeleton" : "waiting";
  };
  const column = () => ({ width: metricsWidth(), shrink: 0 });
  return (
    <Column
      ref={(id) => { root = id; afterView(16, measure); }}
      style={{ grow: 1, minWidth: 0, minHeight: 0, align: "stretch" }}
    >
      <Row style={{ justify: "center", shrink: 0 }}>
        <Box style={() => ({ ...column(), padding: [metricsLayout.padding, 0, metricsLayout.sectionGap, 0] })}>
          <MetricsHeader data={data} />
        </Box>
      </Row>
      <Column style={{ grow: 1, minWidth: 0, minHeight: 0, clip: true }}>
        <Column
          style={{ grow: 1, minWidth: 0, minHeight: 0, scroll: true, align: "center" }}
          onScroll={({ top }) => { fadeHeight.value = Math.min(top, metricsLayout.scrollFadeHeight); }}
        >
          <Column style={() => ({ ...column(), padding: [0, 0, metricsLayout.padding, 0] })}>
            <Dynamic value={state}>
              {(current) => current === "ready" ? <MetricsBody data={data} />
                : current === "error" ? <MetricsError />
                : current === "skeleton" ? <MetricsSkeleton /> : <Box />}
            </Dynamic>
          </Column>
        </Column>
        <Box
          style={() => ({ position: "absolute", left: 0, top: 0, width: viewWidth.value, height: fadeHeight.value })}
          paint={() => {
            const background = palette().appBg;
            return { gradient: [background, withAlpha(background, 0)] };
          }}
        />
      </Column>
    </Column>
  );
}
