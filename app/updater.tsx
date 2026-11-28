// Sidebar update chip with an Accordee popover, release notes modal, and restart
// confirmation. Only visible when GitHub Releases reports a newer version or a download is staged.

function updateBusy(): boolean {
  const st = updateState.value.stage;
  return st === "downloading" || st === "verifying";
}

function updateReady(): boolean {
  return updateState.value.stage === "ready";
}

function updateFailed(): boolean {
  return updateState.value.stage === "error";
}

function updateChecking(): boolean {
  return updateState.value.checkStatus === "checking";
}

function updateCheckAvailable(): boolean {
  return !updateChecking() && !updateBusy() && !updateReady();
}

function updateStatusText(): string {
  const up = updateState.value;
  if (updateChecking()) return "Checking for updates…";
  if (up.checkStatus === "failed" && !updateBusy() && !updateReady() && !updateFailed()) {
    return up.hasUpdate
      ? "Could not refresh the release feed. The last known update is still available."
      : "Could not check for updates. Try again later.";
  }
  return up.statusText;
}

function shortVersion(v: string): string {
  return v.split("-")[0];
}

// Row-local rect of the hover gradient, pushed by the host each frame it moves (release_glow.rs).
const releaseGlowRect = signal<[number, number, number, number]>([0, 0, 0, 0]);

// The Release notes row's hover is jsnircord's Missions row, literally: Rive-baked pulse frames
// recoloured from the theme accent, following the pointer on Rive's curve, dimmed behind the
// label, faded in/out over 350 ms. The host renders the frames into `release:tab-gradient`.
function ReleaseNotesMenuItem(props: { label: string; onPress: () => void }): Element {
  const layout = releaseNotesLayout;
  const hovered = signal(false);
  let row: number | undefined;
  // Centre / inner / outer stops from the accent (the basis sheets bind them to R / G / B).
  const colors = () => {
    const brand = accent().brand;
    return [brand, mixColor(brand, 0x000000ff, layout.glowInner), mixColor(brand, 0x000000ff, layout.glowOuter)]
      .map((color) => color >>> 8);
  };
  const track = (inside: boolean) => {
    if (row === undefined) return;
    // Where the label's ink ends inside the row (menu padding + icon + gap + text).
    const content = layout.glowInkStart + props.label.length * layout.glowCharWidth;
    requestHost({ type: "releaseGlowTarget", id: row, inside, content, colors: colors() });
  };
  let tracked = "";
  effect(() => {
    const inside = hovered.value;
    const key = inside ? colors().join(",") : "";
    if (key === tracked) return;
    tracked = key;
    track(inside);
  });
  return <MenuItem icon="file-diff" label={props.label} onPress={props.onPress}
    ref={(id) => {
      row = id;
      onCleanup(() => {
        if (tracked) track(false);
        tracked = "";
        hovered.value = false;
        row = undefined;
      });
    }}
    onHover={(inside) => {
      if (row === undefined || inside === hovered.value) return;
      hovered.value = inside;
    }}
    backdrop={<Box style={() => {
      const [left, top, width, height] = releaseGlowRect.value;
      return { position: "absolute", left, top, width, height };
    }} paint={() => ({
      opacity: hovered.value ? 1 : 0,
      transitions: [{ property: "opacity", duration: layout.glowFade, easing: "bezier", bezier: [0.42, 0, 0.58, 1] }],
    })}>
      <Image source="release:tab-gradient" style={{ grow: 1 }} />
    </Box>}
  />;
}

function ReleaseCoverImage(props: { image: () => FileImageEntry | undefined; width: () => number; height: () => number }): Element {
  return (
    <Box style={() => {
      const image = props.image();
      if (!image) return { position: "absolute", width: 0, height: 0 };
      const scale = Math.max(props.width() / image.width, props.height() / image.height);
      const width = image.width * scale;
      const height = image.height * scale;
      return { position: "absolute", left: (props.width() - width) / 2, top: (props.height() - height) / 2, width, height };
    }}>
      <Image source={() => props.image()?.key ?? ""} style={{ grow: 1 }} />
    </Box>
  );
}

