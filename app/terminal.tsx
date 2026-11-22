// Main terminal viewport with clickable file/URL spans and Accordee-style input composer.

function activeSessionItem(): SessionItem | undefined {
  const s = sessionsState.value;
  return s.items.find((item) => item.id === s.activeId) ?? s.items[0];
}

function visibleLinks(): DetectedLink[] {
  const seen = new Set<string>();
  const collected: DetectedLink[] = [];
  const rows = screenState.value.rows;
  for (let i = rows.length - 1; i >= 0; i--) {
    for (const link of rows[i].links) {
      const key = `${link.kind}:${link.target}:${link.line ?? ""}`;
      if (!seen.has(key)) {
        seen.add(key);
        collected.push(link);
        if (collected.length >= 4) return collected;
      }
    }
  }
  return collected;
}

function TerminalPane(): Element {
  const draft = signal("");

  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      {/* Top toolbar */}
      <Row
        style={{
          height: spacing.toolbar,
          shrink: 0,
          padding: [0, 14, 0, 16],
          align: "center",
          justify: "between",
          gap: 12,
        }}
        paint={() => ({
          edge: palette().border,
          edgeWidth: spacing.hairline,
          edgeSides: [0, 0, 1, 0],
        })}
      >
        <Row style={{ align: "center", gap: 10, grow: 1, minWidth: 0 }}>
          <StatusDot alive={() => activeSessionItem()?.alive ?? false} />
          <Text
            font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
            paint={() => ({ color: palette().textPrimary })}
          >
            {() => activeSessionItem()?.title ?? "shell-1"}
          </Text>
          <Text
            lines={1}
            font={{ family: fontFamilies.mono, size: typeScale.caption, weight: fontWeights.normal }}
            paint={() => ({ color: palette().textMuted })}
          >
            {() => activeSessionItem()?.cwd ?? ""}
          </Text>
        </Row>

        <Row style={{ align: "center", gap: 6, shrink: 0 }}>
          <KbdChip label="Ctrl+C" onPress={() => sendKey("ctrl-c")} />
          <KbdChip label="Esc" onPress={() => sendKey("esc")} />
          <KbdChip label="Tab" onPress={() => sendKey("tab")} />
          <KbdChip label="↑" onPress={() => sendKey("up")} />
          <KbdChip label="↓" onPress={() => sendKey("down")} />
        </Row>
      </Row>

      {/* Optional URL/notice bar */}
      <Show when={() => noticeState.value !== null}>
        {() => (
          <Row
            style={{
              height: 32,
              shrink: 0,
              padding: [0, 14, 0, 16],
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
              font={{ family: fontFamilies.mono, size: typeScale.caption, weight: fontWeights.normal }}
              paint={() => ({ color: accent().brandText })}
            >
              {() =>
                noticeState.value
                  ? `Preview URL (embedded browser in Phase 4): ${noticeState.value.text}`
                  : ""
              }
            </Text>
            <KbdChip label="dismiss" onPress={() => { noticeState.value = null; }} />
          </Row>
        )}
      </Show>

      {/* Terminal VT grid */}
      <For
        each={() => screenState.value.rows}
        key={(row) => row.idx}
        style={{
          grow: 1,
          minHeight: 0,
          scroll: true,
          padding: [12, 16, 12, 16],
          gap: 0,
        }}
      >
        {(row) => {
          const hasLinks = () => row().links.length > 0;
          return (
            <Box
              style={{
                height: 19,
                padding: [0, 4, 0, 4],
                justify: "center",
              }}
              onPress={() => {
                const links = row().links;
                if (links.length > 0) openLink(links[0]);
              }}
              paint={() => ({
                radius: radii.sm,
                hover: hasLinks() ? { background: accent().brandSoft } : undefined,
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <Text
                lines={1}
                font={{ family: fontFamilies.mono, size: 13, lineHeight: 19 }}
                spans={() =>
                  row().spans.map((s) => ({
                    text: s.text,
                    color: s.link ? accent().brandText : s.fg,
                    background: s.bg,
                    weight: s.bold ? 600 : 400,
                    underline: s.underline || s.link,
                  }))
                }
              >
                {() => row().plain}
              </Text>
            </Box>
          );
        }}
      </For>

      {/* Detected links strip */}
      <Show when={() => visibleLinks().length > 0}>
        {() => (
          <Row
            style={{
              height: 32,
              shrink: 0,
              padding: [0, 14, 0, 14],
              align: "center",
              gap: 6,
            }}
            paint={() => ({
              edge: palette().border,
              edgeWidth: spacing.hairline,
              edgeSides: [1, 0, 0, 0],
            })}
          >
            <Text
              font={{ family: fontFamilies.sans, size: typeScale.tiny, weight: fontWeights.medium }}
              paint={() => ({ color: palette().textMuted })}
            >
              Links:
            </Text>
            <For
              each={visibleLinks}
              key={(l) => `${l.kind}:${l.target}:${l.line ?? ""}`}
              style={{ direction: "row", gap: 6, align: "center", grow: 1, minWidth: 0 }}
            >
              {(link) => (
                <KbdChip
                  accented={() => link().resolved || link().kind === "url"}
                  label={() => {
                    const l = link();
                    return l.line ? `${l.target}:${l.line}` : l.target;
                  }}
                  onPress={() => openLink(link())}
                />
              )}
            </For>
          </Row>
        )}
      </Show>

      {/* Accordee-inspired bottom composer */}
      <Box
        style={{
          shrink: 0,
          padding: [10, 14, 12, 14],
        }}
        paint={() => ({
          edge: palette().border,
          edgeWidth: spacing.hairline,
          edgeSides: [1, 0, 0, 0],
        })}
      >
        <Row
          style={{
            padding: [6, 8, 6, 12],
            align: "center",
            gap: 8,
          }}
          paint={() => ({
            radius: radii.panel,
            background: palette().panelBg,
            edge: palette().border,
            edgeWidth: spacing.hairline,
          })}
        >
          <Text
            font={{ family: fontFamilies.mono, size: typeScale.label, weight: fontWeights.normal }}
            paint={() => ({ color: accent().brandText })}
          >
            ❯
          </Text>
          <TextInput
            style={{ grow: 1, minHeight: 28, padding: [4, 6, 4, 2] }}
            font={{ family: fontFamilies.mono, size: typeScale.label }}
            placeholder="Run command or send input to active PTY..."
            placeholderColor={palette().textMuted}
            submitOnEnter={true}
            onChange={(text) => {
              draft.value = text;
            }}
            onSubmit={(submit) => {
              draft.value = "";
              sendInput(submit.text);
            }}
            paint={() => ({ color: palette().textPrimary })}
          />
          <PrimaryButton
            label="Run"
            height={spacing.control}
            onPress={() => {
              const text = draft.value;
              sendInput(text);
            }}
          />
        </Row>
      </Box>
    </Column>
  );
}
