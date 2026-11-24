// Browser pane: an address row over the page. CEF renders the page into the image `browser:<id>`
// (crates/icy-app/src/browser.rs), so this view only sends host requests and draws that frame.

function BrowserView(props: { id: number; spec: TileSpec }): Element {
  const st = () => browserStates.value[props.id];
  const hovered = signal(false);
  // The page is sized to the body node; the host gets its logical size (see reportSize).
  let bodyId: number | undefined;
  let opened = false;
  let initialUrl = props.spec.url ?? "about:blank";
  let focusAddress = initialUrl === "about:blank" && !st();
  let sentSize = "";
  let settleAt = 0;
  let settling = false;
  const reportSize = (): void => {
    if (bodyId === undefined) return;
    const width = Math.round(jsnir.columns.width[bodyId] ?? 0);
    const height = Math.round(jsnir.columns.height[bodyId] ?? 0);
    const key = `${width}x${height}`;
    if (width <= 0 || height <= 0 || key === sentSize) return;
    const sent = requestHost(opened
      ? { type: "browser", op: "resize", id: props.id, width, height }
      : { type: "browser", op: "open", id: props.id, url: initialUrl, width, height, node: bodyId });
    if (!sent) return;
    if (!opened) {
      opened = true;
      // Focus given before the page existed is not kept by the host: send it again.
      if (activeNav.value === "agents" && tilingState.value.focusedSession === props.id) {
        requestHost({ type: "browser", op: "focus", id: props.id });
      }
    }
    sentSize = key;
  };
  const settle = (): void => {
    const remaining = settleAt - Date.now();
    if (remaining > 0) { afterView(remaining, settle); return; }
    settling = false;
    reportSize();
  };
  // Panes move under a spring; report the settled size once it stops.
  effect(() => {
    void tilingState.value;
    void viewportSize.value;
    void noticeState.value;
    void sidebarCompact.value;
    settleAt = Date.now() + SPRING_MS + browserLayout.settleDelay;
    if (!settling) {
      settling = true;
      afterView(SPRING_MS + browserLayout.settleDelay, settle);
    }
  });
  // Wheel and clicks reach the page only while the pointer is over its body.
  const pointer = (inside: boolean): void => {
    hovered.value = inside;
    if (inside && bodyId !== undefined) requestHost({ type: "browser", op: "pointer", id: props.id, node: bodyId });
    else if (!inside) requestHost({ type: "browser", op: "pointer", id: 0, from: props.id });
  };
  onCleanup(() => {
    if (hovered.value) requestHost({ type: "browser", op: "pointer", id: 0, from: props.id });
    // Unmounting (tab switch, overview, fullscreen of another pane) pauses the page; remount unhides it.
    requestHost({ type: "browser", op: "hidden", id: props.id, hidden: true });
  });
  const addressOf = () => {
    const url = st()?.url || initialUrl;
    return url === "about:blank" ? "" : url;
  };
  const navigate = (text: string) => {
    if (!text.trim()) return;
    if (opened) {
      requestHost({ type: "browser", op: "navigate", id: props.id, text });
      return;
    }
    const url = resolveAddress(text, true);
    if (url) {
      initialUrl = url;
      reportSize();
    }
  };
  return (
    <Column style={{ grow: 1, minWidth: 0, minHeight: 0 }}>
      <Row
        style={{ height: browserLayout.barHeight, padding: browserLayout.barPadding, align: "center",
          gap: browserLayout.barGap, shrink: 0 }}
        paint={() => ({ edge: palette().border, edgeWidth: spacing.hairline, edgeSides: [0, 0, 1, 0] })}
      >
        <IconButton size="sm" side="bottom" icon="arrow-left" label="Back" disabled={() => !st()?.canBack}
          onPress={() => requestHost({ type: "browser", op: "back", id: props.id })} />
        <IconButton size="sm" side="bottom" icon="arrow-right" label="Forward" disabled={() => !st()?.canForward}
          onPress={() => requestHost({ type: "browser", op: "forward", id: props.id })} />
        <IconButton size="sm" side="bottom" icon="refresh" label="Reload"
          onPress={() => requestHost({ type: "browser", op: "reload", id: props.id })} />
        <Row
          style={{ grow: 1, minWidth: 0, height: browserLayout.fieldHeight, padding: browserLayout.fieldPadding, align: "center" }}
          paint={() => ({ radius: browserLayout.fieldRadius, edge: palette().border, edgeWidth: spacing.hairline })}
        >
          {/* Remounted when the page reports another address, so the field shows where the page is. */}
          <Dynamic value={addressOf} style={{ grow: 1, minWidth: 0 }}>
            {(address) => {
              const autoFocus = focusAddress && activeNav.value === "agents" && tilingState.value.focusedSession === props.id;
              focusAddress = false;
              return <TextInput
                value={address}
                autoFocus={autoFocus}
                retainOnSubmit
                style={{ grow: 1, minWidth: 0, height: browserLayout.fieldHeight }}
                font={{ family: fontFamilies.sans, size: typeScale.label }}
                placeholder="Search or enter a URL"
                placeholderColor={palette().textMuted}
                onSubmit={(entry) => navigate(entry.text)}
                paint={() => ({ color: palette().textPrimary })}
              />;
            }}
          </Dynamic>
        </Row>
        <IconButton size="sm" side="bottom" icon="external-link" label="Open in external browser"
          disabled={() => !addressOf()} onPress={() => openExternal(addressOf())} />
      </Row>
      <Box
        ref={(id) => { bodyId = id; afterView(browserLayout.settleDelay, reportSize); }}
        style={{ grow: 1, minHeight: 0, clip: true }}
        onHover={pointer}
      >
        <Dynamic value={() => !!st()?.error} style={{ grow: 1, minHeight: 0 }}>
          {(failed) => failed ? (
            <Column style={{ grow: 1, minHeight: 0, align: "center", justify: "center", gap: spacing.unit * 2 }}>
              <Text font={{ family: fontFamilies.sans, size: typeScale.label }}
                paint={() => ({ color: palette().textMuted })}>{() => st()?.error ?? ""}</Text>
              <Show when={() => !!addressOf()}>
                {() => <GhostButton icon="external-link" label="Open in external browser" onPress={() => openExternal(addressOf())} />}
              </Show>
            </Column>
          ) : (
            <Image source={() => "browser:" + props.id} style={{ grow: 1, minWidth: 0, minHeight: 0 }} />
          )}
        </Dynamic>
      </Box>
    </Column>
  );
}
