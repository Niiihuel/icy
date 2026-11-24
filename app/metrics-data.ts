// Rust supplies daily aggregates in UTC. Period selection and chart binning stay local to the view.

interface MetricTokens {
  input: number;
  output: number;
  cached: number;
  calls: number;
}

interface MetricModel {
  name: string;
  provider: ChartProvider;
  tokens: MetricTokens;
}

interface MetricDay {
  day: number;
  tokens: MetricTokens;
  sessions: number;
  durationMs: number;
  timedTurns: number;
  hours: number[];
  models: MetricModel[];
}

interface MetricAgent {
  id: string;
  periods: { current: number; previous: number }[];
  days: MetricDay[];
}

interface MetricsSnapshot {
  today: number;
  agents: MetricAgent[];
}

interface MetricTotals extends MetricTokens {
  sessions: number;
  duration: number;
  timedTurns: number;
}

interface MetricBucket extends MetricTotals {
  start: number;
  end: number;
}

const metricsUsageState = signal<MetricsSnapshot | null>(null);
const metricsScanState = signal<"idle" | "loading" | "ready" | "error">("idle");
const metricsPeriod = signal(1);
const metricsAgent = signal("");

const metricsPeriods = [
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
  { label: "90 days", days: 90 },
  { label: "All", days: 0 },
];

const metricProviderNames: Record<ChartProvider, string> = {
  anthropic: "Anthropic",
  google: "Google",
  openai: "OpenAI",
  xai: "xAI",
  other: "Other",
};

const metricWeekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const metricMonths = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const metricHours = Array.from({ length: 24 }, (_, hour) => hour);
const metricNoValue = "—";

function metricDate(day: number, year = false): string {
  const date = new Date(day * 86_400_000);
  return metricMonths[date.getUTCMonth()] + " " + date.getUTCDate() + (year ? ", " + date.getUTCFullYear() : "");
}

function metricDateRange(start: number, end: number): string {
  const crossesYear = new Date(start * 86_400_000).getUTCFullYear() !==
    new Date(end * 86_400_000).getUTCFullYear();
  return start === end ? metricDate(end, crossesYear) : `${metricDate(start, crossesYear)} – ${metricDate(end, crossesYear)}`;
}