// Browser tiles use ordinary application IDs. Keep the modal's short-lived view separate.
let nextReleaseVideoId = Number.MAX_SAFE_INTEGER;

function ReleaseCoverVideo(props: { url: string; width: () => number; height: () => number }): Element {
  const id = nextReleaseVideoId--;
  let node: number | undefined;
  let opened = false;
  const url = props.url.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const background = (palette().appBg >>> 8).toString(16).padStart(6, "0");
  const foreground = (palette().textSecondary >>> 8).toString(16).padStart(6, "0");
  const page = '<!doctype html><meta charset="utf-8"><style>'
    + `body{margin:0;display:grid;place-items:center;height:100vh;background:#${background};color:#${foreground};font:menu}`
    + 'video{width:100vw;height:100vh;object-fit:contain}</style>'
    + `<video controls autoplay playsinline src="${url}" onerror="this.hidden=true;document.querySelector('p').hidden=false"></video>`
    + '<p hidden>Video unavailable</p>';
  effect(() => {
    const width = Math.round(props.width());
    const height = Math.round(props.height());
    if (width <= 0 || height <= 0) return;
    if (opened) {
      requestHost({ type: "browser", op: "resize", id, width, height });
    } else {
      opened = requestHost({ type: "browser", op: "open", id, width, height,
        url: "data:text/html;charset=utf-8," + encodeURIComponent(page) });
    }
  });
  onCleanup(() => {
    requestHost({ type: "browser", op: "pointer", id: 0, from: id });
    if (opened) requestHost({ type: "browser", op: "close", id });
    const states = { ...browserStates.value };
    delete states[id];
    browserStates.value = states;
  });
  return (
    <Box ref={(value) => { node = value; }} style={{ grow: 1, align: "center", justify: "center" }}
      onHover={(inside) => {
        if (inside && node !== undefined && !browserStates.value[id]?.error) {
          requestHost({ type: "browser", op: "pointer", id, node });
        } else {
          requestHost({ type: "browser", op: "pointer", id: 0, from: id });
        }
      }}
    >
      <Dynamic value={() => !!browserStates.value[id]?.error} style={{ grow: 1, align: "center", justify: "center" }}>
        {(failed) => failed ? (
          <GhostButton icon="external-link" label="Open video" onPress={() => openExternal(props.url)} />
        ) : (
          <Box style={() => ({ position: "absolute", left: 0, top: 0, width: props.width(), height: props.height() })}>
            <Image source={`browser:${id}`} style={{ grow: 1 }} />
          </Box>
        )}
      </Dynamic>
    </Box>
  );
}

