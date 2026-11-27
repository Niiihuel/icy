# icy

**icy** es un workspace e IDE **terminal-first y agent-first** construido en Rust sobre el motor nativo **JSNIR** (TSX en QuickJS + layout Taffy + renderizado GPU wgpu, sin WebView).

Combina el enfoque de múltiples terminales para agentes CLI de [`herdr`](https://herdr.dev/) con vistas previas para inspeccionar archivos, líneas de error y enlaces de la terminal dentro del mismo entorno.

## Estado actual (Fase 0)

- **Core cross-platform (`crates/icy-core`):** PTY real (`portable-pty`, ConPTY en Windows), emulación VT tras el trait `VtEngine` (`alacritty_terminal`), gestión de sesiones y detector de enlaces `links.rs` (`path:line:col`, rutas Windows `C:\...`, rutas entre comillas, prefijos `git diff`, URLs `http(s)://...`).
- **App nativa (`crates/icy-app` + `app/`):** sidebar de sesiones, viewport de terminal en `Geist Mono`, barra de accesos rápidos (`Ctrl+C`, `Esc`, `Tab`, `↑`, `↓`), composer de comandos, tira de enlaces detectados y visor de archivos con resaltado de línea.
- **Sistema de diseño:** tokens extraídos de Accordee (`design/accordee-tokens.css`, `app/theme.ts`, [DESIGN.md](DESIGN.md)).
- **Arquitectura y roadmap:** detallado en [ARCHITECTURE.md](ARCHITECTURE.md).

### Continuar sesiones al reabrir icy

Podés cerrar la ventana mientras un agente, una shell o un comando sigue trabajando. Cuando vuelvas a abrir icy, vas a encontrar las mismas terminales, con su historial y su distribución en workspaces. El botón de cerrar de cada terminal sí termina esa sesión.

El servidor de sesión es local y conserva las terminales mientras sigue en ejecución. `icy --stop-session-server` lo detiene y cierra sus terminales. Si reiniciás el equipo o se detiene el servidor, la recuperación de conversaciones depende de cada CLI.

### Pi

Instalá Pi y dejá `pi` en el `PATH`. Podés iniciarlo desde **Settings → CLIs & Agents → Pi** o escribiendo `pi` en una terminal de icy. Pi conserva sus modelos, extensiones y sesiones. Dentro de Pi, usá `/login` para conectar un proveedor y `/resume` para retomar una conversación. [Instalación y uso de Pi](https://github.com/earendil-works/pi/tree/main/packages/coding-agent#getting-started)

Las versiones de Pi con OSC 7501 informan cuándo empieza o termina una tarea y cuándo espera una respuesta. **Metrics** muestra los tokens y las sesiones del historial local de Pi. [Estados de terminal de Pi](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/terminal-setup.md#program-status)

### Notificaciones de agentes

En **Settings → CLIs & Agents → Desktop notifications** podés activar **Task completion** y **Approvals and questions** por separado. Con **Only for background sessions**, icy avisa por otros panes o cuando su ventana pierde el foco. Los avisos incluyen la CLI y la carpeta del proyecto y se entregan mientras icy está abierto, incluso minimizado. Usá **Send test** para comprobar la entrega.

Los avisos se basan en el estado que reporta cada CLI. Las versiones que dependen de detección visual pueden variar con el idioma o la interfaz de la CLI. Linux usa el servicio de notificaciones del escritorio por D-Bus. En Windows, la primera entrega registra un acceso directo de icy en el menú Inicio para identificar el aviso.

### Transparencia de la ventana

En **Settings → Appearance → Window**, ajustá **Background transparency** entre 0 % (sólido, por defecto) y 100 %. El cambio se aplica en el momento y se guarda en `config.json` como `windowTransparency`. El ajuste requiere una superficie Wayland con alfa. Los fondos propios de las páginas web y las celdas con color explícito conservan su opacidad.

icy solicita el desenfoque automáticamente a Niri 26.04 o posterior mediante `ext-background-effect`. También podés controlarlo con una regla en `config.kdl`. [Efectos de ventana de Niri](https://niri-wm.github.io/niri/Window-Effects.html)

```kdl
window-rule {
    match title="^icy$"
    draw-border-with-background false
    background-effect {
        blur true
        xray true
    }
}
```

### Editor integrado

- Abre un archivo desde **Files**. El archivo se lee con resaltado y números de línea. Arrastrá para seleccionar, `Ctrl+A` selecciona todo y `Ctrl+C` copia (sin selección, copia la línea). Para editar, escribí, pegá o hacé doble clic: el editor nativo de JSNIR se abre con el cursor en ese punto. `Ctrl+S` (`Cmd+S` en macOS) guarda y `Esc` vuelve a la lectura si no hay cambios sin guardar.
- Con una selección aparece **Add to Chat** (`Ctrl+L`): pega la ruta, el rango de líneas y el fragmento en el CLI del proyecto enfocado más recientemente, sin pulsar Enter. Si hay varios y ninguno fue enfocado, abre la lista de sesiones, que también ofrece **Copy context**.
- Los `.ttf` y `.otf` tienen vista previa de solo lectura sin instalar la fuente. Los binarios, los archivos demasiado grandes y los textos que JSNIR no puede editar sin modificar su formato se pueden abrir con **Open in…**.

## Requisitos locales

1. Rust `1.99.1` (`rust-toolchain.toml`).
2. El checkout de **JSNIR** (`https://github.com/Niiihuel/jsnir.git`, commit fijado en [jsnir-ref.txt](jsnir-ref.txt)) ubicado como directorio hermano `../Jurui`.
3. Compilador nativo de TypeScript 7 disponible en `../Jurui/target/typescript/lib/tsc` (o `npm ci` dentro de `../Jurui`, o `JSNIR_TSC`).

## Navegador integrado (CEF)

**Browser** en el sidebar o en la paleta abre un pane con la barra de direcciones enfocada. Los enlaces HTTP y HTTPS de la terminal también abren un pane. Al pasar el cursor, los enlaces locales muestran una vista en vivo y los públicos muestran su dirección. Un clic en la vista previa abre esa URL en el tiling.

El navegador de los tiles usa CEF 147 en modo offscreen, así que el desarrollo necesita su runtime. Se exporta una vez con `export-cef-dir`:

```bash
# Linux
cargo install export-cef-dir --version 147.1.0+147.0.10 --locked
export-cef-dir --force ~/.local/share/cef/147.0.10/cef_linux_x86_64
export CEF_PATH=~/.local/share/cef/147.0.10/cef_linux_x86_64
export LD_LIBRARY_PATH=$CEF_PATH   # libcef.so en las pruebas y el binario
```

```bat
:: Windows (PowerShell o cmd)
cargo install export-cef-dir --version 147.1.0+147.0.10 --locked
export-cef-dir --force %USERPROFILE%\.local\share\cef\147.0.10\cef_windows_x86_64
setx CEF_PATH %USERPROFILE%\.local\share\cef\147.0.10\cef_windows_x86_64
:: Añade %CEF_PATH% al PATH del usuario para que libcef.dll se encuentre al ejecutar.
```

Sin `CEF_PATH`, icy busca el runtime junto al ejecutable. Si no lo encuentra, la tile del navegador muestra «Browser runtime not found». El build también necesita `CEF_PATH` apuntando al directorio exportado: si no existe, `cef-dll-sys` lo descarga.

Una release incluye los archivos de CEF junto al binario (`libcef.so` o `libcef.dll`, `icudtl.dat`, `*.pak`, `locales/`…), así que quien usa icy no tiene que instalar nada.

## Comandos principales

```bash
# Tests (core + app)
cargo test --workspace --locked

# Formato y clippy estricto
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings

# Compilar y ejecutar en release
cargo build --release --locked -p icy-app
./target/release/icy

# Captura offscreen para smoke test
./target/release/icy --offscreen /tmp/icy.ppm
```

## Referencia visual (opcional)

Los activos crudos descargados para referencia no se versionan en este repositorio público (`design/reference/` está ignorado en `.gitignore`). Para descargar nuevamente HTML/CSS de referencia en local:

```bash
bash design/fetch-reference.sh
```

## Licencia

icy se distribuye bajo **GPL-3.0-or-later** (ver `LICENSE`). Parte del tiling y de las
animaciones está adaptada de [niri](https://github.com/niri-wm/niri) (GPL-3.0-or-later,
© sus autores); el commit de referencia está en `niri-ref.txt` y cada archivo adaptado lo indica
en su cabecera.
