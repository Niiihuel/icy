// Image viewer modal for inline terminal images: stepper (arrows / thumbnails / ←→ Home End),
// zoom (buttons, + − 0, mouse wheel, touchpad pinch) and pan (↑↓←→ while zoomed). While open the
// host routes keyboard, wheel and pinch here (`modalKeys`) instead of to the PTY.

interface ViewerImage {
  key: string;
  width: number;
  height: number;
  // File images load asynchronously: the entry resolves through `fileImages` once ready.
  path?: string;
}

function resolveViewerImage(img: ViewerImage): ViewerImage {
  if (!img.path) return img;
  return fileImage(img.path) ?? img;
}

const imageViewer = signal<{ images: ViewerImage[]; index: number } | null>(null);
const imageZoom = signal(1);
const imagePan = signal<[number, number]>([0, 0]);

const ZOOM_STEPS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8];
const PAN_STEP = 80;

function openImageViewer(images: ViewerImage[], index: number): void {
  if (images.length === 0) return;
  imageViewer.value = { images, index: Math.min(index, images.length - 1) };
  imageZoom.value = 1;
  imagePan.value = [0, 0];
  requestHost({ type: "modalKeys", on: true });
}

function closeImageViewer(): void {
  imageViewer.value = null;
  requestHost({ type: "modalKeys", on: false });
}

function stepImage(delta: number): void {
  const v = imageViewer.value;
  if (!v) return;
  const next = Math.max(0, Math.min(v.images.length - 1, v.index + delta));
  if (next === v.index) return;
  imageViewer.value = { ...v, index: next };
  imageZoom.value = 1;
  imagePan.value = [0, 0];
}

function zoomImage(direction: 1 | -1): void {
  const z = imageZoom.value;
  const next =
    direction > 0 ? ZOOM_STEPS.find((s) => s > z + 1e-3) : [...ZOOM_STEPS].reverse().find((s) => s < z - 1e-3);
  if (next === undefined) return;
  imageZoom.value = next;
  if (next <= 1) imagePan.value = [0, 0];
}

function panImage(dx: number, dy: number): void {
  imagePan.value = [imagePan.value[0] + dx, imagePan.value[1] + dy];
}

// Keys / wheel / pinch forwarded by the host while the viewer is open.
function handleModalKey(action: string): void {
  if (!imageViewer.value) return;
  const zoomed = imageZoom.value > 1;
  if (action === "escape") closeImageViewer();
  else if (action === "zoomIn") zoomImage(1);
  else if (action === "zoomOut") zoomImage(-1);
  else if (action === "reset") {
    imageZoom.value = 1;
    imagePan.value = [0, 0];
  } else if (action === "left") zoomed ? panImage(PAN_STEP, 0) : stepImage(-1);
  else if (action === "right") zoomed ? panImage(-PAN_STEP, 0) : stepImage(1);
  else if (action === "up" && zoomed) panImage(0, PAN_STEP);
  else if (action === "down" && zoomed) panImage(0, -PAN_STEP);
  else if (action === "first") stepImage(-1e6);
  else if (action === "last") stepImage(1e6);
}

