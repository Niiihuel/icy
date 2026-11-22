// Sidebar update announcement, header-left circular progress ring with Accordee popover,
// rich release notes modal (with video/media preview cards), and auto-restart warning modal.

function UpdateSidebarBanner(): Element {
  const up = () => updateState.value;
  const visible = () => up().hasUpdate || up().stage === "ready";
  return (
    <Show when={visible} style={{ shrink: 0 }}>
      {() => (
        <Row
          style={{
            height: 30,
            padding: [0, 8, 0, 8],
            align: "center",
            justify: "between",
            gap: 6,
            shrink: 0,
          }}
          onPress={() => {
            releaseNotesOpen.value = true;
          }}
          paint={() => ({
            radius: radii.md,
            background: palette().updateBannerBg,
            edge: palette().updateBannerEdge,
            edgeWidth: spacing.hairline,
            hover: { brightness: 1.12 },
            active: { scale: 0.98 },
            ...accordeeFadeUpMotion(),
          })}
        >
          <Row style={{ align: "center", gap: 6, grow: 1, minWidth: 0 }}>
            <Glyph name="sparkles" size={13} color={() => palette().updateBannerAccent} />
            <Text
              lines={1}
              font={{
                family: fontFamilies.sans,
                size: typeScale.tiny,
                weight: fontWeights.medium,
              }}
              paint={() => ({ color: palette().textPrimary })}
            >
              {() => {
                const shortVer = up().availableVersion.split("-")[0];
                return up().stage === "ready"
                  ? `v${shortVer} ready to install`
                  : `Update v${shortVer} available`;
              }}
            </Text>
          </Row>
          <Glyph name="chevron-right" size={11} color={() => palette().updateBannerAccent} />
        </Row>
      )}
    </Show>
  );
}

function HeaderUpdateButton(): Element {
  let anchorId: number | undefined;
  const up = () => updateState.value;
  const isBusy = () => up().stage === "downloading" || up().stage === "verifying";
  const isReady = () => up().stage === "ready";

  return (
    <Box
      ref={(id) => {
        anchorId = id;
      }}
      style={{
        width: 28,
        height: 28,
        align: "center",
        justify: "center",
        shrink: 0,
      }}
      onPress={() => {
        installPopoverOpen.value = !installPopoverOpen.value;
      }}
      paint={() => ({
        radius: 14,
        hover: { background: palette().panelBg, scale: 1.06 },
        active: { scale: 0.95 },
        transitions: [
          smoothTransition("background", motion.fast),
          smoothTransition("scale", motion.fast),
        ],
      })}
    >
      <Image source="update:ring" style={{ width: 26, height: 26 }} />

      {/* Accordee-animated installation popover */}
      <Popover
        open={installPopoverOpen}
        anchor={() => anchorId}
        side="bottom"
        align="start"
        layer={1100}
        style={{ width: 276, padding: [12, 12, 12, 12] }}
        paint={() => ({
          radius: radii.popover,
          background: palette().cardBg,
          edge: palette().border,
          edgeWidth: spacing.hairline,
          shadows: [shadows.popover],
          ...accordeeMenuMotion("bottom"),
        })}
      >
        <Column style={{ gap: 10 }}>
          <Row style={{ align: "center", justify: "between", gap: 8 }}>
            <Row style={{ align: "center", gap: 7 }}>
              <Glyph
                name={() => (isReady() ? "check" : "download")}
                size={14}
                color={() => (isReady() ? palette().successText : accent().brandText)}
              />
              <Text
                font={{
                  family: fontFamilies.sans,
                  size: typeScale.label,
                  weight: fontWeights.semibold,
                }}
                paint={() => ({ color: palette().textPrimary })}
              >
                icy Updater
              </Text>
            </Row>
            <CountPill count={() => `v${up().availableVersion}`} />
          </Row>

          <Text
            font={{ family: fontFamilies.sans, size: typeScale.caption }}
            paint={() => ({ color: palette().textSecondary })}
          >
            {() => up().statusText}
          </Text>

          {/* Animated installation progress bar */}
          <Column style={{ gap: 4 }}>
            <Box
              style={{ width: 252, height: 6, clip: true }}
              paint={() => ({
                radius: 3,
                background: palette().kbdBg,
              })}
            >
              <Box
                style={() => ({
                  width: Math.round((252 * Math.max(0, Math.min(100, up().progress))) / 100),
                  height: 6,
                })}
                paint={() => ({
                  radius: 3,
                  background: isReady() ? palette().successText : accent().brand,
                  transitions: [smoothTransition("width", motion.panel)],
                })}
              />
            </Box>
            <Row style={{ align: "center", justify: "between" }}>
              <Text
                font={{ family: fontFamilies.mono, size: typeScale.micro }}
                paint={() => ({ color: palette().textMuted })}
              >
                {() => `current v${up().currentVersion}`}
              </Text>
              <Text
                font={{ family: fontFamilies.mono, size: typeScale.micro }}
                paint={() => ({ color: palette().textSecondary })}
              >
                {() => `${up().progress}%`}
              </Text>
            </Row>
          </Column>

          <Row style={{ gap: 6 }}>
            <Show when={isReady} style={{ grow: 1 }}>
              {() => (
                <PrimaryButton
                  label="Install & Restart"
                  grow={1}
                  height={28}
                  onPress={() => {
                    installPopoverOpen.value = false;
                    restartConfirmOpen.value = true;
                  }}
                />
              )}
            </Show>
            <Show when={() => !isReady()} style={{ grow: 1 }}>
              {() => (
                <PrimaryButton
                  label={() => (isBusy() ? `Downloading ${up().progress}%` : "Download update")}
                  grow={1}
                  height={28}
                  disabled={isBusy}
                  onPress={startUpdateDownload}
                />
              )}
            </Show>
            <SecondaryButton
              label="Notes"
              height={28}
              onPress={() => {
                installPopoverOpen.value = false;
                releaseNotesOpen.value = true;
              }}
            />
          </Row>
        </Column>
      </Popover>
    </Box>
  );
}

