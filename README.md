# icy

**icy** es un workspace e IDE **terminal-first y agent-first** construido en Rust sobre el motor nativo **JSNIR** (TSX en QuickJS + layout Taffy + renderizado GPU wgpu, sin WebView).

Combina el enfoque de múltiples terminales para agentes CLI de [`herdr`](https://herdr.dev/) con la experiencia visual de previsualización e inspección bajo demanda: cualquier archivo, línea de error o URL `localhost` que aparezca en la terminal se vuelve clickeable y abre una vista directa dentro del entorno.

## Estado actual (Fase 0)

- **Core cross-platform (`crates/icy-core`):** PTY real (`portable-pty`, ConPTY en Windows), emulación VT tras el trait `VtEngine` (`alacritty_terminal`), gestión de sesiones y detector de enlaces `links.rs` (`path:line:col`, rutas Windows `C:\...`, rutas entre comillas, prefijos `git diff`, URLs `http(s)://...`).
- **App nativa (`crates/icy-app` + `app/`):** sidebar de sesiones, viewport de terminal en `Geist Mono`, barra de accesos rápidos (`Ctrl+C`, `Esc`, `Tab`, `↑`, `↓`), composer de comandos, tira de enlaces detectados y visor de archivos con resaltado de línea.
- **Sistema de diseño:** tokens extraídos de Accordee (`design/accordee-tokens.css`, `app/theme.ts`, [DESIGN.md](DESIGN.md)).
- **Arquitectura y roadmap:** detallado en [ARCHITECTURE.md](ARCHITECTURE.md).

## Requisitos locales

1. Rust `1.99.1` (`rust-toolchain.toml`).
2. El checkout de **JSNIR** (`https://github.com/Niiihuel/jsnir.git`, rama `discord-engine`, commit fijado en [jsnir-ref.txt](jsnir-ref.txt)) ubicado como directorio hermano `../Jurui`.
3. Compilador nativo de TypeScript 7 disponible en `../Jurui/target/typescript/lib/tsc` (o `npm ci` dentro de `../Jurui`, o `JSNIR_TSC`).

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