function ReleaseNotesCover(props: {
  width: () => number; height: () => number; media: () => ReleaseMediaView | undefined;
}): Element {
  const layout = releaseNotesLayout;
  return (
    <Box style={() => ({ height: props.height(), minWidth: 0, shrink: 0, clip: true })}
      paint={() => ({
        gradient: [
          mixColor(palette().panelBg, accent().brand, layout.coverAccentMix),
          palette().panelBg,
        ],
      })}
    >
      <Dynamic value={() => props.media()?.url} style={{ grow: 1, minWidth: 0 }}>
        {(url) => {
          const playing = signal(false);
          const video = () => props.media()?.kind === "video";
          const imageUrl = () => video() ? props.media()?.posterUrl : url;
          const image = () => fileImage(imageUrl() ?? "");
          effect(() => {
            const target = imageUrl();
            if (!target || fileImages.value[target] !== undefined) return;
            fileImages.value = { ...fileImages.value, [target]: "loading" };
            if (!requestHost({ type: "releaseCover", url: target })) {
              fileImages.value = { ...fileImages.value, [target]: "error" };
            }
          });
          return (
            <Dynamic value={() => playing.value && video() && !!url} style={{ grow: 1, minWidth: 0 }}>
              {(showVideo) => showVideo && url ? (
                <ReleaseCoverVideo url={url} width={props.width} height={props.height} />
              ) : (
                <Box style={{ grow: 1, align: "center", justify: "center" }}>
                  <Show when={() => !!image()} style={{ position: "absolute", left: 0, top: 0 }}>
                    {() => <ReleaseCoverImage image={image} width={props.width} height={props.height} />}
                  </Show>
                  <Show when={() => !image()} style={{ position: "absolute", left: 0, top: 0,
                    width: props.width(), height: props.height() }}>
                    {() => <Box style={{ grow: 1 }} paint={() => ({
                      // Dots clip to this radius, not the dialog's, so match its top corners.
                      radius: [radii.xxl, radii.xxl, 0, 0],
                      mask: workspacePixelBlast.releaseMask,
                      gradient: [
                        withAlpha(palette().textMuted, workspacePixelBlast.baseOpacity),
                        withAlpha(accent().brandText, workspacePixelBlast.accentOpacity),
                      ],
                      a11y: { hidden: true },
                    })} />}
                  </Show>
                  <Show when={video}>
                    {() => (
                      <Box style={{ width: layout.coverPlaySize, height: layout.coverPlaySize, align: "center", justify: "center" }}
                        onPress={() => { playing.value = true; }}
                        paint={() => ({
                          radius: layout.coverPlaySize / 2, background: palette().cardBg,
                          edge: palette().border, edgeWidth: spacing.hairline,
                          cursor: "pointer", a11y: { role: "button", label: "Play video" },
                          hover: { scale: layout.coverPlayHoverScale }, active: { scale: controlState.pressScale },
                          transitions: [smoothTransition("scale", motion.fast)],
                        })}>
                        <Glyph name="play" size={layout.coverPlayIcon} color={() => palette().textPrimary} />
                      </Box>
                    )}
                  </Show>
                </Box>
              )}
            </Dynamic>
          );
        }}
      </Dynamic>
    </Box>
  );
}

function UpdateProgressBar(props: { width: number; height?: number }): Element {
  const height = props.height ?? 4;
  const pct = () => Math.max(0, Math.min(100, updateState.value.progress));
  return (
    <Box
      style={{ width: props.width, height, clip: true, shrink: 0 }}
      paint={() => ({ radius: height / 2, background: palette().kbdBg })}
    >
      <Box
        style={() => ({ width: Math.round((props.width * pct()) / 100), height })}
        paint={() => ({
          radius: height / 2,
          background: updateReady() ? palette().successText : accent().brand,
          transitions: [smoothTransition("width", motion.panel)],
        })}
      />
    </Box>
  );
}

