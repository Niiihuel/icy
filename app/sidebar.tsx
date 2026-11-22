// Accordee-style workspace sidebar with session list and theme controls.

function Sidebar(): Element {
  return (
    <Column
      style={{
        width: spacing.sidebarWidth,
        shrink: 0,
        padding: [10, 10, 12, 10],
        gap: 10,
      }}
      paint={() => ({ background: palette().sidebarBg })}
    >
      <Row
        style={{
          height: 32,
          padding: [0, 4, 0, 6],
          align: "center",
          justify: "between",
        }}
      >
        <Row style={{ align: "center", gap: 8 }}>
          <Box
            style={{ width: 8, height: 8, shrink: 0 }}
            paint={() => ({ radius: 4, background: accent().brand })}
          />
          <Text
            font={{ family: fontFamilies.sans, size: typeScale.subtitle, weight: fontWeights.semibold }}
            paint={() => ({ color: palette().textPrimary })}
          >
            icy
          </Text>
        </Row>
        <KbdChip
          label={() => (themeMode.value === "dark" ? "dark" : "light")}
          onPress={() => {
            themeMode.value = themeMode.value === "dark" ? "light" : "dark";
          }}
        />
      </Row>

      <SecondaryButton label="+ New session" onPress={createSession} height={spacing.controlLg} />

      <Row style={{ padding: [4, 6, 0, 6], align: "center", justify: "between" }}>
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.tiny, weight: fontWeights.medium }}
          paint={() => ({ color: palette().textMuted })}
        >
          SESSIONS
        </Text>
        <Text
          font={{ family: fontFamilies.mono, size: typeScale.tiny, weight: fontWeights.normal }}
          paint={() => ({ color: palette().textMuted })}
        >
          {() => String(sessionsState.value.items.length)}
        </Text>
      </Row>

      <For
        each={() => sessionsState.value.items}
        key={(item) => item.id}
        style={{ grow: 1, minHeight: 0, scroll: true, gap: 2 }}
      >
        {(item) => {
          const isActive = () => sessionsState.value.activeId === item().id;
          return (
            <Row
              style={{
                height: spacing.controlLg,
                padding: [0, 10, 0, 10],
                align: "center",
                justify: "between",
                gap: 8,
              }}
              onPress={() => selectSession(item().id)}
              paint={() => ({
                radius: radii.md,
                background: isActive() ? palette().sidebarActive : 0,
                hover: { background: isActive() ? palette().sidebarActive : palette().panelBg },
                transitions: [smoothTransition("background", motion.fast)],
              })}
            >
              <Row style={{ align: "center", gap: 8, grow: 1, minWidth: 0 }}>
                <StatusDot alive={() => item().alive} />
                <Text
                  lines={1}
                  font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
                  paint={() => ({
                    color: isActive() ? palette().textPrimary : palette().textSecondary,
                  })}
                >
                  {() => item().title}
                </Text>
              </Row>
              <Text
                font={{ family: fontFamilies.mono, size: typeScale.micro, weight: fontWeights.normal }}
                paint={() => ({ color: palette().textMuted })}
              >
                {() => (item().alive ? "pty" : "exit")}
              </Text>
            </Row>
          );
        }}
      </For>

      <Column
        style={{
          padding: [10, 10, 10, 10],
          gap: 4,
        }}
        paint={() => ({
          radius: radii.lg,
          background: palette().panelBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
        })}
      >
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
          paint={() => ({ color: palette().textPrimary })}
        >
          Click any path
        </Text>
        <Text
          font={{ family: fontFamilies.sans, size: typeScale.tiny, weight: fontWeights.normal }}
          paint={() => ({ color: palette().textMuted })}
        >
          Files, lines and localhost links in terminal output open directly.
        </Text>
      </Column>
    </Column>
  );
}
