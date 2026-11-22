# Sistema de Diseño de icy (Patrón Accordee)

Este documento fija el lenguaje visual de **icy**, extraído de [`https://demo.accordee.app/`](https://demo.accordee.app/) y adaptado al runtime nativo **JSNIR** (`app/theme.ts`).

> **Regla obligatoria:** Cualquier componente nuevo en `app/*.tsx` debe tomar todos sus valores (colores, radios, espaciados, pesos, sombras, duraciones y curvas) desde [app/theme.ts](app/theme.ts) y este documento. No se permiten valores hexadecimales ni medidas sueltas fuera del sistema.

---

## 1. Principios visuales

1. **Superficies de alto contraste con bordes finos (`0.5px`):** sin sombras pesadas ni neón. La jerarquía entre sidebar (`#0f0f10`), superficie principal (`#0a0a0a`) y tarjetas/paneles (`#171717`) se marca con saltos sutiles de luminancia y bordes hairline (`#ffffff1a` / `#0000001a`).
2. **Layout en marco moderno (`html[data-layout=modern]`):** el fondo exterior de la ventana usa `--ac-sidebar-bg`; el panel principal (`.app-main`) se incrusta con `margin: 8px 8px 8px 0`, `border-radius: 8px`, borde `0.5px` y sombra suave `0 1px 2px #0000000d`.
3. **Tipografía contenida:** pesos `400` (`normal`) y `440` (`medium` / `semibold` / `bold`) con **Inter** para toda la UI y **Geist Mono** para terminal, código, rutas, líneas y teclas (`kbd`).
4. **Acentos neutros por defecto, color para foco y acción:** el acento `neutral` usa blanco/negro invertido para acciones primarias y reserva azul (`#3b82f6` / `#60a5fa`) para `--ac-highlight` y enlaces clickeables.

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

- **Sans UI:** `Inter` (`Jurui/assets/fonts/Inter-Regular.ttf`, `Inter-SemiBold.ttf`)
- **Monospace (terminal / visor / `kbd`):** `Geist Mono` (`assets/fonts/GeistMono-Regular.ttf`)
- **Escala:** `micro: 10`, `tiny: 11`, `caption: 12`, `label: 13`, `body: 14`, `subtitle: 15`, `title: 16`, `headingSm: 18`, `heading: 20`, `metric: 24`, `displaySm: 26`, `display: 30`.
- **Pesos:** `light: 300`, `normal: 400`, `medium: 440`, `semibold: 440`, `bold: 440`.

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
| **App Frame & Main Panel** | `.app-frame`: `background: var(--ac-sidebar-bg)`. `.app-main`: `margin: 8px 8px 8px 0`, `border: .5px solid var(--ac-border)`, `border-radius: 8px`, `box-shadow: 0 1px 2px #0000000d`. | `app/main.tsx`: `Row` con `sidebarBg` envolviendo el contenedor principal redondeado `8px`. |
| **Sidebar e ítems** | Fondo `--ac-sidebar-bg`; ítem activo `--ac-sidebar-active`; texto activo `--ac-text-primary`, inactivo `--ac-text-secondary`. | `app/sidebar.tsx`: ancho `232px`, filas `32px`, radio `6px`, `StatusDot` de `6px`. |
| **Botones** | Primario (`[data-send][data-ready=true]`): `--ac-brand` / `--ac-brand-fg`, radio `8px` (`32x32` en hero). Secundario: `--ac-panel-bg`, borde `.5px`, altura `28–32px`, radio `6px`. | `PrimaryButton` y `SecondaryButton` en `app/ui.tsx`. |
| **Composer / Input** | `.chatbox-glow` usa `--ac-card-bg`; botón send idle `--ac-send-idle`, listo `--ac-brand`. | `app/terminal.tsx`: panel `12px` radius, `--ac-panel-bg`, borde `0.5px`, `TextInput` en `Geist Mono 13px`. |
| **Kbd / Chips** | Fondo `--ac-kbd-bg`, `font-mono`, radio `--radius-chip` (`4px`). | `KbdChip` (`height: 22px`, `Geist Mono 11px`, variante acentuada con `brandSoft` + `ring`). |
| **Tabs (`.detail-tab`)** | Texto `--ac-text-muted`, hover `--ac-text-secondary`, `[aria-selected=true]` `--ac-text-primary` peso `440`, indicador inferior `1px` con escala X (`250ms ease-smooth`). | Reservado para tabs de sesiones / diffs / preview en Fase 1–2. |
| **Tablas / Filas (`.crm-table`)** | Celdas `40px` (`32px` compact, `52px` comfortable), padding `0 12px`, borde `.5px solid var(--ac-border)`, hover `--ac-panel-bg`, seleccionada `--ac-sidebar-active`. | Aplicable en lista de archivos tocados, to-dos y tabla de modelos/agentes (Fase 5). |
| **Checkbox (`.crm-check`)** | `16x16`, radio `4px`, borde `.5px`, checked `--ac-text-primary`, focus outline `2px solid #8b5cf6`. | Reservado para lista de to-dos y filtros de métricas. |
| **Scrollbars** | Ocultos por defecto en Accordee (`scrollbar-width: none`); en `icy` el scroll nativo de JSNIR se mantiene discreto sobre `appBg` / `panelBg`. | `scroll: true` en lista de sesiones, terminal y visor. |

---

## 6. Reglas de layout del IDE

- **Terminal-first:** el pane de terminal ocupa el máximo espacio flexible (`grow: 1`) con tipografía `Geist Mono 13px` / `lineHeight: 19px`.
- **Editor / Visor bajo demanda:** el visor (`480px`, `--ac-panel-bg`, borde izquierdo `0.5px`) solo se monta cuando `viewerState.value.open === true`.
- **Enlaces clickeables:** toda referencia detectada en terminal usa `brandText` + subrayado, y activa hover `brandSoft` en su fila o chip.

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
- **Sidebar (`app-sidebar`):** ancho expandido `196px` (`272px` en Settings), `padding: 8px 4px`, fondo `--ac-sidebar-bg` (`#0f0f10` dark / `#fafafa` light), sin borde derecho en `modern` layout (`border-right-width: 0`).
- **Ítem de navegación:** `height: 28px`, `border-radius: 6px`, `padding: 6px 8px 6px 6px`, `gap: 8px`, transición `300ms cubic-bezier(.16, 1, .3, 1)`. Estado `hover` y ruta activa (`active`) aplican `--ac-sidebar-active` (`#1f1f22` dark / `#f0f0f1` light).
- **Badge de conteo / `kbd`:** `16px` de alto (`16x16` en shortcuts), `border-radius: 9999px` (badge) o `4px` (`kbd`), fondo `--ac-kbd-bg` (`#262626` dark / `#e5e7eb` light), `10px`, peso `440`.

### Botones
- **Primary (`+ Add Card`, `+ New invoice`, `Done`):** fondo `--ac-brand`, texto `--ac-brand-fg`, `height: 28px` / `30px`, `border-radius: 6px`, `padding: 0 12px` (`0 12px 0 10px` con ícono), `gap: 6px`, `13px`, peso `440`, `box-shadow: 0 1px 2px #0000000d`. `hover` → `opacity: 0.9`; `disabled` → `opacity: 0.4`.
- **Secondary (`Edit Cards`, `See plans`, chips de prompt):** fondo `--ac-app-bg` / `--ac-card-bg`, texto `--ac-text-secondary` o `--ac-text-primary`, `border: 1px solid var(--ac-border)`, `height: 28px / 30px / 32px`, `border-radius: 6px`, `padding: 0 10px` / `0 12px`, `box-shadow: 0 1px 2px #0000000d`. `hover` → `--ac-panel-bg`.
- **Ghost (`Share`, `Filter`, `Quick Chat`, `Reset`):** `background: transparent`, `height: 28px / 30px`, `border-radius: 6px`, `padding: 4px 8px 4px 6px`, `gap: 6px`. `hover` / `data-state=open` → fondo `--ac-panel-bg` + texto `--ac-text-primary`.
- **Icon Button (`Close`, `Send`):** `32x32` (o `28x28`), `border-radius: 6px`. Botón `Send` usa `--ac-send-idle` (`#3f3f46` dark / `#8e8e93` light) en reposo y `--ac-brand` / `--ac-brand-fg` cuando está listo.

### Inputs, Selects y Composer
- **Input estándar / Wrapper de búsqueda:** `height: 32px`, `padding: 0 10px` (`0 8px` con `gap: 8px` en búsqueda), `border: 1px solid var(--ac-border)`, `border-radius: 6px`, fondo `--ac-app-bg`, `14px`. `focus` / `focus-within` → borde `--ac-text-muted`.
- **Select / Combobox:** `height: 36px`, `padding: 8px 12px`, `border-radius: 6px`, `box-shadow: 0 1px 2px #0000000d`. `focus-visible` → borde `--ring` + halo `0 0 0 3px color-mix(in oklab, var(--ring) 50%, transparent)`.
- **Composer (`Ask Rune`):** contenedor `--ac-card-bg`, `border: 1px solid var(--ac-border)`, `border-radius: 18px`, `padding: 10px`, `gap: 12px`, `min-height: 106px`, sombra doble (`0 1px 2px, 0 6px 20px -6px`) que crece en `:focus` (`0 14px 34px -10px`). Textarea `14px` / `line-height: 22px`.

### Cards, Tablas, Tabs, Dropdowns, Modales, Tooltips y Controles
- **Metric Card:** `--ac-card-bg`, `border: 1px solid var(--ac-border)`, `border-radius: 8px`, `padding: 12px 16px`, `height: 144px`. `hover` → borde `color-mix(in oklab, var(--ac-text-muted) 40%, transparent)`.
- **Task / Rich Card:** `border-radius: 12px`, `padding: 16px`, `gap: 14px`, `box-shadow: 0 1px 2px #0000000d`; `hover` → `shadow-sm` + borde atenuado.
- **Tablas (`.crm-table`):** `border-bottom: 0.5px solid var(--ac-border)`, `border-right: 0.5px solid var(--ac-border)`, filas `40px` (`32px` compact), `tfoot 44px` (`36px` compact), `padding: 0 12px`. `hover` → `--ac-panel-bg`; `selected` → `--ac-sidebar-active`.
- **Tabs de filtro & Segmented Control:** filtros `height: 28px`, `border-radius: 6px`, `padding: 4px 10px`, seleccionado con `--ac-panel-bg` + `border: 1px solid var(--ac-border)`. Segmented control con contenedor `height: 28px`, `padding: 2px`, ítems `23px`, `12px`, `border-radius: 4px–5px`.
- **Dropdown Menu:** `border-radius: 10px`, `padding: 6px`, `border: 1px solid var(--ac-border)`, `shadow-xl`. Ítems `height: 32px`, `padding: 0 8px`, `gap: 10px`, `13px`, `hover/focus-visible` → `--ac-panel-bg` + `--ac-text-primary`.
- **Modales (`Themes`, `Search`):** `border-radius: 16px`, `border: 1px solid var(--ac-border)`, anchos `480px` / `700px`, sombra profunda `0 24px 80px -16px rgba(0,0,0,0.35)` / `0 32px 100px -24px rgba(0,0,0,0.35)`.
- **Tooltips:** fondo invertido `--foreground` (`#fafafa` dark / `#09090b` light), texto `--background`, `border-radius: 6px`, `padding: 6px 8px`, `12px` / `line-height: 16px`.
- **Switch & Checkbox:** switch `32x18px`, pill `9999px` (`--ac-kbd-bg` off → `--ac-brand` on); checkbox `16x16px`, radio `4px–5px`, `focus-visible` con `outline: 2px solid #8b5cf6; outline-offset: 2px`.
- **Avatars:** `18x18px` circular (`9999px`) con borde `1.5px solid var(--ac-app-bg)`.

### Capturas locales (gitignored en `design/reference/accordee/captures/`)
Se guardaron las capturas de referencia en Light y Dark para Ask Rune, Themes, Search Modal, Quick Chat, Dashboard, Inbox, Tasks (Cards y List), Invoices y Settings (`General` y `Preferences`). Para volver a generar la referencia en local, ejecutar `bash design/fetch-reference.sh`.

