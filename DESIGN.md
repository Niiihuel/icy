# Sistema de Diseño de icy (Patrón Accordee)

Este documento fija el lenguaje visual de **icy**, extraído de [`https://demo.accordee.app/`](https://demo.accordee.app/) y adaptado al runtime nativo **JSNIR** (`app/theme.ts`).

> **Regla obligatoria:** Cualquier componente nuevo en `app/*.tsx` debe tomar todos sus valores (colores, radios, espaciados, pesos, sombras, duraciones y curvas) desde [app/theme.ts](app/theme.ts) y este documento. No se permiten valores hexadecimales ni medidas sueltas fuera del sistema.

---

## 1. Principios visuales

1. **Superficies de alto contraste con bordes finos (`0.5px`):** sin sombras pesadas ni neón. La jerarquía entre sidebar (`#0f0f10`), superficie principal (`#0a0a0a`) y tarjetas/paneles (`#171717`) se marca con saltos sutiles de luminancia y bordes hairline (`#ffffff1a` / `#0000001a`).
2. **Layout en marco moderno (`html[data-layout=modern]`):** el fondo exterior de la ventana usa `--ac-sidebar-bg`; el panel principal (`.app-main`) se incrusta con `margin: 8px 8px 8px 0`, `border-radius: 8px`, borde `0.5px` y sombra suave `0 1px 2px #0000000d`.
3. **Tipografía contenida:** pesos `400` (`normal`) y `440` (`medium` / `semibold` / `bold`) con **Inter** para toda la UI y **Geist Mono** para terminal, código, rutas, líneas y teclas (`kbd`).
4. **Acentos neutros por defecto, color para foco y acción:** el acento `neutral` usa blanco/negro invertido para acciones primarias y reserva azul (`#3b82f6` / `#60a5fa`) para `--ac-highlight` y enlaces clickeables.
5. **Chrome flotante sobre el tiling:** los modales y el Overview no son una tarjeta grande de alto fijo. Flotan piezas separadas sobre el workspace atenuado, así el tiling sigue a la vista: una barra de búsqueda a altura fija (`floatingLayout.topFraction` del alto de la ventana), un panel de resultados que mide lo que su contenido y una fila de atajos suelta al pie de la ventana. Todas usan la misma superficie (`floatingSurface()`: `cardBg` al `72%`, borde `0.5px`, radio `radii.lg`, sombra `card`) con desenfoque de fondo (`backdropBlur` `16px`, saturación `1.4`), así lo que queda detrás se lee como vidrio y no compite con el texto. El fondo del modal desenfoca todo el workspace (`24px`); el desenfoque entra y sale animado junto con la atenuación (`200ms`). Es el blur sin xray de niri (dual Kawase sobre lo ya dibujado debajo, recortado al radio del nodo), expuesto por JSNIR como `paint.backdropBlur`.
6. **Selección y hover con línea, no con relleno:** una fila elegida lleva un borde `0.5px` del acento al `50%` (`controlState.selectedLineMix`) y el hover uno de `textMuted` al `18%` (`hoverLineMix`), sin fondo (`selectableRowPaint()`). Una fila seleccionada junto a una con hover nunca se ven como dos cajas iguales. El radio es `radii.md`, concéntrico con el panel `radii.lg` que la contiene. El relleno queda para superficies, no para estados.
7. **Scroll que se desliza:** al recorrer una lista con el teclado, la fila elegida se desliza al centro del panel con el glide de JSNIR (`LazyRows` con `revealAlign: 0.5` y `revealSmooth`), en vez de saltar. El selector de workspaces lleva esto al extremo: la fila elegida queda fija y la lista se mueve debajo.

---

## 2. Colores (Dark por defecto y Light)

Todos los tokens literales están preservados en [design/accordee-tokens.css](design/accordee-tokens.css) y convertidos a `0xRRGGBBAA` en [app/theme.ts](app/theme.ts).

| Token CSS | Dark (default) | `theme.ts` (Dark) | Light | `theme.ts` (Light) | Uso |
| --- | --- | --- | --- | --- | --- |
| `--ac-app-bg` | `#0a0a0a` | `0x0a0a0aff` | `#ffffff` | `0xffffffff` | Fondo principal (terminal / canvas) |
| `--ac-sidebar-bg` | `#0f0f10` | `0x0f0f10ff` | `#fafafa` | `0xfafafaff` | Fondo exterior / sidebar |
| `--ac-sidebar-active` | `#1f1f22` | `0x1f1f22ff` | `#f0f0f1` | `0xf0f0f1ff` | Ítem activo en sidebar / fila seleccionada |
| `--ac-panel-bg` | `#171717` | `0x171717ff` | `#f4f4f5` | `0xf4f4f5ff` | Paneles secundarios, visor, hover de filas |
| `--ac-card-bg` | `#171717` | `0x171717ff` | `#ffffff` | `0xffffffff` | Tarjetas, composer, contenedores |
| `--ac-kbd-bg` | `#262626` | `0x262626ff` | `#e5e7eb` | `0xe5e7ebff` | Chips `kbd`, atajos, badges neutros |
| `--ac-text-primary` | `#fafafa` | `0xfafafaff` | `#09090b` | `0x09090bff` | Texto principal |
| `--ac-text-secondary` | `#a3a3a3` | `0xa3a3a3ff` | `#52525b` | `0x52525bff` | Texto secundario e ítems inactivos |
| `--ac-text-muted` | `#8a8a8a` | `0x8a8a8aff` | `#71717a` | `0x71717aff` | Placeholders, metadatos, números de línea |
| `--ac-text-subtle` | `#e5e5e5` | `0xe5e5e5ff` | `#3f3f46` | `0x3f3f46ff` | Texto de apoyo con alto contraste |
| `--ac-border` | `#ffffff1a` | `0xffffff1a` | `#0000001a` | `0x0000001a` | Bordes hairline (`0.5px`) y divisores |
| `--ac-send-idle` | `#3f3f46` | `0x3f3f46ff` | `#8e8e93` | `0x8e8e93ff` | Botón enviar inactivo |
| `--ac-highlight` | `#3b82f6` | `0x3b82f6ff` | `#3b82f6` | `0x3b82f6ff` | Selección (`24%` alpha) y foco de acento |
| `--destructive` | `#f87171` | `0xf87171ff` | `#ef4444` | `0xef4444ff` | Errores / acciones destructivas |
| `--ring` | `#52525b` | `0x52525bff` | `#a1a1aa` | `0xa1a1aaff` | Anillo de foco en modo neutral |

---

## 3. Acentos (`html[data-accent=...]`)

En modos con color, `--ac-brand-soft` y `--ring` usan `color-mix(in oklab, var(--ac-brand) P%, transparent)` (`20%` en dark y `12%` en light para `brand-soft`; `55%` para `ring`), calculados en [design/tools/extract-tokens.mjs](design/tools/extract-tokens.mjs).

| Acento | Dark `brand` | Dark `brandText` | Dark `brandSoft` (`20%`) | Light `brand` | Light `brandText` | Light `brandSoft` (`12%`) |
| --- | --- | --- | --- | --- | --- | --- |
| `neutral` | `0xfafafaff` (`fg 0x0a0a0aff`) | `0xfafafaff` | `0xffffff14` | `0x09090bff` (`fg 0xffffffff`) | `0x09090bff` | `0x0000000f` |
| `blue` | `0x3b82f6ff` | `0x60a5faff` | `0x3b82f633` | `0x2563ebff` | `0x1d4ed8ff` | `0x2563eb1f` |
| `violet` | `0x8b5cf6ff` | `0xa78bfaff` | `0x8b5cf633` | `0x7c3aedff` | `0x6d28d9ff` | `0x7c3aed1f` |
| `rose` | `0xf43f5eff` | `0xfb7185ff` | `0xf43f5e33` | `0xe11d48ff` | `0xbe123cff` | `0xe11d481f` |
| `orange` | `0xf97316ff` | `0xfb923cff` | `0xf9731633` | `0xea580cff` | `0xc2410cff` | `0xea580c1f` |
| `green` | `0x16a34aff` | `0x4ade80ff` | `0x16a34a33` | `0x16a34aff` | `0x15803dff` | `0x16a34a1f` |
| `teal` | `0x14b8a6ff` | `0x2dd4bfff` | `0x14b8a633` | `0x0d9488ff` | `0x0f766eff` | `0x0d94881f` |

---

## 4. Tipografía, Espaciado, Radios, Sombras y Motion

### Tipografía (`fontFamilies`, `typeScale`, `fontWeights`)

- **Sans UI:** `Inter` (`Jurui/assets/fonts/Inter-Regular.ttf`, `Inter-SemiBold.ttf` + `assets/fonts/Inter-Medium.ttf` v4.0)
- **Monospace (terminal / visor / `kbd`):** `Geist Mono` (`assets/fonts/GeistMono-Regular.ttf` + `GeistMono-SemiBold.ttf` v1.9 para negritas de terminal)
- **Escala:** `micro: 10`, `tiny: 11`, `caption: 12`, `label: 13`, `body: 14`, `subtitle: 15`, `title: 16`, `headingSm: 18`, `heading: 20`, `metric: 24`, `displaySm: 26`, `display: 30`.
- **Pesos:** `light: 300`, `normal: 400`, `medium / semibold / bold: 500`. Accordee usa `440` sobre Inter variable; JSNIR solo resuelve pesos de fuentes embebidas, y un peso sin archivo cae a otra familia (antes `440` se renderizaba en Geist Mono), por eso se embebe `Inter-Medium` (500).

### Espaciado y Geometría (`spacing`, `radii`)

- **Base:** `--spacing: 0.25rem` (`4px`), densidad compacta `0.21rem` (`3.36px`).
- **Controles:** `hairline: 0.5px`, `control: 28px`, `button: 30px`, `controlLg: 32px`, `toolbar: 44px`, `tabbar: 48px`, `--sidebar-menu-w: 180px`, `sidebarWidth: 232px`.
- **Radios:** `xs: 2px`, `sm / chip: 4px`, `md: 6px`, `lg: 8px`, `popover / base: 10px`, `xl / panel: 12px`, `2xl: 16px`.

### Sombras (`shadows`) y Z-Index

| Token `theme.ts` | Equivalente CSS | Valor JSNIR `Shadow` |
| --- | --- | --- |
| `dropSm` | `--drop-shadow-sm: 0 1px 2px #00000026` | `{ y: 1, blur: 2, spread: 0, color: 0x00000026 }` |
| `panel` | `.app-main: 0 1px 2px #0000000d` | `{ y: 1, blur: 2, spread: 0, color: 0x0000000d }` |
| `card` | `0 2px 6px -2px rgba(0,0,0,0.08)` | `{ y: 2, blur: 6, spread: -2, color: 0x00000014 }` |
| `popover` | `0 8px 24px -8px rgba(0,0,0,0.12)` | `{ y: 8, blur: 24, spread: -8, color: 0x0000001f }` |
| `modal` | `0 25px 50px -12px #00000040` | `{ y: 24, blur: 50, spread: -12, color: 0x00000040 }` |
| `dragGhost` | `.col-drag-ghost: 0 10px 28px -6px #00000038` | `{ y: 10, blur: 28, spread: -6, color: 0x00000038 }` |

Capas (`z-index`): sticky `1..3`, resizers `5`, popovers/drawers `66..70`, modales `80..91`, tooltips/sheets `100`, drag ghost `200` (en JSNIR `layer: 1` para popovers y `layer: 1000` para tooltips).

### Motion (`motion`)

- **Duraciones:** `fast: 150ms`, `base: 200ms`, `enter: 240ms`, `enterSlow: 280ms`, `panel: 300ms`.
- **Curvas:** `--ease-smooth: cubic-bezier(.16, 1, .3, 1)`, `--ease-out: cubic-bezier(0, 0, .2, 1)`, `--ease-in-out: cubic-bezier(.4, 0, .2, 1)`, `--ease-in: cubic-bezier(.4, 0, 1, 1)`.

---

## 5. Especificación de componentes (CSS extraído + adaptación IDE)

| Componente | Estados y medidas verificadas en CSS | Adaptación en `icy` |
| --- | --- | --- |
| **App Frame & Main Panel** | `.app-frame`: `background: var(--ac-sidebar-bg)`. `.app-main`: `margin: 8px 8px 8px 0`, `border: .5px solid var(--ac-border)`, `border-radius: 8px`, `box-shadow: 0 1px 2px #0000000d`. | `app/main.tsx`: `Row` con `sidebarBg` envolviendo el contenedor principal redondeado `8px`, **sin borde**: el marco se lee por el contraste `sidebarBg`/`appBg` (como la terminal, que tampoco lleva borde). |
| **Sidebar e ítems** | Fondo `--ac-sidebar-bg`; ítem activo `--ac-sidebar-active`; texto activo `--ac-text-primary`, inactivo `--ac-text-secondary`. | `app/sidebar.tsx`: ancho `232px`, filas `32px`, radio `6px`, `StatusDot` de `6px`. |
| **Botones** | Primario: `--ac-brand` / `--ac-brand-fg`, radio `6px`. Secundario: `--ac-panel-bg`, borde `.5px`, altura `28–32px`, radio `6px`. Ícono: `size-button` `30x30`. | `PrimaryButton`, `SecondaryButton` e `IconButton` en `app/ui.tsx` (ver §5.1). |
| **Composer / Input** | `.chatbox-glow` usa `--ac-card-bg`; botón send idle `--ac-send-idle`, listo `--ac-brand`. | `app/terminal.tsx`: panel `12px` radius, `--ac-panel-bg`, borde `0.5px`, `TextInput` en `Geist Mono 13px`. |
| **Kbd / Chips** | Fondo `--ac-kbd-bg`, `font-mono`, radio `--radius-chip` (`4px`). | `KbdChip` (`height: 22px`, `Geist Mono 11px`, variante acentuada con `brandSoft` + `ring`). |
| **Tabs (`.detail-tab`)** | Texto `--ac-text-muted`, hover `--ac-text-secondary`, `[aria-selected=true]` `--ac-text-primary` peso `440`, indicador inferior `1px` con escala X (`250ms ease-smooth`). | Reservado para tabs de sesiones / diffs / preview en Fase 1–2. |
| **Tablas / Filas (`.crm-table`)** | Celdas `40px` (`32px` compact, `52px` comfortable), padding `0 12px`, borde `.5px solid var(--ac-border)`, hover `--ac-panel-bg`, seleccionada `--ac-sidebar-active`. | Aplicable en lista de archivos tocados, to-dos y tabla de modelos/agentes (Fase 5). |
| **Checkbox (`.crm-check`)** | `16x16`, radio `4px`, borde `.5px`, checked `--ac-text-primary`, focus outline `2px solid #8b5cf6`. | Reservado para lista de to-dos y filtros de métricas. |
| **Scrollbars** | Ocultos por defecto en Accordee (`scrollbar-width: none`); en `icy` el scroll nativo de JSNIR se mantiene discreto sobre `appBg` / `panelBg`. | `scroll: true` en lista de sesiones, terminal y visor. |

---

### 5.1 Reglas unificadas de controles (verificadas en el CSS servido)

Todo botón, pestaña y menú de `app/` usa estos componentes y tokens. No se crean variantes locales.

- **Bordes:** siempre `spacing.hairline` (`0.5px`). Accordee fuerza `.border{border-width:.5px!important}` y lo mismo en `.border-t/b/l/r/x/y`, así que no existen bordes de `1px`. Solo la selección o un destino de arrastre usan `2px`, y el anillo del avatar usa `1.5px`. Los separadores de menú y del rail son `h-px` (`1px` de alto, relleno `border`).
- **Hover de borde:** `hoverBorder()` en `theme.ts` es `color-mix(in oklab, var(--ac-text-muted) 40%, transparent)` (`hover:border-textMuted/40`). Lo usan las tarjetas y `SecondaryButton`.
- **Estados (`controlState`):** el primario usa `hover` `opacity .9`. Cualquier control deshabilitado queda en `opacity .4`. Al presionar, los botones con texto usan `scale .98` y los de solo ícono `scale .95`.
- **`IconButton`:** es el único botón de ícono. Está transparente en reposo, usa `sidebarActive` en hover, radio `6px`, glifo `textSecondary` → `textPrimary` y tooltip con atajo. `md` mide `30px` con glifo de `16px`. `sm` mide `24px` con glifo de `14px` y es para headers de card de `30px` y barras densas. Con `highlighted` usa `brandSoft` + `brandText`.
- **`SegmentedControl`:** pista de `28px`, fondo `appBg`, borde hairline, inset `2px` y segmento `sidebarActive` con radio `4px`. Con `width` fijo, los segmentos se reparten el ancho y un indicador se desliza con el resorte de layout. Sin `width`, cada segmento se ajusta a su texto. Lo usan el filtro de sesiones, la densidad y el período de métricas.
- **Menús (`MenuPanel`):** fondo `appBg`, borde hairline, radio `10px`, padding `6px`, sombra `shadow-xl` de dos capas (`shadows.menu` + `shadows.menuTight`). Las cards flotantes (barra del visor, preview de imagen) usan la misma sombra.
- **Pestañas del header:** el hover y la pastilla activa comparten radio `6px`.

## 6. Reglas de layout del IDE

- **Terminal-first:** el pane de terminal ocupa el máximo espacio flexible (`grow: 1`) con tipografía `Geist Mono 13px` / `lineHeight: 19px`.
- **Editor / Visor bajo demanda:** el visor (`480px`, `--ac-panel-bg`, borde izquierdo `0.5px`) solo se monta cuando `viewerState.value.open === true`.
- **Enlaces clickeables:** toda referencia detectada en terminal usa `brandText` + subrayado. El área de clic coincide con las celdas enlazadas y la vista previa queda debajo de la barra de pestañas.

---

## 7. Cómo actualizar la referencia local

Las capturas y archivos CSS crudos viven bajo `design/reference/` (ignorado en Git porque este repositorio es público). Para volver a descargar el HTML y CSS de `https://demo.accordee.app/` y regenerar `design/accordee-tokens.css`:

```bash
bash design/fetch-reference.sh
node design/tools/extract-tokens.mjs
```

---

## 8. Especificación detallada de componentes medidos en `demo.accordee.app`

> **Nota metodológica:** Se inspeccionó `https://demo.accordee.app/` en modo Dark y Light junto con sus temas (`Neutral`, `Blue`, `Violet`, `Rose`, `Orange`, `Green`, `Teal`), fuentes y densidad (`Comfortable`/`Compact`). Como el sandbox del navegador deshabilita `page.evaluate()` / `$eval`, los estilos exactos se resolvieron cruzando clases DOM reales, atributos de estado, `boundingBox()` medidos en pantalla y los bundles CSS servidos.

### Sidebar e ítems de navegación
- **Sidebar (`app-sidebar`):** ancho expandido `196px`, `padding: 8px 4px`, fondo `--ac-sidebar-bg` (`#0f0f10` dark / `#fafafa` light), sin borde derecho en `modern` layout (`border-right-width: 0`).
- **Ítem de navegación:** `height: 28px`, `border-radius: 6px`, `padding: 6px 8px 6px 6px`, `gap: 8px`, transición `300ms cubic-bezier(.16, 1, .3, 1)`. Estado `hover` y ruta activa (`active`) aplican `--ac-sidebar-active` (`#1f1f22` dark / `#f0f0f1` light).
- **Badge de conteo / `kbd`:** `16px` de alto (`16x16` en shortcuts), `border-radius: 9999px` (badge) o `4px` (`kbd`), fondo `--ac-kbd-bg` (`#262626` dark / `#e5e7eb` light), `10px`, peso `440`.

### Botones
- **Primary (`+ Add Card`, `+ New invoice`, `Done`):** fondo `--ac-brand`, texto `--ac-brand-fg`, `height: 28px` / `30px`, `border-radius: 6px`, `padding: 0 12px` (`0 12px 0 10px` con ícono), `gap: 6px`, `13px`, peso `440`, `box-shadow: 0 1px 2px #0000000d`. `hover` → `opacity: 0.9`; `disabled` → `opacity: 0.4`; `active` → `scale: .98`.
- **Secondary (`Edit Cards`, `See plans`, chips de prompt):** fondo `--ac-app-bg` / `--ac-card-bg`, texto `--ac-text-secondary` o `--ac-text-primary`, `border: .5px solid var(--ac-border)`, `height: 28px / 30px / 32px`, `border-radius: 6px`, `padding: 0 10px` / `0 12px`, `box-shadow: 0 1px 2px #0000000d`. `hover` → `--ac-panel-bg`.
- **Ghost (`Share`, `Filter`, `Quick Chat`, `Reset`):** `background: transparent`, `height: 28px / 30px`, `border-radius: 6px`, `padding: 4px 8px 4px 6px`, `gap: 6px`. `hover` / `data-state=open` → fondo `--ac-panel-bg` + texto `--ac-text-primary`.
- **Icon Button (`Close`, `Send`):** `size-button` `30x30` (`24x24` en headers densos), `border-radius: 6px`, `active` → `scale: .95`. Botón `Send` usa `--ac-send-idle` (`#3f3f46` dark / `#8e8e93` light) en reposo y `--ac-brand` / `--ac-brand-fg` cuando está listo.

### Inputs, Selects y Composer
- **Input estándar / Wrapper de búsqueda:** `height: 32px` (`28px` en el Search del sidebar, a la altura del botón de colapsar), `padding: 0 10px` (`0 8px` con `gap: 8px` en búsqueda), `border: .5px solid var(--ac-border)`, `border-radius: 6px`, fondo `--ac-app-bg`, `14px`. `focus` / `focus-within` → borde `--ac-text-muted`.
- **Select / Combobox:** `height: 36px`, `padding: 8px 12px`, `border-radius: 6px`, `box-shadow: 0 1px 2px #0000000d`. `focus-visible` → borde `--ring` + halo `0 0 0 3px color-mix(in oklab, var(--ring) 50%, transparent)`.
- **Composer (`Ask Rune`):** contenedor `--ac-card-bg`, `border: .5px solid var(--ac-border)`, `border-radius: 18px`, `padding: 10px`, `gap: 12px`, `min-height: 106px`, sombra doble (`0 1px 2px, 0 6px 20px -6px`) que crece en `:focus` (`0 14px 34px -10px`). Textarea `14px` / `line-height: 22px`.

### Cards, Tablas, Tabs, Dropdowns, Modales, Tooltips y Controles
- **Metric Card:** `--ac-card-bg`, `border: .5px solid var(--ac-border)`, `border-radius: 8px`, `padding: 12px 16px`, `height: 144px`. `hover` → borde `color-mix(in oklab, var(--ac-text-muted) 40%, transparent)`.
- **Rich Card:** `border-radius: 12px`, `padding: 16px`, `gap: 14px`, `box-shadow: 0 1px 2px #0000000d`; `hover` → `shadow-sm` + borde atenuado.
- **Tablas (`.crm-table`):** `border-bottom: 0.5px solid var(--ac-border)`, `border-right: 0.5px solid var(--ac-border)`, filas `40px` (`32px` compact), `tfoot 44px` (`36px` compact), `padding: 0 12px`. `hover` → `--ac-panel-bg`; `selected` → `--ac-sidebar-active`.
- **Tabs de filtro & Segmented Control:** filtros `height: 28px`, `border-radius: 6px`, `padding: 4px 10px`, seleccionado (`aria-selected`) con `--ac-panel-bg` + borde `.5px` en `--ac-text-primary`. Segmented control con contenedor `height: 28px`, `padding: 2px`, ítems `23px`, `12px`, `border-radius: 4px–5px`.
- **Dropdown Menu:** `border-radius: 10px`, `padding: 6px`, `border: .5px solid var(--ac-border)`, `shadow-xl` (`0 20px 25px -5px` + `0 8px 10px -6px`). Ítems `height: 32px`, `padding: 0 8px`, `gap: 10px`, `13px`, `hover/focus-visible` → `--ac-panel-bg` + `--ac-text-primary`.
- **Modales (`Themes`, `Search`):** en Accordee son una tarjeta de `border-radius: 16px` con anchos `480px` / `700px` y sombra profunda. icy los reemplaza por el chrome flotante (principio 5): barra de búsqueda y panel de resultados separados, con `floatingSurface()` y el fondo `backdrop` que aparece y se va en `150ms`.
- **Tooltips (`HintTooltip` en `app/ui.tsx`):** fondo invertido `--foreground` (`#fafafa` dark / `#09090b` light), texto `--background`, `border-radius: 6px`, alto `26px`, `padding: 0 8px`, `12px`. Flecha (`glyph:caret-*`) apuntando al ancla, detalle opcional atenuado y chips de atajo (`18px`, fondo `tooltipKbdBg`) con separador `then` (p. ej. `Inbox [G] then [I]`). Entrada `fade-in-0 zoom-in-95` + desplazamiento `4px` desde el lado del ancla en `150ms --ease-smooth`. Solo se muestran atajos reales (`Ctrl+⇧+T/N/D/M`, ver `window.rs`).
- **Menús (`MenuPanel`, `MenuLabel`, `MenuItem`, `MenuSeparator`):** panel `--popover` (`--ac-app-bg`), borde `.5px`, radio `10px`, `padding: 6px`, sombra `shadow-xl`; label en mayúsculas `11px` muted con `letter-spacing: .04em`; ítems `32px`, ícono `16px` + texto `13px`, `gap: 10px`, seleccionado/hover `--ac-panel-bg`, check a la derecha; separador `1px` a sangre completa.
- **Sidebar compacta (rail):** ancho `52px`, botones `36x36` radio `8px`, íconos `18px`, divisores `18x1px`, tooltips a la derecha con atajo. Se alterna con el botón `panel-left`; el host recibe `sidebarWidth` para recalcular columnas/filas del PTY.
- **Avatares de workspace:** cuadrado redondeado (`30%` del lado) con degradado de dos paradas elegido por hash del nombre (`avatarGradients` en `theme.ts`).
- **Switch & Checkbox:** switch `32x18px`, pill `9999px` (`--ac-kbd-bg` off → `--ac-brand` on); checkbox `16x16px`, radio `4px–5px`, `focus-visible` con `outline: 2px solid #8b5cf6; outline-offset: 2px`.
- **Avatars:** `18x18px` circular (`9999px`) con borde `1.5px solid var(--ac-app-bg)`.

### Chrome de página (`app/tabs.tsx`)
- El header de 44 px reúne el acceso a Workspaces, la tira horizontal de pestañas del workspace activo y el botón `+` que abre la paleta. La cabecera usa el fondo del sidebar. El divisor bajo las pestañas se une a los laterales y a las esquinas inferiores del área de contenido en un trazo de `1px`. La pestaña activa comparte el fondo del contenido y su contorno se une al divisor. New session está en el sidebar. El aviso de actualización aparece junto al pie del sidebar y usa un ícono en el rail.

### Estados vacíos

- `EmptyState` en `app/ui.tsx` presenta un título breve y, cuando aporta contexto, una línea de ayuda en `textMuted`. El contenido se apoya directamente sobre la superficie de la vista, sin tarjeta, marco, separador ni ícono decorativo.
- El título principal usa Inter `16px` en peso medio. Dentro de paneles, listas y búsquedas se usa la variante compacta, Inter `12px` en `textMuted`.
- Si hay acciones, se muestran como filas `MenuItem` de una sola línea con ícono y el atajo vigente cuando existe. El workspace centra el bloque con el contenido alineado a la izquierda, un ancho máximo de `280px` y `12px` entre el título y las filas. Las medidas viven en `emptyStateLayout` de `app/theme.ts`.

### Atajos editables (Settings › Layout & Shortcuts)
- Catálogo de acciones en `icy-core::keybinds::ACTIONS` (id, nombre, grupo, descripción, atajos por defecto). La config guarda solo los cambios (`keybinds`: id → combinaciones; lista vacía = sin asignar).
- Cada fila muestra el nombre, la descripción, los chips de teclas y un grupo de iconos con tooltips. El teclado inicia la captura, la cruz cancela o desasigna y la flecha circular restaura el valor predeterminado cuando hay cambios. Durante la captura se muestra «Press keys…» y el chip de `Esc`. Si la combinación estaba en uso, se reasigna y el aviso indica de qué acción se quitó.
- Reglas: hace falta Ctrl o Alt (o una tecla F) para no robarle teclas a la terminal; las combinaciones Ctrl+Shift funcionan también con un campo de texto enfocado; con Ctrl+Alt, si la distribución produce otro carácter (AltGr) se escribe en vez de disparar el atajo. `Ctrl+Shift+V` (pegar) queda fijo.
- Los tooltips, los menús y el estado vacío leen el atajo vigente con `keysFor(id)`.
- Alt + flechas cambia el foco. Izquierda y derecha recorren columnas. Arriba y abajo recorren los panes de la columna activa. Alt + Shift + flechas reordena la columna o el pane enfocado. Ctrl + Alt + flechas cambia el tamaño y Ctrl + Alt + C centra la columna en modo Niri.

### Pestañas y panes estilo Silica (dqhieu/Silica)
- **Pestaña conectada en línea:** un solo contorno SVG de `1px` reemplaza el tramo correspondiente de la línea inferior del header. Las esquinas superiores y las curvas cóncavas inferiores tienen radio `8px`; no hay borde debajo de la pestaña. El contorno se rasteriza a la escala de pantalla y se desliza entre pestañas con el resorte de layout. Referencias: [trazados SVG en MDN](https://developer.mozilla.org/en-US/docs/Web/SVG/Tutorials/SVG_from_scratch/Paths) y [contorno de pestaña en Chromium](https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/ui/views/tabs/common/horizontal_tab_style_views.cc). Las inactivas se atenúan (título `textMuted`, ícono al `55%`) y en hover muestran una pastilla `sidebarActive` flotante de `30px`. El sidebar de proyectos usa los mismos estados: sesión activa con borde de línea, hover con pastilla y el resto atenuado; el proyecto de la sesión enfocada va en `textPrimary`.
- Las pestañas miden 156 px y la tira muestra la activa al cambiar de sesión. Una terminal de shell usa el nombre de la carpeta. La pestaña activa muestra el botón de cierre y las demás lo muestran al pasar el puntero.
- Las pestañas y la fila de CLIs comparten el desplazamiento horizontal. Cuando la tira desborda, cada flecha ocupa un espacio propio junto al contenido recortado y se anima con el resorte del tiling. Se puede avanzar con las flechas, el gesto horizontal del touchpad o Shift + rueda. Al llegar al extremo, la flecha queda atenuada y deshabilitada hasta que el puntero la deja.
- **Panes como cards:** radio `10px`, `appBg`, borde `.5px` (`border`; el enfocado usa `ring` + sombra `card` en vez de un borde de acento). Header `30px`: ícono del CLI `14px`, título `13px` y, al hover o con foco, `Fullscreen pane` y `Close` (botones `24px` con tooltip y atajo). Para apilar una shell debajo se usa el atajo `Split terminal down`. Para apilar una ventana abierta se puede arrastrar a la mitad inferior de un pane.
- El resize por arrastre muestra una línea de acento de 2 px en el borde y mueve las ventanas sin resortes mientras sigue el puntero. Tanto la previsualización como el resultado al soltar usan `icy-core`. En los dos modos, el borde compartido reparte el ancho entre las dos columnas vecinas, así que ninguna se desplaza fuera de la vista. En Niri, el borde exterior (antes de la primera columna o después de la última) agranda o achica esa columna.

### Tiling y vistas del workspace
- El compositor usa el modo configurado en Settings. Niri desplaza las columnas y Hyprland reparte el ancho del área visible. Los panes apilados comparten el alto de una columna.
- **Aperturas en Niri:** una ventana o CLI nueva se inserta inmediatamente a la derecha de la columna activa con el ancho predeterminado y recibe el foco. En modo Focus, la ventana nueva ocupa toda el área. Para crear una shell apilada se usa `Split terminal down`.
- **Grid** distribuye los panes en grilla con bordes arrastrables.
- **Gaps:** `16px` (`spacing.paneGap`) entre ventanas y alrededor de ellas, como `gaps 16` de niri. Una columna sola maximizada con Maximize column conserva los gaps y la card; Focus es el único modo sin gaps.
- **Focus** (`Ctrl+⇧+M`) muestra el pane activo a pantalla completa dentro del workspace. Al cambiar de pestaña, el pane elegido ocupa toda el área. El resorte de layout anima el cambio de tamaño y desplaza las columnas vecinas a los lados y los panes apilados hacia arriba o abajo. El borde, el encabezado y las esquinas del pane se desvanecen con el mismo resorte. La terminal recibe su tamaño nuevo al empezar, así el programa redibuja durante la animación y no al final.
- **Overview** (`Alt+O`, el `Mod+O` de niri) aleja los workspaces reales con zoom 0.5, como niri. La ventana seleccionada lleva un anillo de acento de `2px`.
- Abrir una columna, dividir hacia abajo y centrar una columna tienen atajos editables en Settings › Layout & Shortcuts. En la grilla de Niri, la búsqueda de la paleta y el botón del encabezado del pane ofrecen Maximize column / Restore column width (`Alt+F`, el `Mod+F` de niri). Maximize column usa todo el ancho disponible y conserva los panes apilados, los gaps y el marco de la grilla. [Modos de tamaño en Niri](https://niri-wm.github.io/niri/Fullscreen-and-Maximize.html)
- Al arrastrar una pestaña o el encabezado de un pane, una miniatura de su contenido sigue el puntero. La guía de inserción marca la posición y el tamaño que tendrá el pane en el layout al soltar. [Guía de movimiento y resize de Niri](https://github.com/niri-wm/niri/wiki/Gestures)

### Workspaces estilo niri
- Lista vertical de workspaces, cada uno con su propio tiling (columnas, focus, Grid/Focus). Siempre hay **un workspace vacío al final**; abrir o soltar una ventana ahí crea otro vacío debajo; un workspace vacío en el medio desaparece cuando dejás de estar en él. Las pestañas del header muestran solo las ventanas del workspace activo.
- Teclado (editables, con `Alt` como el `Mod` de niri porque `Super` es del compositor): `Alt` solo enfoca, `Alt+Shift` mueve y `Ctrl+Alt` cambia tamaños o mueve columnas enteras. `Alt+U/I` o `Alt+PageDown/PageUp` cambian de workspace; `Alt+Shift+U/I` (o `+PageDown/PageUp`) mueven la ventana enfocada al de abajo/arriba (el foco la sigue; desde el primero, "arriba" crea uno nuevo arriba); `Ctrl+Alt+U/I` mueven la columna entera. Solo se usan letras y teclas con nombre, que no cambian entre distribuciones de teclado.
- Overview (`Alt+O`, [overview de niri](https://niri-wm.github.io/niri/Configuration%3A-Miscellaneous.html#overview)): los mismos workspaces que se ven fuera del Overview, vivos y apilados en vertical con un hueco del 10 % de su alto, alejados con zoom 0.5 alrededor del workspace de la selección. No hay miniaturas: cada ventana se dibuja una sola vez, con su contenido real. En el hueco sobre cada workspace va su nombre (la carpeta de su terminal enfocada), la cantidad de ventanas y `Alt N`, que lo abre; las etiquetas siguen a la pila con el mismo resorte. El encabezado y el fondo del Overview forman una sola superficie sin marco exterior ni divisor. Solo queda el botón de workspaces. Cada workspace tiene el fondo de la app y radio `radii.panel` sobre el fondo `sidebarBg`. ↓↑ recorre panes apilados y luego workspaces (la pila se centra en el workspace elegido), ←→ cambia de columna. Enter o clic abre la ventana en el estado que tenía; clic en el fondo vuelve.
- **Filtro del Overview:** una barra flotante arriba (`/` o un clic la abre, `Esc` la limpia). Busca por título, carpeta y agente, y por estado: `@working`, `@blocked`, `@stopped`, `@idle`, `@ai`. Al escribir `@` aparecen las sugerencias en un panel flotante; `Tab` completa la primera, marcada con línea. Las ventanas que no coinciden bajan a `overviewFilterLayout.dimOpacity` en su lugar, sin mover la geometría, y la selección salta a la primera que coincide. Sin coincidencias: «No windows match».
- **Selector de workspaces** (`Alt+W`, editable): la misma barra flotante y debajo una rueda de filas de una línea (nombre, ventanas en `textMuted`, número en Geist Mono). La fila elegida queda fija en el centro, con borde de acento, y la lista se desliza debajo con el resorte de niri; las demás pierden opacidad y escala según su distancia (`workspaceSwitcherLayout`). Sin íconos ni tarjetas.
- **Pastilla de workspace:** al cambiar de workspace aparece abajo, centrada, `• Workspace N / M · nombre` sobre `floatingSurface()` con radio completo, durante `1.2s`, con fade-up. No aparece al restaurar la sesión ni con el Overview abierto.
- **Add workspace** aparece arriba del workspace vacío del final, la parte que asoma bajo el workspace centrado. Al seleccionarlo con ↓ la pila lo centra, el texto pasa de `textMuted` a `textPrimary` y el workspace lleva el anillo de acento. Clic o Enter lo abre.
- La entrada y la salida del Overview son un solo zoom de la pila con el resorte de niri (`overview-open-close`, rigidez 800, 326 ms). Al cerrar, la pila se centra en el workspace elegido mientras vuelve a zoom 1. Los cambios de workspace mueven la pila entera en vertical con el resorte de `workspace-switch` (rigidez 1000, 291 ms); entre workspaces se ve el fondo `sidebarBg`. [Código del monitor en niri](https://github.com/niri-wm/niri/blob/main/src/layout/monitor.rs) [Animaciones de niri](https://niri-wm.github.io/niri/Configuration%3A-Animations.html#overview-open-close)
- Al arrastrar una ventana en el Overview, una miniatura sigue el puntero y la ventana original se atenúa. El workspace destino se ilumina al mover a otro workspace. El hueco sobre un workspace inserta uno nuevo ahí (su borde superior se marca). El vacío del final dice «Drop here for a new workspace».
- El botón de workspaces está a la izquierda del primer tab: grilla de cuatro cuadrados y número del workspace activo; abre y cierra el overview (tooltip con su atajo). El sidebar ya no tiene selector de workspaces: su primera fila es la búsqueda con el botón de colapsar. Si el workspace está vacío, la tira queda vacía y el área muestra `EmptyState` con las acciones New session, Add project folder, Open launcher y Workspace overview.
- Al reabrir icy se recuperan las terminales del servidor de sesión con sus workspaces y su foco. Si todavía no hay sesiones, icy arranca en un workspace vacío y usa la carpeta de lanzamiento como punto de partida para New session.
- **Workspace vacío:** [Pixel Blast de React Bits](https://reactbits.dev/backgrounds/pixel-blast) inspira la grilla de círculos del panel principal, incluida la barra de pestañas. Los puntos tienen un paso de `7.5px` lógicos y forman nubes que se desplazan y centellean. Una onda de `1.8s` revela la grilla al entrar, el cursor ilumina un radio de `140px` con una estela y cada clic propaga una onda. La mezcla va de `textMuted` al `brandText` activo, con opacidades máximas de `32%` y `57%` en `workspacePixelBlast`. Los bordes se difuminan y el centro se atenúa para dar contraste a las acciones. El bloque de acciones se puede desplazar en ventanas bajas.
- New session abre la paleta directamente en la lista de agentes y Terminal. Una vez elegida la opción, la misma paleta muestra las carpetas. Confirmar abre la sesión en el tiling junto al pane activo, sin volver a elegir el agente. Add project folder entra directamente en la vista de carpetas para guardar un proyecto.
- Todos los atajos son editables en Settings › Layout & Shortcuts (ver abajo).

### Movimiento tipo compositor (resortes)
- `springTransition(property)` en `theme.ts` (`560ms`, `easing: "spring"`) es el único resorte de layout: ancho del sidebar (rail `52px` ↔ `224px`, contenedor recortado que revela el contenido) y `left`/`width`/`height` de columnas y panes del tiling. El área del tiling usa el ancho **destino** (ventana − sidebar − margen), así columnas y sidebar se asientan juntos en el mismo resorte.
- Animaciones de niri que no salen del layout: `window-open` (fade + escala `0.92 → 1`, `150ms` `easeSmooth`) en columnas y panes nuevos; `window-close` (la ventana real, vía `paint.exiting`, se desvanece y escala a `0.94`); `workspace-switch` (desplazamiento vertical de un alto completo del área en 240 ms, con el contenido real de ambos workspaces). Las esquinas de la card animan `radius` con el mismo resorte al entrar o salir de fullscreen. Tokens en `tilingMotion`.
- Terminal sin su primer frame (sesión recién creada o pane recién montado): en lugar de texto, un *skeleton* de siete líneas de `8px` (anchos fijos entre el `28%` y el `84%` de la grilla, color `kbdBg`, radio `xs`) que pulsa con `skeletonMotion` y se desvanece al llegar el frame. Una sesión terminada muestra «Terminal exited» con **New session**.
- Terminal desplazado al historial: barra inferior con "N lines up in the history" y **Jump to latest**; aparece con un fade-up y se va al volver al final.

### Tooltip: flecha según la posición resuelta
- `HintTooltip` usa el lado resuelto por `Popover` para orientar la flecha y `anchorOffset` para centrarla sobre el ancla, con margen para los radios de las esquinas. La base de la flecha se superpone `1px` al borde de la burbuja y comparte su color y transición.

### Menú de dos líneas (Accordee "Quick chat with")
- `MenuItem` con `description`: ícono `22px`, nombre `14px` primario + rol `13px` muted, `padding: 7px 10px 7px 8px`. El menú `New session ▾` lista cada CLI con su descripción, luego `Plain shell` y `Manage agents` (abre Settings → CLIs). El segmento chevron del split button usa `--ac-sidebar-active` en hover/abierto.

### Títulos e indicadores de estado
- Los CLIs ponen un spinner al inicio de su título OSC mientras trabajan (Codex braille `⠋⠙…`, Claude Code `·✢✶✻✽`; `✳` es Claude en reposo). icy lo quita del texto (`cleanTitle`) y lo convierte en estado (`sessionWorking`).
- **Sidebar:** el indicador de actividad cambia de fotograma cada `70ms` mientras alguna sesión trabaja. El tooltip de cada fila muestra el título completo y el estado.
- **Títulos largos:** `FadeText` mantiene el texto fijo, en una línea, y desvanece los últimos `16px` del espacio disponible solo si el título desborda. Se usa en pestañas, encabezados y filas de sesiones.

### Proyectos y selector de carpetas
- `Projects` tiene un botón `folder-plus` para agregar una carpeta. Los proyectos fijados en `config.projects` quedan a la vista incluso cuando están vacíos. Debajo aparecen las carpetas de las sesiones abiertas.
- La fila de proyecto muestra `folder-open` y el nombre. Al pasar el puntero aparecen las acciones para copiar la ruta, mostrar la carpeta, fijar o quitar el proyecto e iniciar una sesión.
- **Agents:** por defecto agrupa por prioridad: `Needs you`, `Done`, `Working`, `Idle` y `Exited`. El menú permite ordenar por proyecto y filtrar por actividad. Los proyectos fijados siguen accesibles cuando se ordena por prioridad.
- La vista de carpetas del launcher reúne el campo de ruta, los accesos a la carpeta superior e inicio y el árbol de subcarpetas. En ventanas anchas muestra también lugares, marcadores y proyectos recientes; en ventanas angostas se pueden buscar proyectos desde el campo. `↑`/`↓` seleccionan, `Tab` navega a la carpeta marcada y `Enter` confirma. El botón del pie dice qué se va a abrir o guardar. `Esc` vuelve a los resultados con la búsqueda intacta, o cierra si se entró directamente a elegir carpeta.
- Fila de sesión: `30px` con `4px` de separación vertical, icono del CLI, nombre y título en una línea e indicador de estado. El botón `⋯` aparece al pasar el puntero o en la sesión enfocada. También se abre el menú con clic derecho, incluido en el rail. `Alt+1…9` enfoca la sesión por orden de lista y los iconos conservan su lugar. No se interceptan esas combinaciones en campos de texto ni cuando un diálogo tiene prioridad.

### Uso por CLI (fila inferior del sidebar)
- Logo `18px` con un anillo `12px` en la esquina superior derecha (pista `kbdBg`, glifos `uring-*` en pasos de 5%), separado por una hairline superior. El anillo aparece **solo si el CLI informa un límite real** y muestra el más usado; es verde, ámbar desde `75%` y rojo desde `90%`. Nada se estima.
- Tooltip rico (`HintTooltip` con `body`): nombre y `% used`, y debajo una sección por grupo separada por una hairline (`Limits`, un grupo de modelos de Antigravity, `Last 7 days`). Cada límite muestra su valor, una barra y "Resets in…"; los modelos muestran tokens de entrada/salida y las sesiones.
- Fuentes (`icy-core::usage::cli` y `remote`): **Codex** lee `rate_limits` de sus transcripts (ventanas de 5 h y semanal por `limit_id`, créditos). **Claude Code** consulta `GET api.anthropic.com/api/oauth/usage` con el token OAuth del CLI y cae a los registros locales de `usage_limit_reached`. **Antigravity** ejecuta `agy -p /usage --output-format json`, que no consume turnos. **Grok** consulta su endpoint de créditos (sin verificar). Los tokens nunca se refrescan ni pasan por argv. Los datos locales se refrescan cada 2 min y los remotos cada 10 min.

### Imágenes en la terminal y visor (`app/terminal.tsx`, `app/imageviewer.tsx`)
- Las imágenes Kitty se dibujan sobre la grilla en su caja `c×r` celdas (celda `0.6em × 1.4em` de Geist Mono, origen en el padding de filas), manteniendo proporción; hover `brightness 1.06`.
- Click → visor modal (`position: fixed`, capa `2200`): barra flotante centrada (`Image n of m`, anterior/siguiente, zoom − / % / +, ajustar, cerrar) con tooltips y atajos; imagen centrada con sombra de modal, tamaño y posición con el resorte de layout; tira de miniaturas `44px` con la activa en `--ac-brand`.
- Mientras está abierto, el host desvía teclado, rueda y pinch del touchpad al visor (`MODAL_CAPTURE`): `←/→` paso (o paneo con zoom), `↑/↓` paneo, `+ − 0`, `Home/End`, `Esc`.

### Rutas de imagen en la salida de cualquier CLI
- Una ruta resuelta a `png/jpg/jpeg/gif/webp/svg` se dibuja como etiqueta (span con fondo `brandSoft`, radio `4px`, texto `brandText`, sin subrayado) en lugar de link.
- Hover sobre la ruta (`220ms`): card flotante sobre la fila (lado `top`, da vuelta si no entra) con la miniatura encajada en `280×180`, nombre, `ancho×alto` en mono y "Click to open in the viewer"; estado "Loading preview…" mientras el host decodifica.
- Click: abre el visor de imágenes con todas las imágenes de esa pantalla como stepper (nombre del archivo en la barra). El host resuelve la ruta contra el cwd de la sesión, acepta solo esas extensiones y archivos regulares ≤ 32 MiB, y decodifica en un hilo (PNG/JPEG/GIF/WebP vía resvg, SVG nativo; tamaño por cabecera en `icy-core::graphics::image_dimensions`).

### Previsualizador de código y diffs en la salida de los CLI
- Hover sobre el enlace a un archivo resuelto o a una edición/diff del CLI (`220ms`): card flotante (`480px`, misma superficie Accordee que la vista previa de imágenes, `codePreviewLayout` en `theme.ts`) con un recorte de hasta `14` líneas en Geist Mono `11.5px` con resaltado de sintaxis (`icy-core::syntax`).
- Para archivos enfoca la ventana alrededor de `:línea` (con 3 líneas de contexto y marca de acento en la fila objetivo); para diffs enfoca el cambio principal con fondos `diffAddBg`/`diffDelBg`, barra lateral de `3px` y estadísticas `+N −M`.
- Click: cierra la vista previa y abre directamente el panel completo en el tiling (`TilePane` / `DocView`) con virtualización, contexto plegado y acciones completas (`View file`, `View full diff`, `Pin pane`, `Reload`, `Open in…`, fullscreen).

### Navegador (`app/browser.tsx`)
Un tile de navegador es una ventana más del tiling. Su barra de direcciones toma las medidas de `browserLayout` en `app/theme.ts` (altura de barra y de campo, padding, gap y radio `md`). El campo se enmarca con un hairline `palette().border` y no lleva fondo; el texto usa `textPrimary`, los errores `textMuted`, y los botones son `IconButton sm`. La página ocupa el cuerpo del tile como una imagen, sin marco propio.

- **Browser** usa el glifo `globe` en el sidebar expandido, el rail y la paleta. Abre un pane independiente con la barra de direcciones enfocada. La pestaña muestra el título de la página o su dirección.
- Los enlaces HTTP y HTTPS de la terminal muestran una tarjeta con la dirección al pasar el cursor. Las direcciones locales muestran además el frame de la página, escalado desde el tamaño definido en `urlPreviewLayout`. Se puede mover el cursor desde el enlace a esa tarjeta y hacer clic en cualquiera de los dos para abrir la dirección en un pane.

### Ventanas del tiling (`app/panel.tsx`, `TilePane`)
Files, Review, Overview, Browser, cada archivo y cada diff son ventanas del mismo tiling que las terminales. Comparten card, foco, resize por arrastre, Grid/Focus/Overview y atajos de layout.
- **Una sola fila de pestañas:** una pestaña por ventana en orden de lectura del tiling (columnas de izquierda a derecha, panes de arriba abajo). Terminales con el ícono del CLI; el resto con su glifo (`folder`, `file-diff`, `article`, `file-code`). `Ctrl+Tab` recorre todas.
- **Card de ventana:** igual que la de terminal (radio `10px`, `appBg`, `ring` + sombra al enfocar). Header `30px` con glifo `14px`, título con fade al desbordar y `+N −M` en diffs y Review; las acciones (`View diff/View file`, `Pin pane`, `Reload`, `Open in…`, `Fullscreen pane`, `Close`) se montan al hover o mientras el menú `Open in…` está abierto, para que una columna angosta conserve el título. Cuando un archivo o diff está fijado (`Pin pane`), muestra un indicador `pin` en `brandText` en reposo. La ruta completa del archivo enfocado va en el header de la página.
- **Abrir cosas:** Files / Review / Overview son ventanas únicas (se enfocan si ya existen; el botón o atajo cierra la que tiene foco). Los archivos y diffs sin fijar reutilizan la ventana de vista previa del proyecto; al fijar un pane (`Pin pane`), el siguiente archivo o diff se abre en una nueva columna al lado sin pisarlo. Los fragmentos del CLI tienen su propia vista previa, independiente de Review.
- **Abrir en IDE externo (`Open in…`):** menú Accordee (`MenuPanel`) presente en el header de archivos/diffs (abre en `:línea:columna` o en el primer cambio del diff) y en el menú `⋮` de cada proyecto en el sidebar (abre la carpeta). Lista los IDEs detectados por el SO (`icy-core::platform::detect_editors`) con sus íconos, marca y prioriza el último usado (`preferredEditor` en `config.json`), e incluye `System default`, `Show in file manager` y `Copy path`.
- **Abrir ventanas:** paleta del header (`+`, `Ctrl+Shift+P`), atajos de Overview (`Ctrl+Shift+G`) y Files (`Ctrl+Shift+E`) o accesos a Files y Review en el sidebar.
- **Mover ventanas arrastrando (niri interactive move / Hyprland `movewindow`):** se arrastra el header de la card (cursor `grab`) o su pestaña; pasados `6px` empieza el arrastre. Un fantasma `240×34` (glifo/ícono + título, sombra `popover`) sigue al puntero y la ventana original queda al `35%`. El destino se marca con el *insert hint* de niri: rectángulo `brandSoft` con borde `2px` de acento, radio `10px`, que se mueve con el resorte de layout. Cerca del borde de una columna (`22%` de su ancho, máx. `90px`) o sobre la fila de pestañas → franja de `56px` entre columnas (columna nueva ahí); en el centro de una columna → la mitad superior o inferior del pane bajo el puntero (se apila arriba o abajo, con alto repartido en partes iguales). Al soltar se aplica `icy-core::tiling::move_pane`; soltar sobre su propio lugar no hace nada. Deshabilitado en Focus (pantalla completa) y con una sola ventana.
- **Rendimiento.** Código, diff, árbol y Review usan `LazyRows` para montar solo las filas cercanas al viewport (±`600px`). El árbol y Review calculan los desplazamientos con la altura fija de sus filas. El código y el diff usan un índice de alturas estimadas según el ancho y conservan la longitud calculada de cada línea entre redimensionamientos. El rango visible se reconcilia al cruzar una fila. Los archivos y diffs se pueden recorrer completos, incluidos los cambios después de la línea 20 000.
- **Enlaces desde la terminal.** Una mención abre el archivo completo y respeta el número de línea cuando aparece. Un encabezado de edición abre **single edit**, con las líneas de la vista previa del CLI y sus propios conteos. Si solo hay un resumen, busca un único bloque contiguo que coincida con sus conteos y su línea, cuando se informa. Si no puede aislarlo, muestra «Edit unavailable». Review abre el diff acumulado del archivo.

### Launcher (`app/palette.tsx`, `app/launcher-folders.tsx`)
- El `+` del header y su atajo editable (`Ctrl+Shift+P` por defecto) abren el mismo launcher, con la misma vista, foco y comportamiento. Archivos, URLs, herramientas y sesiones abiertas se abren o enfocan directamente. Un agente o Terminal conserva la elección y pide solo confirmar la carpeta dentro del mismo modal.
- Search sessions abre el mismo modal desde el sidebar, incluido el rail, o con `Ctrl+Shift+K` (editable). Busca por título, CLI y carpeta en todos los workspaces. Al elegir una sesión se activa su workspace.
- Ancho de hasta `700px` con el chrome flotante: la barra de búsqueda (`floatingLayout.searchHeight`, Inter `16px`) queda siempre a la misma altura y el panel de resultados crece hacia abajo hasta lo que entra en la ventana, sin espacio vacío bajo una lista corta (un estado vacío ocupa una línea). Los atajos (`↑↓ Navigate`, `↵ Open`, `Esc Close`) van centrados al pie de la ventana, fuera del panel. La vista de carpetas tiene alto fijo y su barra de confirmación sigue dentro de su panel. Sin búsqueda, las sesiones vivas más nuevas aparecen en una grilla adaptable de hasta tres columnas. Debajo, las secciones de sesiones nuevas y acciones usan títulos con un divisor fino y filas con ícono, título y detalle cuando aporta. Se conserva la separación de `4px` entre filas y al lado del carril de scroll. `↑`/`↓` recorren los resultados en orden de lectura, `Enter` abre y un clic fuera cierra el launcher completo.

### Contenido de las ventanas (Antigravity, extraído del bundle: `PanelItemRow`, `FileTreeNode`)
- **Overview:** filas `PanelItemRow` (`rounded-lg`, ícono `16px` al `70%`, hover `bg-secondary`): `Review +adds -dels`, `Files`, `Terminal`. Secciones con label muted + badge pill + chevron: Sessions, Modified files, Detected in terminal.
- **Files.** Filas de `26px` con radio de `6px`, guías de indentación de `10px` visibles al hover, ícono `folder`/`folder-open`/`file` y punto VCS de `8px` (`M` ámbar, `A` esmeralda, `U` teal, `D` rosa). Las carpetas se cargan bajo demanda en segundo plano y conservan sus filas durante un refresh.
- **Review:** barra fija con `Working tree`, proyecto, cantidad de archivos, `+N −M` y Refresh con tooltip (deshabilitado mientras hay una consulta en curso). Lista virtualizada de archivos cambiados (`icy-core::git`): nombre Inter `13px`, carpeta muted `11px`, estadísticas Geist Mono `12px` y punto VCS. Filas de `52px` / `40px` compactas, radio `6px`, inset `8px` y lavado `rowHoverBackground()`; click abre el diff. Durante un Refresh se conserva la lista anterior y se anuncia la carga en la barra. El estado vacío distingue la primera carga de un working tree sin cambios. Medidas en `reviewLayout` de `theme.ts`.
- **Archivo:** Geist Mono `12.5px` / `1.5`, gutter de `48px`, resaltado por tokens (`icy-core::syntax`), mismo fondo que la terminal.
- **Diff de contexto completo plegado:** las corridas sin cambios dejan 3 líneas alrededor de cada cambio; el pliegue muestra `+N more lines` y dos `+10`. Líneas agregadas/borradas con fondo tenue y barra de `3px` en el gutter. Scrollbar fina auto-oculta (`8px`, `--ring`).

### Settings (`app/settings.tsx`)

- La navegación de `212px` incluye un enlace al workspace, búsqueda y grupos de ajustes. En ventanas angostas pasa a `168px` y los controles de cada fila se apilan cuando hace falta.
- El contenido ocupa una columna centrada de hasta `700px`. Los grupos usan divisores finos, controles alineados y solo las descripciones que aclaran una acción.
- Appearance muestra los temas en filas con muestras de color y una marca de selección. Accent, density y el tamaño de fuente se ajustan en el mismo bloque.
- Window ofrece un control de transparencia de 0 a 100 % en pasos de 5. Cuando el valor supera 0 %, el fondo base usa `sidebarBg` con el alfa elegido y se comparte entre el sidebar, el workspace y las terminales. Las tarjetas, los controles y el contenido de cada pane conservan sus colores. Las medidas del control están en `windowAppearance`.
- CLIs & Agents incluye Desktop notifications, con interruptores independientes para fin de tarea, solicitudes de aprobación o entrada y filtro por foco. Send test envía un aviso de prueba y muestra el resultado en la misma fila.

### Capturas locales (gitignored en `design/reference/accordee/captures/`)
Se guardaron las capturas de referencia en Light y Dark para Ask Rune, Themes, Search Modal, Quick Chat, Dashboard, Inbox, Tasks (Cards y List), Invoices y Settings (`General` y `Preferences`). Para volver a generar la referencia en local, ejecutar `bash design/fetch-reference.sh`.

## Métricas de agentes

La vista usa Inter en el encabezado, los controles, los ejes y las tablas. Las tarjetas de indicadores siguen el patrón Accordee de 144 px, radio de 8 px y borde hairline. Las gráficas y los desgloses pasan a una columna cuando el área disponible se reduce, y el contenido se limita a `1360px` en ventanas anchas. Las filas respetan la densidad seleccionada en Appearance.

Al hacer scroll, el contenido se funde con el fondo del encabezado fijo mediante un degradado de hasta `24px` en el borde superior.

Las gráficas siguen el modelo de Apache ECharts dentro de JSNIR. Tienen ejes con escala "nice" (pasos 1 / 2 / 2,5 / 5 × 10ⁿ), cuadrícula, una banda de *axis pointer* detrás del punto señalado y una etiqueta del eje que la acompaña. Al señalar un punto, el resto de las barras se atenúa. Cada gráfica interactiva reutiliza un tooltip con fondo del tema, ligeramente teñido por el acento. Sigue al puntero a `20px` cuando hay espacio: elige el lado al entrar y se mantiene dentro del panel sin saltar al lado opuesto. Los huecos del mapa de actividad cuentan como parte de las celdas y la salida del hover se demora `90ms` para mantener el tooltip visible al pasar entre elementos. La leyenda de Token volume oculta o muestra cada serie, y la serie visible ocupa toda la altura. Si la primera carga supera `120ms`, aparece un *skeleton* estático en `track` que comparte la grilla de tarjetas y las alturas de la gráfica y la tabla de agentes. Al recibir los datos, el contenido aparece en el ancho medido del pane. Un Refresh atenúa el último escaneo hasta que llega el nuevo. Las filas de las tablas usan el lavado de hover `rowHoverBackground()` (`textPrimary` al 5 %). La fila del agente filtrado usa `brandSoft`.

Los gráficos heredan la paleta activa. Para personalizarlos, se puede agregar un objeto `charts` dentro de `colors` en cualquier JSON de Appearance → Custom JSNIR Themes. Este fragmento va dentro de `colors`.

```json
{
  "charts": {
    "input": "#a3a3a3",
    "output": "#60a5fa",
    "sessions": "#a78bfa",
    "duration": "#2dd4bf",
    "axis": "#a3a3a3",
    "grid": "#ffffff1a",
    "track": "#262626",
    "heatmap": ["#262626", "#16283c", "#1d3d5d", "#265378", "#3b82f6", "#60a5fa"],
    "providers": {
      "anthropic": "#d95926",
      "google": "#3987e5",
      "openai": "#199e70",
      "xai": "#c98500",
      "other": "#8a8a8a"
    }
  }
}
```

Cada valor es opcional y acepta el mismo formato hexadecimal que el resto de la paleta. `heatmap` admite seis niveles, desde sin actividad hasta el máximo. Se puede usar `null` para heredar un nivel. Sin valores explícitos, `output` y `duration` usan el acento activo, y `input` y `sessions` usan una versión suavizada. El mapa de actividad mezcla `output` con el fondo de la tarjeta y reserva `track` para el nivel cero. Los proveedores conservan tonos propios, matizados con el acento. `axis`, `grid` y `track` heredan `textMuted`, `border` y `kbdBg`. El botón **Reload** aplica los cambios al tema activo.

## Git › Graph

La entrada **Git** del sidebar (ícono `git-branch`) abre el panel con un header mínimo de pestañas de texto subrayadas por una línea fina: **Changes** (el working tree, una línea por archivo con letra de estado `M/A/U/D`, nombre, carpeta atenuada y `+/-`) y **Graph** (los últimos 500 commits de todas las ramas, con lanes de colores, badges de refs y tiempo relativo). Los commits de merge se dibujan como anillo y el resto como punto relleno. Las medidas salen de `gitGraphLayout` en `app/theme.ts`, y el host rasteriza las líneas con esos valores.

Los colores de las lanes se recorren en ciclo desde `palette().graphLanes`. Un tema puede reemplazarlos con una lista `graph` dentro de `colors`:

```json
{
  "graph": ["#7aa2f7", "#73daca", "#bb9af7", "#e0af68", "#f7768e", "#9ece6a"]
}
```

El árbol de archivos sigue el mismo ritmo: filas de una línea, sin relleno de selección (solo el lavado sutil de hover), letra de estado git en los archivos y un punto en las carpetas con cambios.

## Editor › Selección, Add to Chat y pie

El editor no tiene encabezado. Un archivo se lee con resaltado de sintaxis y números de línea; se selecciona por caracteres (arrastre desde cualquier punto de la fila, o `Ctrl+A`) y `Ctrl+C` copia la selección o, sin selección, la línea del cursor. Escribir, pegar, `Enter`, `Backspace`, `Supr` o un doble clic abren el editor nativo en ese punto con la tecla ya aplicada; `Esc` vuelve a la lectura si no hay cambios sin guardar, y también se vuelve al enfocar otra ventana. El estado (`Saving…`, `Saved`, `Unsaved changes` o por qué el archivo es de solo lectura) y el botón para descartar cambios están en el pie. La selección se dibuja con `palette().selection` (por defecto `highlight` al 35 %) y se parte por spans para respetar el resaltado de sintaxis. Una selección de una sola línea tiene radio `3px`; la de varias líneas tiene esquinas rectas y las líneas se tocan, como en VS Code. La altura de línea del código se redondea a píxeles enteros para que no quede una costura entre líneas. Un tema puede fijarla con `selection` dentro de `colors`, en `#rrggbbaa`:

```json
{
  "selection": "#4678dc6e"
}
```

- **Add to Chat (`Ctrl+L`):** al terminar un arrastre aparece un chip `Add to Chat` con su atajo bajo el extremo de la selección. En modo edición se ancla a la esquina inferior derecha del campo. Pega el contexto `archivo:línea-línea` en el CLI enfocado más reciente del proyecto; si hay varios y ninguno fue enfocado, abre el menú de sesiones (con `Copy context`) en la esquina del editor. `Ctrl+L` sigue siendo de la terminal y de la barra del navegador cuando tienen foco.
- **Pie del editor:** una línea fina bajo el código con la rama y la autoría de la línea (`Ada, 2d ago`, o `Not committed yet`) a la izquierda; a la derecha `Ln`, `Col` y `(N selected)`, el indentado (`Tab Size` o `Spaces`), la codificación (`UTF-8` / `UTF-8 with BOM`), el fin de línea (`LF` / `CRLF`) y el lenguaje. En la vista previa, `Ln` y `Col` aparecen tras el primer clic.

## Íconos personalizables por tema

Cada tema puede reemplazar cualquier ícono con un mapa `icons` al nivel raíz del JSON (junto a `colors`). La clave es el nombre del glyph (`folder`, `file`, `git-branch`, `search`, `refresh`… la lista completa está en `UI_GLYPHS` de `crates/icy-app/src/icons.rs`) o `cli:<id>` para el logo de un CLI (`cli:claude`, `cli:codex`…). El valor es una ruta a un SVG relativa al archivo del tema, o el SVG en línea.

```json
{
  "id": "mi-tema",
  "colors": { "…": "…" },
  "icons": {
    "folder": "icons/folder.svg",
    "git-branch": "icons/branch.svg",
    "cli:claude": "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'>…</svg>"
  }
}
```

Los glyphs de la UI se usan como máscara: se toma la forma del SVG y se pinta con el color del tema, así que conviene dibujarlos en un solo color. Los logos `cli:` conservan sus colores. Se ignoran los nombres desconocidos, los archivos que no existen y los SVG de más de 512 KiB. Así, un tema (colores + íconos) se comparte como una carpeta, igual que unos dotfiles. El botón **Reload** vuelve a leer los SVG.

## Sidebar: menús y sesiones

- **Accesos a ventanas:** Files, Review, Metrics y Archived no mantienen fondo activo por estar abiertos, ni en el sidebar expandido ni en el rail. Solo muestran hover real del puntero; el foco pertenece a la pestaña y a la card del tiling. Se conservan sus acciones de abrir/enfocar y los atajos existentes.
- **Menú de cuenta:** arriba va la identidad, sin interacción: avatar, usuario y `repo · rama · método`. Después van Settings, Themes (abre Appearance) y Release notes. Al final está **Refresh GitHub status**. El pie del sidebar no tiene `⋯` ni una fila Settings.
- **Release notes:** el ícono de documento con cambios identifica la acción en el menú de cuenta y en el aviso de actualización. El halo del menú sigue el puntero y se desvanece al salir de la fila. La portada a todo el ancho usa una imagen o un video de la release cuando está disponible. Mientras carga la imagen, o en releases sin medios, muestra un arco de luz de líneas finas teñido por el acento activo. La textura se genera una vez al iniciar. La versión, la fecha y las notas quedan debajo. Las acciones permanecen visibles en el pie.
- **Archived:** el estado vacío centrado muestra el glifo de archivo y explica cómo archivar desde el menú de una sesión. Los tres puntos aparecen al pasar el puntero por una fila. El botón tiene su propio hover encima del fondo de la fila. **Archive session** la oculta del sidebar y del tiling mientras conserva el proceso y su historial. Un clic en la fila archivada o **Restore session** devuelve la misma sesión al workspace. Las filas indican si sigue en ejecución o ya terminó. El menú permite quitar del archivo las sesiones que ya terminaron.
- **Títulos de sesión:** se quita el spinner que los CLIs ponen en el título OSC (braille de Codex, `·✢✶✻✽` y `◐◓◑◒` de Claude Code, `✳` en reposo). El estado reportado por el motor alimenta los indicadores y los filtros de Agents.