function metricCount(value: number): string {
  return String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

function metricCompact(value: number): string {
  for (const [size, suffix] of [[1e12, "T"], [1e9, "B"], [1e6, "M"], [1e3, "K"]] as const) {
    if (value >= size) return `${Number((value / size).toFixed(value >= size * 100 ? 0 : 1))}${suffix}`;
  }
  return metricCount(value);
}

function metricDuration(ms: number): string {
  if (ms >= 3_600_000) return `${Number((ms / 3_600_000).toFixed(1))} h`;
  if (ms >= 60_000) return `${Math.round(ms / 60_000)} min`;
  return `${Math.round(ms / 1000)} s`;
}

function metricDelta(value: number, previous: number): string {
  if (previous <= 0) return "";
  const change = Math.round(((value - previous) / previous) * 100);
  return `${change >= 0 ? "+" : "−"}${metricCompact(Math.abs(change))}%`;
}

// ECharts-style "nice" axis: the smallest 1 / 2 / 2.5 / 5 × 10ⁿ step that draws `max` in at most
// `maxTicks` intervals. Returns every tick from 0; the last one is the axis maximum.
function metricNiceScale(max: number, maxTicks: number): number[] {
  if (!(max > 0)) return [0, 1];
  const raw = max / Math.max(1, maxTicks);
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const nice = [1, 2, 2.5, 5, 10].map((factor) => factor * magnitude).find((step) => step >= raw) ?? raw;
  // Counts are whole numbers: never split an axis below one.
  const step = Math.max(1, nice);
  const count = Math.max(1, Math.ceil(max / step - 1e-9));
  return Array.from({ length: count + 1 }, (_, index) => index * step);
}

function emptyMetricTokens(): MetricTokens {
  return { input: 0, output: 0, cached: 0, calls: 0 };
}

function emptyMetricTotals(): MetricTotals {
  return { ...emptyMetricTokens(), sessions: 0, duration: 0, timedTurns: 0 };
}

function addMetricTokens(target: MetricTokens, source: MetricTokens): void {
  target.input += source.input;
  target.output += source.output;
  target.cached += source.cached;
  target.calls += source.calls;
}

function addMetricDay(target: MetricTotals, source: MetricDay): void {
  addMetricTokens(target, source.tokens);
  target.duration += source.durationMs;
  target.timedTurns += source.timedTurns;
}

function buildMetricsDashboard(snapshot: MetricsSnapshot | null, period: number, filter: string) {
  const agents = snapshot?.agents ?? [];
  const selected = filter ? agents.filter((agent) => agent.id === filter) : agents;
  const today = snapshot?.today ?? Math.floor(Date.now() / 86_400_000);
  const length = metricsPeriods[period].days;
  const start = length
    ? today + 1 - length
    : selected.reduce((min, agent) => Math.min(min, agent.days[0]?.day ?? today), today);
  const step = Math.max(1, Math.ceil((today - start + 1) / metricsLayout.plotBars));
  const buckets: MetricBucket[] = [];
  for (let day = start; day <= today; day += step) {
    buckets.push({ ...emptyMetricTotals(), start: day, end: Math.min(today, day + step - 1) });
  }
  const previous = emptyMetricTotals();
  const totals = emptyMetricTotals();
  const daily = new Set<number>();
  const models = new Map<string, MetricModel>();
  const hours = Array<number>(7 * 24).fill(0);
  const agentTotals = new Map<string, MetricTotals>();

  for (const agent of agents) {
    const row = emptyMetricTotals();
    agentTotals.set(agent.id, row);
    const active = !filter || filter === agent.id;
    row.sessions = agent.periods[period].current;
    if (active) {
      totals.sessions += row.sessions;
      previous.sessions += agent.periods[period].previous;
    }

    for (const day of agent.days) {
      if (!length || day.day >= start) addMetricDay(row, day);
      if (day.day < start) {
        if (active && length && day.day >= start - length) addMetricDay(previous, day);
        continue;
      }
      if (!active) continue;

      addMetricDay(totals, day);
      daily.add(day.day);
      const bucket = buckets[Math.floor((day.day - start) / step)];
      addMetricDay(bucket, day);
      bucket.sessions += day.sessions;

      // 1970-01-01 was Thursday. The grid starts on Monday.
      const weekday = (day.day + 3) % 7;
      day.hours.forEach((count, hour) => { hours[weekday * 24 + hour] += count; });
      for (const model of day.models) {
        const key = metricModelKey(model);
        let item = models.get(key);
        if (!item) {
          item = { name: model.name, provider: model.provider, tokens: emptyMetricTokens() };
          models.set(key, item);
        }
        addMetricTokens(item.tokens, model.tokens);
      }
    }
  }

  const ranked = [...models.values()].sort((a, b) =>
    b.tokens.input - a.tokens.input || b.tokens.output - a.tokens.output || a.name.localeCompare(b.name));
  const providerInputs = new Map<ChartProvider, number>();
  let attributedInput = 0;
  for (const model of ranked) {
    const input = model.tokens.input;
    attributedInput += input;
    providerInputs.set(model.provider, (providerInputs.get(model.provider) ?? 0) + input);
  }
  const providers = [...providerInputs].filter(([, input]) => input > 0)
    .sort((a, b) => b[1] - a[1])
    .map(([id, input]) => ({ id, input, share: input / attributedInput }));

  const peak = hours.indexOf(Math.max(...hours));
  return { start, today, step, buckets, previous, totals, activeDays: daily.size, ranked, providers, hours, peak, agentTotals };
}

function metricModelKey(model: MetricModel): string {
  return `${model.provider}/${model.name}`;
}

type MetricsDashboard = ReturnType<typeof buildMetricsDashboard>;

function metricsDashboardSource(): () => MetricsDashboard {
  let snapshot: MetricsSnapshot | null = null;
  let period = -1;
  let agent = "";
  let result: MetricsDashboard;
  return () => {
    const nextSnapshot = metricsUsageState.value;
    const nextPeriod = metricsPeriod.value;
    const nextAgent = metricsAgent.value;
    if (nextSnapshot !== snapshot || nextPeriod !== period || nextAgent !== agent) {
      snapshot = nextSnapshot;
      period = nextPeriod;
      agent = nextAgent;
      result = buildMetricsDashboard(snapshot, period, agent);
    }
    return result;
  };
}