function ReleaseNotesModal(): Element {
  const notes = () => updateState.value.notes;
  const isReady = () => updateState.value.stage === "ready";
  const isBusy = () =>
    updateState.value.stage === "downloading" || updateState.value.stage === "verifying";

  return (
    <Show when={releaseNotesOpen}>
      {() => (
        <Box
          style={() => ({
            position: "absolute",
            left: 0,
            top: 0,
            width: viewportSize.value[0],
            height: viewportSize.value[1],
            layer: 2000,
            align: "center",
            justify: "center",
            padding: [24, 24, 24, 24],
          })}
          onPress={() => {
            releaseNotesOpen.value = false;
          }}
          paint={() => ({
            background: palette().backdrop,
            starting: { opacity: 0 },
            transitions: [smoothTransition("opacity", motion.base)],
          })}
        >
          <Column
            style={{
              width: 580,
              maxHeight: 620,
              padding: [18, 20, 18, 20],
              gap: 14,
              clip: true,
            }}
            onPress={() => {
              // Capture click so clicking inside modal does not dismiss it.
            }}
            paint={() => ({
              radius: radii.xxl,
              background: palette().cardBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
              shadows: [shadows.modal],
              ...accordeeDialogMotion(),
            })}
          >
            {/* Header */}
            <Row style={{ align: "center", justify: "between", gap: 10 }}>
              <Row style={{ align: "center", gap: 8 }}>
                <Box
                  style={{
                    height: 22,
                    padding: [0, 8, 0, 8],
                    align: "center",
                    justify: "center",
                  }}
                  paint={() => ({
                    radius: radii.md,
                    background: palette().updateBannerBg,
                    edge: palette().updateBannerEdge,
                    edgeWidth: spacing.hairline,
                  })}
                >
                  <Text
                    font={{
                      family: fontFamilies.mono,
                      size: typeScale.tiny,
                      weight: fontWeights.medium,
                    }}
                    paint={() => ({ color: palette().updateBannerAccent })}
                  >
                    {() => `v${notes().version}`}
                  </Text>
                </Box>
                <Text
                  font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {() => notes().publishedAt}
                </Text>
              </Row>

              <Box
                style={{ width: 24, height: 24, align: "center", justify: "center" }}
                onPress={() => {
                  releaseNotesOpen.value = false;
                }}
                paint={() => ({
                  radius: radii.md,
                  hover: { background: palette().sidebarActive },
                })}
              >
                <Glyph name="close" size={12} color={() => palette().textSecondary} />
              </Box>
            </Row>

            <Column style={{ gap: 4 }}>
              <Text
                font={{
                  family: fontFamilies.sans,
                  size: typeScale.headingSm,
                  weight: fontWeights.semibold,
                }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() => notes().title}
              </Text>
              <Text
                font={{ family: fontFamilies.sans, size: typeScale.label, lineHeight: 1.45 }}
                paint={() => ({ color: palette().textSecondary })}
              >
                {() => notes().summary}
              </Text>
            </Column>

            {/* Scrollable body: Video/Media preview cards + text highlights */}
            <Column style={{ grow: 1, minHeight: 0, scroll: true, gap: 12 }}>
              <For each={() => notes().media} key={(m) => m.title} style={{ gap: 8 }}>
                {(media) => (
                  <Column
                    style={{ padding: [14, 16, 14, 16], gap: 10 }}
                    onPress={() => openExternal(media().url)}
                    paint={() => ({
                      radius: radii.xl,
                      gradient: palette().mediaGradient,
                      edge: palette().border,
                      edgeWidth: spacing.hairline,
                      hover: { brightness: 1.08 },
                      transitions: [smoothTransition("brightness", motion.fast)],
                    })}
                  >
                    <Row style={{ align: "center", justify: "between" }}>
                      <Row style={{ align: "center", gap: 8 }}>
                        <Box
                          style={{
                            width: 30,
                            height: 30,
                            align: "center",
                            justify: "center",
                          }}
                          paint={() => ({
                            radius: 15,
                            background: accent().brand,
                          })}
                        >
                          <Glyph name="play" size={12} color={() => accent().brandFg} />
                        </Box>
                        <Column style={{ gap: 2 }}>
                          <Text
                            font={{
                              family: fontFamilies.sans,
                              size: typeScale.label,
                              weight: fontWeights.semibold,
                            }}
                            paint={() => ({ color: accent().brandFg })}
                          >
                            {() => media().title}
                          </Text>
                          <Text
                            font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                            paint={() => ({ color: palette().textSubtle })}
                          >
                            {() => media().caption}
                          </Text>
                        </Column>
                      </Row>
                      <CountPill count={() => media().duration} />
                    </Row>
                  </Column>
                )}
              </For>

              <Column style={{ gap: 6 }}>
                <Text
                  font={{
                    family: fontFamilies.sans,
                    size: typeScale.caption,
                    weight: fontWeights.semibold,
                  }}
                  paint={() => ({ color: palette().textPrimary })}
                >
                  What's new in this release
                </Text>
                <For each={() => notes().highlights} key={(line) => line} style={{ gap: 5 }}>
                  {(line) => (
                    <Row style={{ align: "start", gap: 8 }}>
                      <Box
                        style={{ width: 5, height: 5, margin: [6, 0, 0, 0], shrink: 0 }}
                        paint={() => ({ radius: 3, background: accent().brand })}
                      />
                      <Text
                        style={{ grow: 1 }}
                        font={{ family: fontFamilies.sans, size: typeScale.caption, lineHeight: 1.45 }}
                        paint={() => ({ color: palette().textSecondary })}
                      >
                        {line}
                      </Text>
                    </Row>
                  )}
                </For>
              </Column>
            </Column>

            {/* Footer Actions */}
            <Row
              style={{
                padding: [12, 0, 0, 0],
                align: "center",
                justify: "between",
                gap: 8,
              }}
              paint={() => ({
                edge: palette().border,
                edgeWidth: spacing.hairline,
                edgeSides: [1, 0, 0, 0],
              })}
            >
              <SecondaryButton
                label="View on GitHub"
                height={30}
                onPress={() => openExternal(notes().releaseUrl)}
              />
              <Row style={{ gap: 8 }}>
                <SecondaryButton
                  label="Close"
                  height={30}
                  onPress={() => {
                    releaseNotesOpen.value = false;
                  }}
                />
                <PrimaryButton
                  label={() =>
                    isReady()
                      ? "Install & Restart"
                      : isBusy()
                        ? `Downloading ${updateState.value.progress}%`
                        : "Download Update"
                  }
                  height={30}
                  disabled={isBusy}
                  onPress={() => {
                    if (isReady()) {
                      releaseNotesOpen.value = false;
                      restartConfirmOpen.value = true;
                    } else {
                      startUpdateDownload();
                      releaseNotesOpen.value = false;
                      installPopoverOpen.value = true;
                    }
                  }}
                />
              </Row>
            </Row>
          </Column>
        </Box>
      )}
    </Show>
  );
}