function UpdateChip({ compact = false }: { compact?: boolean }): Element {
  let anchorId: number | undefined;
  const up = () => updateState.value;
  const visible = () => up().hasUpdate || up().stage !== "idle";
  const label = () => updateReady()
    ? "Restart to update"
    : updateBusy()
      ? `Updating ${up().progress}%`
      : updateFailed()
        ? "Update failed"
        : `Update v${shortVersion(up().availableVersion)}`;

  const chip = () => (
    <Row
      ref={(id) => {
        anchorId = id;
      }}
      style={{ width: compact ? spacing.button : undefined, height: spacing.control,
        padding: compact ? 0 : [0, spacing.unit * 2, 0, spacing.unit * 2],
        align: "center", justify: compact ? "center" : "start", gap: spacing.unit * 1.5, shrink: 0 }}
      onPress={() => {
        installPopoverOpen.value = !installPopoverOpen.value;
      }}
      paint={() => ({
        radius: radii.md,
        background: updateFailed() ? palette().warningBg : accent().brandSoft,
        hover: { brightness: 1.1 },
        active: { scale: 0.97 },
        starting: accordeeFadeUpMotion().starting,
        transitions: [
          ...accordeeFadeUpMotion().transitions,
          smoothTransition("brightness", motion.fast),
          smoothTransition("scale", motion.fast),
        ],
      })}
    >
      <Glyph
        name={() => (updateReady() ? "refresh" : updateFailed() ? "alert" : "download")}
        size={14}
        color={() => (updateFailed() ? palette().warningText : accent().brandText)}
      />
      {compact ? null : (
        <Text
          lines={1}
          font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
          paint={() => ({ color: updateFailed() ? palette().warningText : accent().brandText })}
        >
          {label}
        </Text>
      )}
      {compact ? null : (
        <Show when={updateBusy}>{() => <UpdateProgressBar width={36} height={3} />}</Show>
      )}

      <MenuPanel
        open={installPopoverOpen}
        anchor={() => anchorId}
        side="right"
        align="end"
        width={288}
        layer={1100}
      >
        <Column style={{ padding: [6, 8, 8, 8], gap: 10 }}>
          <Row style={{ align: "center", justify: "between", gap: 8 }}>
            <Column style={{ gap: 2 }}>
              <Text
                font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() => `icy v${shortVersion(up().availableVersion)}`}
              </Text>
              <Text
                font={{ family: fontFamilies.sans, size: typeScale.tiny }}
                paint={() => ({ color: palette().textMuted })}
              >
                {() => `You have v${up().currentVersion}`}
              </Text>
            </Column>
            <Glyph
              name={() => (updateReady() ? "check" : "download")}
              size={16}
              color={() => (updateReady() ? palette().successText : palette().textSecondary)}
            />
          </Row>

          <Text
            font={{ family: fontFamilies.sans, size: typeScale.caption, lineHeight: 1.45 }}
            paint={() => ({ color: updateFailed() ? palette().warningText : palette().textSecondary })}
          >
            {updateStatusText}
          </Text>

          <Show when={() => updateBusy() || updateReady()}>
            {() => <UpdateProgressBar width={256} />}
          </Show>
        </Column>
        <MenuSeparator />
        <Show when={() => !!up().asset || updateReady()}>{() => <MenuItem
          icon={() => (updateReady() ? "refresh" : "download")}
          label={() =>
            updateReady()
              ? "Install & restart"
              : updateBusy()
                ? `Downloading… ${up().progress}%`
                : updateChecking()
                  ? "Checking…"
                  : updateFailed()
                  ? "Retry download"
                  : "Download update"
          }
          onPress={() => {
            if (updateReady()) {
              installPopoverOpen.value = false;
              restartConfirmOpen.value = true;
            } else if (updateCheckAvailable()) {
              startUpdateDownload();
            }
          }}
        />}</Show>
        <ReleaseNotesMenuItem
          label="What's new"
          onPress={() => {
            installPopoverOpen.value = false;
            releaseNotesOpen.value = true;
          }}
        />
        <MenuItem
          icon="external-link"
          label="View on GitHub"
          onPress={() => {
            installPopoverOpen.value = false;
            openExternal(up().notes.releaseUrl);
          }}
        />
      </MenuPanel>
    </Row>
  );
  return (
    <Show when={visible} style={{ shrink: 0 }}>
      {() => compact ? <HintTooltip label={label} side="right">{chip()}</HintTooltip> : chip()}
    </Show>
  );
}