function ImageViewerModal(): Element {
  const current = () => {
    const v = imageViewer.value;
    return v ? resolveViewerImage(v.images[v.index]) : undefined;
  };
  // Fit inside the stage (window minus chrome), then apply zoom.
  const stage = () => [viewportSize.value[0] - 96, viewportSize.value[1] - 196] as const;
  const size = () => {
    const img = current();
    if (!img) return [0, 0];
    const [sw, sh] = stage();
    const w = img.width || sw;
    const h = img.height || sh;
    const fit = Math.min(1, sw / w, sh / h);
    return [Math.round(w * fit * imageZoom.value), Math.round(h * fit * imageZoom.value)];
  };
  const count = () => imageViewer.value?.images.length ?? 0;
  const index = () => imageViewer.value?.index ?? 0;

  return (
    <Show when={() => imageViewer.value !== null}>
      {() => (
        <Box
          style={() => ({
            // fixed: relative to the window, not to the Show wrapper at the end of the flow.
            position: "fixed",
            left: 0,
            top: 0,
            width: viewportSize.value[0],
            height: viewportSize.value[1],
            layer: 2200,
          })}
          paint={() => ({
            ...modalScrim(),
            starting: { opacity: 0 },
            transitions: [smoothTransition("opacity", motion.base)],
          })}
        >
          {/* Click on the backdrop closes; the image and toolbar stop the press. */}
          <Box
            style={() => ({ position: "absolute", left: 0, top: 0, width: viewportSize.value[0], height: viewportSize.value[1] })}
            onPress={closeImageViewer}
          />

          {/* Toolbar */}
          <Row
            style={() => ({
              position: "absolute",
              left: Math.round(viewportSize.value[0] / 2 - 210),
              top: 20,
              width: 420,
              height: 44,
              padding: [0, 6, 0, 14],
              align: "center",
              gap: 2,
            })}
            onPress={() => {}}
            paint={() => ({
              radius: radii.xl,
              background: chromeTint(palette().appBg, glassChrome.surfaceAlpha),
              edge: palette().border,
              edgeWidth: spacing.hairline,
              shadows: [shadows.menu, shadows.menuTight],
              ...accordeeMenuMotion("top"),
            })}
          >
            <Text
              style={{ grow: 1 }}
              font={{ family: fontFamilies.sans, size: typeScale.label }}
              paint={() => ({ color: palette().textSecondary })}
            >
              {() => {
                const img = imageViewer.value?.images[index()];
                const name = img?.path ? basename(img.path) : "Image";
                return count() > 1 ? `${name} · ${index() + 1} of ${count()}` : name;
              }}
            </Text>
            <IconButton side="bottom" icon="chevron-left" label="Previous" keys={["←"]} disabled={() => index() <= 0} onPress={() => stepImage(-1)} />
            <IconButton side="bottom" icon="chevron-right" label="Next" keys={["→"]} disabled={() => index() >= count() - 1} onPress={() => stepImage(1)} />
            <Box style={{ width: 1, height: 18, margin: [0, 4, 0, 4] }} paint={() => ({ background: palette().border })} />
            <IconButton side="bottom" icon="zoom-out" label="Zoom out" keys={["−"]} onPress={() => zoomImage(-1)} />
            <Box style={{ width: 48, align: "center" }}>
              <Text
                font={{ family: fontFamilies.mono, size: typeScale.caption }}
                paint={() => ({ color: palette().textPrimary })}
              >
                {() => `${Math.round(imageZoom.value * 100)}%`}
              </Text>
            </Box>
            <IconButton side="bottom" icon="zoom-in" label="Zoom in" keys={["+"]} onPress={() => zoomImage(1)} />
            <IconButton side="bottom"
              icon="maximize"
              label="Fit"
              keys={["0"]}
              onPress={() => {
                imageZoom.value = 1;
                imagePan.value = [0, 0];
              }}
            />
            <IconButton side="bottom" icon="close" label="Close" keys={["Esc"]} onPress={closeImageViewer} />
          </Row>

          {/* Image: centred in the stage, scaled and panned with the same spring as the layout. */}
          <Box
            style={() => ({
              position: "absolute",
              left: Math.round(viewportSize.value[0] / 2 - size()[0] / 2 + imagePan.value[0]),
              top: Math.round(86 + stage()[1] / 2 - size()[1] / 2 + imagePan.value[1]),
              width: size()[0],
              height: size()[1],
            })}
            onPress={() => {}}
            paint={() => ({
              radius: radii.md,
              shadows: [shadows.modal],
              starting: { opacity: 0, scale: 0.96 },
              transitions: [
                springTransition("left"),
                springTransition("width"),
                springTransition("height"),
                smoothTransition("opacity", motion.base),
                smoothTransition("scale", motion.base),
              ],
            })}
          >
            <Dynamic value={() => current()?.key ?? ""} style={{ grow: 1 }}>
              {(key) =>
                key ? (
                  <Image source={key} style={{ grow: 1 }} />
                ) : (
                  <Column style={{ grow: 1, align: "center", justify: "center" }} paint={() => ({ background: palette().appBg })}>
                    <Text font={{ family: fontFamilies.sans, size: typeScale.label }} paint={() => ({ color: palette().textMuted })}>
                      Loading image…
                    </Text>
                  </Column>
                )
              }
            </Dynamic>
          </Box>

          {/* Stepper: thumbnails, active one outlined. */}
          <Show when={() => count() > 1}>
            {() => (
              <For
                each={() => (imageViewer.value?.images ?? []).map((img, i) => ({ img, i }))}
                key={(t) => `${t.i}:${t.img.key}`}
                style={() => ({
                  position: "absolute",
                  left: Math.round(viewportSize.value[0] / 2 - (count() * 52) / 2),
                  top: viewportSize.value[1] - 76,
                  direction: "row",
                  gap: 8,
                })}
              >
                {(t) => (
                  <Box
                    style={{ width: 44, height: 44, padding: [2, 2, 2, 2] }}
                    onPress={() => stepImage(t().i - index())}
                    paint={() => ({
                      radius: radii.md,
                      edge: t().i === index() ? accent().brand : palette().border,
                      edgeWidth: t().i === index() ? 2 : spacing.hairline,
                      background: palette().appBg,
                      opacity: t().i === index() ? 1 : 0.7,
                      hover: { opacity: 1 },
                      transitions: [smoothTransition("opacity", motion.fast), smoothTransition("edge", motion.fast)],
                    })}
                  >
                    <Dynamic value={() => resolveViewerImage(t().img).key} style={{ grow: 1 }}>
                      {(key) => (key ? <Image source={key} style={{ grow: 1 }} /> : <Box style={{ grow: 1 }} />)}
                    </Dynamic>
                  </Box>
                )}
              </For>
            )}
          </Show>
        </Box>
      )}
    </Show>
  );
}