function RestartConfirmModal(): Element {
  return (
    <Show when={restartConfirmOpen}>
      {() => (
        <Box
          style={() => ({
            position: "absolute",
            left: 0,
            top: 0,
            width: viewportSize.value[0],
            height: viewportSize.value[1],
            layer: 2100,
            align: "center",
            justify: "center",
            padding: [24, 24, 24, 24],
          })}
          onPress={() => {
            restartConfirmOpen.value = false;
          }}
          paint={() => ({
            background: palette().backdrop,
            starting: { opacity: 0 },
            transitions: [smoothTransition("opacity", motion.base)],
          })}
        >
          <Column
            style={{ width: 420, padding: [18, 20, 18, 20], gap: 14 }}
            onPress={() => {}}
            paint={() => ({
              radius: radii.xxl,
              background: palette().cardBg,
              edge: palette().border,
              edgeWidth: spacing.hairline,
              shadows: [shadows.modal],
              ...accordeeDialogMotion(),
            })}
          >
            <Row style={{ align: "center", gap: 10 }}>
              <Box
                style={{ width: 34, height: 34, align: "center", justify: "center", shrink: 0 }}
                paint={() => ({
                  radius: radii.lg,
                  background: palette().warningBg,
                })}
              >
                <Glyph name="alert" size={18} color={() => palette().warningText} />
              </Box>
              <Column style={{ gap: 2 }}>
                <Text
                  font={{
                    family: fontFamilies.sans,
                    size: typeScale.subtitle,
                    weight: fontWeights.semibold,
                  }}
                  paint={() => ({ color: palette().textPrimary })}
                >
                  Restart icy to install update?
                </Text>
                <Text
                  font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                  paint={() => ({ color: palette().textMuted })}
                >
                  {() =>
                    `v${updateState.value.currentVersion} → v${updateState.value.availableVersion}`
                  }
                </Text>
              </Column>
            </Row>

            <Text
              font={{ family: fontFamilies.sans, size: typeScale.caption, lineHeight: 1.5 }}
              paint={() => ({ color: palette().textSecondary })}
            >
              icy will close running terminal sessions, replace the binary with the verified
              update package, and relaunch automatically. Make sure any unsaved edits in your
              CLIs are saved before continuing.
            </Text>

            <Row style={{ justify: "end", gap: 8 }}>
              <SecondaryButton
                label="Later"
                height={30}
                onPress={() => {
                  restartConfirmOpen.value = false;
                }}
              />
              <PrimaryButton
                label="Restart & Install Now"
                height={30}
                onPress={() => {
                  restartConfirmOpen.value = false;
                  confirmRestartUpdate();
                }}
              />
            </Row>
          </Column>
        </Box>
      )}
    </Show>
  );
}