function ReleaseNotesModal(): Element {
  const layout = releaseNotesLayout;
  const width = () => Math.min(layout.width, Math.max(1, viewportSize.value[0] - 2 * layout.viewportInset));
  const padding = () => width() < layout.compactAt ? layout.compactPadding : layout.padding;
  const notes = () => updateState.value.notes;
  const highlights = () => [...new Set(notes().highlights)];
  const coverHeight = () => Math.max(1, Math.min(width() / layout.coverRatio, viewportSize.value[1] * layout.coverHeightFraction));
  const cover = () => notes().media.find((media) => media.kind === "video"
    || (media.kind === "image" && fileImages.value[media.url] !== "error"));
  const secondaryMedia = () => notes().media.filter((media) => media !== cover());
  const version = () => `v${notes().version}`;
  const title = () => notes().title || `icy ${version()}`;
  const showVersion = () => !title().split(/\s+/).some((part) => part === version() || part === notes().version);
  const hasSummary = () => notes().summary.trim().length > 0 && !highlights().includes(notes().summary);
  const close = () => { releaseNotesOpen.value = false; };
  const update = () => {
    close();
    if (updateReady()) {
      restartConfirmOpen.value = true;
    } else {
      startUpdateDownload();
      installPopoverOpen.value = true;
    }
  };

  return (
    <Show when={releaseNotesOpen}>
      {() => (
        <Box
          style={() => ({
            position: "fixed", left: 0, top: 0, width: viewportSize.value[0], height: viewportSize.value[1],
            layer: layout.layer, align: "center", justify: "center", padding: layout.viewportInset,
          })}
          onPress={close}
          onDismiss={close}
          paint={() => ({
            ...modalScrim(),
            starting: { opacity: 0 },
            transitions: [smoothTransition("opacity", motion.base)],
          })}
        >
          <Column
            style={() => ({
              width: width(), clip: true,
              maxHeight: Math.min(layout.maxHeight, Math.max(1, viewportSize.value[1] - 2 * layout.viewportInset)),
            })}
            onPress={() => {}}
            paint={() => ({
              radius: radii.xxl, background: chromeTint(palette().cardBg, glassChrome.surfaceAlpha), edge: palette().border,
              edgeWidth: spacing.hairline, shadows: [shadows.modal],
              a11y: { role: "dialog", label: "Release notes", modal: true },
              ...accordeeDialogMotion(),
            })}
          >
            <Column style={{ grow: 1, minHeight: 0, scroll: true }}>
              <ReleaseNotesCover width={width} height={coverHeight} media={cover} />
              <Column style={() => ({ padding: padding(), gap: layout.sectionGap, shrink: 0 })}>
                <Row style={{ align: "center", gap: spacing.unit * 2 }}>
                  <Glyph name="file-diff" size={layout.icon} color={() => accent().brandText} />
                  <Text font={{ family: fontFamilies.sans, size: typeScale.caption, weight: fontWeights.medium }}
                    paint={() => ({ color: palette().textSecondary })}>Release notes</Text>
                </Row>
                <Column style={{ gap: spacing.unit }}>
                  <Text font={{ family: fontFamilies.sans, size: layout.coverTitleSize, weight: fontWeights.medium }}
                    paint={() => ({ color: palette().textPrimary, a11y: { role: "heading" } })}>{title}</Text>
                  <Row style={{ align: "center", gap: spacing.unit * 2 }}>
                    <Show when={showVersion}>
                      {() => (
                        <Text font={{ family: fontFamilies.mono, size: typeScale.tiny }}
                          paint={() => ({ color: accent().brandText })}>{version}</Text>
                      )}
                    </Show>
                    <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
                      paint={() => ({ color: palette().textMuted })}>{() => notes().publishedAt}</Text>
                  </Row>
                </Column>
                <Show when={hasSummary}>
                  {() => (
                    <Text font={{ family: fontFamilies.sans, size: typeScale.label, lineHeight: 1.4 }}
                      paint={() => ({ color: palette().textSecondary })}>{() => notes().summary}</Text>
                  )}
                </Show>
                <Show when={() => highlights().length > 0}>
                  {() => (
                    <Column style={{ gap: spacing.unit * 2 }}>
                      <Text font={{ family: fontFamilies.sans, size: typeScale.body, weight: fontWeights.medium }}
                        paint={() => ({ color: palette().textPrimary, a11y: { role: "heading" } })}>
                        What’s new
                      </Text>
                      <For each={highlights} key={(line) => line} style={{ gap: spacing.unit * 2 }}>
                        {(line) => (
                          <Row style={{ align: "start", gap: spacing.unit * 2 }}>
                            <Box style={{
                              width: layout.bulletSize, height: layout.bulletSize,
                              margin: [layout.bulletTop, 0, 0, 0], shrink: 0,
                            }} paint={() => ({ radius: layout.bulletSize / 2, background: accent().brand })} />
                            <Text style={{ grow: 1, minWidth: 0 }}
                              font={{ family: fontFamilies.sans, size: typeScale.label, lineHeight: 1.4 }}
                              paint={() => ({ color: palette().textSecondary })}>{line}</Text>
                          </Row>
                        )}
                      </For>
                    </Column>
                  )}
                </Show>
                <For each={secondaryMedia} key={(media) => media.url} style={{ gap: spacing.unit * 2 }}>
                  {(media) => (
                    <Row
                      style={{ align: "center", gap: layout.sectionGap, padding: layout.sectionGap }}
                      onPress={() => openExternal(media().url)}
                      paint={() => ({
                        radius: radii.lg, background: palette().panelBg,
                        edge: palette().border, edgeWidth: spacing.hairline,
                        hover: { background: palette().sidebarActive },
                        transitions: [smoothTransition("background", motion.fast)],
                      })}
                    >
                      <Box style={{
                        width: layout.mediaIconSize, height: layout.mediaIconSize,
                        align: "center", justify: "center", shrink: 0,
                      }} paint={() => ({ radius: radii.md, background: accent().brandSoft })}>
                        <Glyph name={() => media().kind === "video" ? "play" : "external-link"} size={layout.icon}
                          color={() => accent().brandText} />
                      </Box>
                      <Column style={{ gap: spacing.unit, grow: 1, minWidth: 0 }}>
                        <Text font={{ family: fontFamilies.sans, size: typeScale.label, weight: fontWeights.medium }}
                          paint={() => ({ color: palette().textPrimary })}>
                          {() => media().title || (media().kind === "video" ? "View video" : "View image")}
                        </Text>
                        <Show when={() => !!media().caption}>
                          {() => (
                            <Text font={{ family: fontFamilies.sans, size: typeScale.caption }}
                              paint={() => ({ color: palette().textMuted })}>{() => media().caption}</Text>
                          )}
                        </Show>
                      </Column>
                      <Show when={() => !!media().duration}>
                        {() => <CountPill count={() => media().duration} />}
                      </Show>
                    </Row>
                  )}
                </For>
                <Show when={() => highlights().length === 0 && notes().media.length === 0 && !notes().summary.trim()}>
                  {() => (
                    <Row style={{ padding: layout.sectionGap, gap: spacing.unit * 2, align: "center" }}
                      paint={() => ({ radius: radii.lg, background: palette().panelBg })}>
                      <Glyph name="file-diff" size={layout.icon} color={() => palette().textMuted} />
                      <EmptyState compact title="No release notes published for this version yet." />
                    </Row>
                  )}
                </Show>
              </Column>
            </Column>

            <Box style={() => ({
              position: "absolute", left: width() - layout.coverCloseInset - iconButtonLayout.sm.size,
              top: layout.coverCloseInset, width: iconButtonLayout.sm.size, height: iconButtonLayout.sm.size,
            })} paint={() => ({ radius: radii.md, background: palette().cardBg })}>
              <IconButton icon="close" label="Close release notes" size="sm" side="bottom" layer={layout.layer + 1} onPress={close} />
            </Box>

            <Box style={() => ({
              direction: width() < layout.compactAt ? "column" : "row",
              padding: [layout.sectionGap, padding(), layout.sectionGap, padding()],
              align: width() < layout.compactAt ? "start" : "center",
              justify: "between", gap: spacing.unit * 2, shrink: 0,
            })} paint={() => ({
              background: palette().panelBg, edge: palette().border, edgeSides: [spacing.hairline, 0, 0, 0],
            })}>
              <GhostButton icon="external-link" label="View on GitHub" onPress={() => openExternal(notes().releaseUrl)} />
              <Show when={() => !!updateState.value.asset || updateReady()}>
                {() => (
                  <PrimaryButton
                    label={() => updateReady() ? "Install & Restart"
                      : updateBusy() ? `Downloading ${updateState.value.progress}%` : "Download Update"}
                    height={spacing.button}
                    disabled={() => updateBusy() || updateChecking()}
                    onPress={update}
                  />
                )}
              </Show>
            </Box>
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
            // fixed: relative to the window, not to the Show wrapper at the end of the flow.
            position: "fixed",
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
            ...modalScrim(),
            starting: { opacity: 0 },
            transitions: [smoothTransition("opacity", motion.base)],
          })}
        >
          <Column
            style={{ width: 420, padding: [18, 20, 18, 20], gap: 14 }}
            onPress={() => {}}
            paint={() => ({
              radius: radii.xxl,
              background: chromeTint(palette().cardBg, glassChrome.surfaceAlpha),
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
              icy will close this window, replace its executable, and reopen automatically.
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
