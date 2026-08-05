# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Proyecto

Ruuutu ("pantalla" en finés) es una utilidad de captura de pantalla estilo Lightshot para Windows,
escrita íntegramente en Rust sin GPU ni frameworks pesados: renderizado por software con `softbuffer`
sobre un buffer `Vec<u32>` (0xRRGGBB por píxel).

Pilares del proyecto (ver `AGENTS.md`): bajo consumo de RAM (<10 MB en reposo), rendimiento en equipos
modestos, código mantenible mayoritariamente en Rust, y open source.

**Idioma**: los textos de UI, menús de bandeja y mensajes al usuario están en español. La documentación
del código y los logs `[DEBUG]` están en inglés. Mantén esa división.

## Comandos

```bash
cargo build --release --bin ruuutu     # binario optimizado (~1.4 MB) en target/release/
cargo run                              # modo servicio: bandeja + hotkeys globales
cargo run -- --capture                 # modo one-shot CLI: abre el overlay y sale
cargo run -- --debug                   # HUD de telemetría + logs [DEBUG] (también -d)
cargo check                            # comprobación rápida de compilación
cargo clippy --all-targets
```

Los tests automatizados cubren solo la lógica pura que no necesita escritorio (de momento, la
persistencia de `config.rs`). El resto de la suite es un binario interactivo:

```bash
cargo test --bin ruuutu                # tests unitarios, sin sesión gráfica
cargo run --bin test_bench             # suites: captura, Rect, clipboard roundtrip, fuente, storage, overlay simulado
```

`test_bench` incluye módulos vía `#[path = "../capture.rs"] mod capture;` en lugar de depender del crate
`ruuutu`; si añades un módulo nuevo que necesite, hay que declararlo igual allí. Requiere sesión de
escritorio real (toca el portapapeles y captura la pantalla).

Binarios auxiliares de diseño visual, todos con el mismo patrón `#[path]`:

```bash
cargo run --bin font_preview    # muestrario de tamaños/pesos de Consolas Bold
cargo run --bin margin_preview  # previsualización estática del layout de botones
cargo run --bin margin_editor   # editor interactivo de paddings/animación de los botones del overlay
cargo run --bin tray_editor     # maqueta del menú de bandeja (Windows no permite estilarlo de verdad)
cargo run --bin clipboard_test  # comprobación mínima de arboard
```

## Arquitectura

Un único event loop de `winit` conduce todo. `RuuutuApp` en `src/main.rs` implementa
`ApplicationHandler` y es la máquina de estados completa:

- `resumed` — inicializa `HotkeyManager` y `SystemTray` (o lanza el overlay directamente en modo `--capture`).
- `about_to_wait` — drena los canales de `TrayIconEvent`, `GlobalHotKeyEvent` y `MenuEvent`, procesa
  `pending_action`, y **rearma `ControlFlow::WaitUntil(+15 ms)`**. No era `Poll`: esos tres canales no
  son eventos de winit, así que el bucle tiene que volver periódicamente a vaciarlos, pero con `Poll`
  giraba al 100 % de CPU en reposo. 15 ms mantiene la latencia del atajo imperceptible.
  **Aquí ya no se repinta el overlay**: `redraw()` llama a `request_redraw()` mientras quede animación
  o retardo de hover pendiente, así que los frames llegan como `RedrawRequested` y se paran solos.
  Pintar en cada vuelta significaba copiar el buffer del escritorio entero sin que nada se moviera.
- `window_event` — delega en el overlay; cuando `overlay.finished` es true, **destruye** el overlay y mueve
  el resultado a `self.pending_action`.

### El flujo de dos fases (crítico)

`window_event` nunca ejecuta la acción de captura. Pone el resultado en `pending_action`, la ventana overlay
se destruye (`self.overlay = None`), y `about_to_wait` ejecuta el guardado/copiado en el siguiente tick.
Motivo: `arboard::Clipboard` y `rfd::FileDialog` son APIs OLE bloqueantes; llamarlas dentro del callback de
evento de ratón de una ventana viva cuelga la aplicación. Ver regla 2 de `AGENTS.md`.

### Módulos

- `capture.rs` — captura GDI cruda (`BitBlt` + `GetDIBits`) de todo el escritorio virtual multi-monitor.
  Hace `OpenInputDesktop`/`SetThreadDesktop` para funcionar sobre escritorios seguros, y convierte BGRA→RGBA
  a mano. Devuelve `(RgbaImage, min_x, min_y, total_w, total_h)`; `min_x`/`min_y` pueden ser negativos con
  monitores a la izquierda del principal.
- `overlay.rs` — ventana sin decoración a pantalla completa que dibuja el escritorio atenuado al 50 %
  (precomputado una vez en `bg_buffer`), la selección, la etiqueta de dimensiones y los cuatro botones de
  acción. `InteractionMode` distingue `Creating` (arrastre nuevo) de `Moving` (clic dentro del rect existente).

  **`button_layouts()` es la única fuente de verdad del layout de los botones.** Devuelve, por botón, la
  caja `draw` (con el desplazamiento de hover) y la caja `hit`; pintado, hover y clic leen las tres de
  ahí. Antes eran tres cálculos independientes y ya habían divergido: el test de hover llevaba ±10 px de
  margen vertical e ignoraba el desplazamiento, así que había una banda arriba y abajo de cada botón
  donde el botón se iluminaba pero el clic lo atravesaba, caía en `Creating` y **borraba la selección**.

  La caja `hit` abarca todo el recorrido vertical del desplazamiento (`hover_lift_y`), no la posición
  instantánea. Si siguiera al desplazamiento, un cursor sobre el borde inferior quedaría fuera al
  levantarse el botón, se apagaría el hover, el botón bajaría y volvería a encenderse: oscilación a cada
  frame.

  `redraw()` está partido en dos fases **por una restricción del préstamo**: `surface.buffer_mut()`
  retiene `&mut self` mientras viva el buffer, así que a partir de ahí no se puede llamar a ningún método
  `&self` y solo funciona el acceso directo a campos. Por eso el estado de los botones se avanza y el
  layout se calcula *antes* de pedir el buffer. Era exactamente el motivo de que la aritmética estuviera
  inlineada y duplicada; si añades algo que necesite un método de `&self` durante el pintado, va en la
  fase 1.

  Los dos bucles que recorren la captura (fondo atenuado en `new`, selección sin atenuar en `redraw`)
  usan `as_raw()` por filas, no `get_pixel`: son 2 M de llamadas con comprobación de límites en 1080p y
  8 M en 4K, y el primero está en el camino entre pulsar el atajo y ver el overlay.
- `font.rs` — rasterizado de texto con `ab_glyph` leyendo Consolas Bold directamente de `C:\Windows\Fonts`
  (con fallback a consolas/segoeuib), e iconos Lucide SVG embebidos rasterizados con `resvg` y compuestos
  con alpha sobre el buffer. Sin assets externos: el binario no incrusta fuentes.

  **Todo lo caro está cacheado, porque estas funciones están en el camino de cada frame**: la cara
  parseada en `PARSED_FONT` (`consolas_bold()`) y los iconos ya rasterizados en `ICON_CACHE`, indexados
  por `(IconType, tamaño)`. Antes se reparseaba la fuente en cada etiqueta y en cada `measure_*`, y se
  parseaba el XML del SVG más el rasterizado vectorial cuatro veces por frame. Si añades otro elemento
  dibujado, cachéalo igual.

  El blend de los iconos usa `src + dst * (1 - a)`: `tiny_skia::Pixmap` guarda **alpha premultiplicado**,
  así que los canales ya llevan el factor de cobertura y multiplicar otra vez oscurece los bordes.
- `console.rs` — `main.rs` se compila con `#![windows_subsystem = "windows"]`, así que el doble clic en
  `ruuutu.exe` va directo a la bandeja sin abrir consola. Como contrapartida, un binario GUI arranca sin
  consola ninguna y también se perdería la salida al lanzarlo desde cmd/PowerShell.
  `attach_parent_console()` la recupera: hace `AttachConsole(ATTACH_PARENT_PROCESS)` y, **solo si los
  handles estándar vienen vacíos** (para no pisar un `ruuutu.exe > log.txt`), los reapunta a `CONOUT$` con
  `CreateFileW` + `SetStdHandle`. Desde Explorer no hay consola padre, la llamada falla y todo queda mudo.
  Tiene que ejecutarse **antes del primer `println!`**: `std::io::stdout` cachea el handle de
  `GetStdHandle` en el primer uso. Efecto secundario inevitable: el shell no espera a un proceso GUI, así
  que el prompt vuelve enseguida y la salida se intercala.
- `hotkey.rs` — `global-hotkey` para los atajos configurables, **más** un hook `WH_KEYBOARD_LL` que
  devuelve `1` para `VK_SNAPSHOT`, consumiendo PrtScn para que no se abra la Herramienta de Recortes de
  Windows: es la única forma, porque Windows encamina esa tecla antes de que ningún `RegisterHotKey`
  la vea. El hook comunica con el loop mediante `PRTSCN_TRIGGERED: AtomicBool`.

  El hook ve **todas** las teclas de la sesión, así que está acotado por los dos extremos:
  - Se instala solo si el preset activo usa PrtScn (`HotkeyPreset::uses_print_screen`). Con
    `Ctrl + Shift + S` no hay hook y PrtScn vuelve a ser de Windows. `set_hook_installed` lo engancha
    y desengancha al cambiar de preset.
  - Dentro del callback compara los modificadores con `GetAsyncKeyState` (no `GetKeyState`: en un hook
    de bajo nivel el estado de teclas del propio hilo no es el que el usuario está pulsando) y solo se
    traga la combinación exacta configurada. Alt+PrtScn sigue copiando la ventana activa salvo que sea
    justo el atajo elegido.

  El handle del hook y el preset viajan al callback en `AtomicIsize`/`AtomicU8`, no en `static mut`:
  el callback corre durante el despacho de mensajes y lee lo que escribe `set_preset`, que como
  estático mutable es carrera de datos — y error duro a partir de la edición 2024.
- `icon.rs` — el icono de Ruuutu dibujado por código a cualquier tamaño (`icon_rgba(size)`): marco
  exterior azul, líneas de visor a 1/8 del borde y cuadrado oscuro dentro. Es la **única** fuente del
  icono: `tray.rs` lo rasteriza a 32×32 en tiempo de ejecución para la bandeja, y `build.rs` lo incluye
  con `#[path]` para generar el `.ico` del ejecutable. Por eso este módulo solo puede usar `std` — nada
  de crates ni de otros módulos del proyecto, o `build.rs` deja de compilar.
- `build.rs` — rasteriza `icon.rs` a 16/24/32/48/64/128/256, empaqueta el `.ico` a mano en `OUT_DIR`
  (DIB crudo hasta 64; PNG a partir de 128, que Windows lee de forma nativa y evita los 256 KB que
  ocuparía un BMP de 256×256) y lo incrusta con `embed-resource`. Las dependencias `embed-resource` e
  `image` son de `[build-dependencies]`: no entran en el binario.
- `tray.rs` — icono de bandeja (de `icon.rs`, 32×32 RGBA) y menú con submenús de
  formato/calidad/hotkey.

  `compact_menu_gutter()` aplica `MNS_CHECKORBMP` (vía `SetMenuInfo` + `MIM_APPLYTOSUBMENUS`) al popup
  raíz. Win32 reserva por defecto **dos** columnas a la izquierda de cada ítem, una para la marca de
  verificación y otra para el bitmap del ítem; como aquí los pictogramas son caracteres dentro de la
  etiqueta y no hay bitmaps, esa segunda columna era margen muerto. La bandera hace que ambas compartan
  columna. Tiene que llamarse **después del último `append`**: solo se propaga a los submenús ya
  enganchados en ese momento.

  Más allá de eso el ancho del gutter no es configurable sin *owner-draw* del menú entero. Hubo un
  submenú "DEBUG: Márgenes" que añadía espacios al principio de cada etiqueta (`indent_spaces` /
  `fmt_label`); se eliminó — el problema no era falta de margen sino exceso.
- `config.rs` — persistencia en `%APPDATA%/Ruuutu/config.json` y autostart vía `HKCU\...\CurrentVersion\Run`.
  `AppConfig::save_options()` es el único sitio donde los ajustes del menú se traducen a parámetros de
  encoder concretos.
- `storage.rs` — codificación y escritura. `save_image` guarda en
  `~/Pictures/Ruuutu/Ruuutu_YYYY-MM-DD_HHMMSS.<ext>`; `save_image_to` escribe en una ruta ya elegida. No
  depende de `config.rs` y no debe hacerlo.
- `save_dialog.rs` — diálogo "Guardar como" nativo por COM (`IFileSaveDialog` + `IFileDialogCustomize`),
  con los combos de calidad y escala y la casilla "Recordar" incrustados en el propio diálogo de Windows.
  Sustituye a `rfd`, que solo exponía filtros de extensión. Usa el crate `windows` (no `windows-sys`, que
  declara `IFileSaveDialog` como `*mut c_void` sin vtables). `windows-core` está declarado como dependencia
  directa porque el macro `#[implement]` genera rutas absolutas a `windows_core::`.

  Dos detalles no obvios de esta API:
  - **La etiqueta de la izquierda la pinta el *grupo visual*, no el control.** Un `StartVisualGroup` con
    varios controles dentro rotula solo el primero y deja los demás sin nombre. Por eso cada combo va en su
    propio grupo de un solo control, y la casilla va fuera de todo grupo (su texto ya es su etiqueta).
  - El combo **trunca sobre los ~20 caracteres**, de ahí `short_label_for` / `short_label` en `config.rs`,
    separados de las etiquetas largas del tray.
  - **No hay ningún control sobre la colocación.** Ni posición, ni tamaño, ni alineación: el shell reparte
    los controles con un esquema propio y no documentado. Es limitación reconocida de la API, no un fallo
    nuestro; los mantenedores de wxWidgets llegaron a lo mismo ("we don't have any control over the layout
    with IFileDialogCustomize", y "label text failed to align vertically when multiple controls were
    present"), y recomiendan no pasar de 2-3 controles. Consecuencia concreta aquí: la casilla cae en la
    celda libre bajo el combo de Escala, esa columna queda más alta que la de Calidad y el rótulo "Escala"
    sube unos píxeles. **No es corregible.** Meter la casilla en su propio grupo visual, con etiqueta o sin
    ella, no le da fila propia: solo añade una columna de rótulo vacía y empeora el desajuste. Probado y
    revertido — no volver a intentarlo.

  - **`SetControlItemText` no repinta un combo ya poblado.** Cambia el dato pero la lista sigue mostrando el
    texto viejo. Para reetiquetar hay que `RemoveControlItem` de cada ítem y volver a añadirlos, guardando y
    restaurando la selección alrededor (al vaciarlo se pierde).

  `TypeChangeHandler` implementa `IFileDialogEvents` y, en `OnTypeChange`, reetiqueta el grupo con
  `SetControlLabel` y reconstruye los cuatro ítems de calidad. Es lo que mantiene la paridad con el tray:
  al cambiar el tipo de archivo dentro del diálogo, "Máxima" pasa a leerse "Sin pérdidas" (WebP),
  "Máxima (nivel 9)" (PNG) o "Máxima (100%)" (JPEG). El cookie de `Advise` se libera con `Unadvise`.

### Calidad y escala

"Calidad" no significa lo mismo en los tres formatos, y el menú de bandeja se reetiqueta según el formato
activo (`QualityChoice::label_for`, `ImageFormatChoice::quality_menu_title`). Por eso **cambiar el formato
reconstruye el `SystemTray` entero** en vez de solo remarcar los checks: las etiquetas del submenú cambian.

- **WebP** — `Max` es VP8L sin pérdidas; el resto es VP8 con pérdida a 90/75/50. Requiere la crate `webp`
  (libwebp) porque el encoder WebP de `image` 0.25 es solo `new_lossless()`, sin parámetro de calidad.
- **JPEG** — calidad 100/90/75/50. Hay que convertir a RGB8 antes de codificar: el encoder de `image`
  rechaza `Rgba8` con `UnsupportedErrorKind::Color`.
- **PNG** — siempre sin pérdidas. El ajuste controla el nivel DEFLATE (9/7/4/1): cambia tamaño y tiempo,
  nunca los píxeles.

`ScaleChoice` (100/75/50/25 %) reescala con Lanczos3 **antes** de codificar, y es la única palanca que
reduce el tamaño de un PNG de verdad. Se aplica por igual al fichero guardado y al portapapeles.

El reescalado ocurre **una sola vez**, en `about_to_wait` justo antes de bifurcar por acción: se resamplea
la selección, se pasan los mismos píxeles al encoder y al portapapeles, y se fuerza
`opts.scale_percent = 100` para que `encode_image` no vuelva a escalar. Si añades otra ruta de salida,
consúmela desde ahí y no vuelvas a aplicar la escala.

**`SaveOnly` es la excepción y se bifurca antes**: su formato, calidad y escala se eligen dentro del
diálogo, así que conserva los píxeles originales y `encode_image` hace el reescalado al final. Si la casilla
"Recordar" está marcada, los tres valores se escriben en `AppConfig` y el tray se reconstruye.

La etiqueta azul del overlay recibe estos ajustes por `SaveHint` (`scale_percent` + `format_name`), un
struct de primitivas a propósito: `overlay.rs` entra en `test_bench` por `#[path]` y no debe arrastrar
`config.rs`. Cuando la escala es < 100 % muestra `1920 x 1080 -> 960 x 540 px · WEBP`.

### Semántica de las acciones

`Guardar (S)` abre el diálogo "Guardar como"; `Ambos (Enter)` guarda sin diálogo en la carpeta por defecto
con el formato configurado; `Copiar (C)` solo va al portapapeles; `Esc` cancela sin efectos.

## Decisiones tomadas a propósito (no son descuidos)

- **El overlay ocupa ~26 MB privados en 1080p y no se va a arreglar.** Son tres buffers de pantalla
  completa (8,3 MB cada uno en 1080p): la captura original —hace falta para recortar—, el fondo atenuado
  precomputado y el de softbuffer. Quitar `bg_buffer` cambia un `memcpy` por un bucle píxel a píxel sobre
  toda la pantalla en cada frame: sale peor. La solución real sería repintar solo la región dañada, que es
  un refactor grande para un problema que en 1080p no existe. Revisarlo solo si se apunta a 4K
  multimonitor, donde son ~100 MB. El pilar de "<10 MB" es **en reposo**, y ahí se cumple (10,7 MB).
- **La cadena de ~20 ramas `else if` del menú en `main.rs` se queda.** No hay ningún bug detrás; es riesgo
  futuro, porque cada opción nueva son cinco líneas copiadas y la posibilidad de olvidar el `set_checked`
  o el `save()`. Convertirla en tabla cuando toque añadir un ajuste, no antes.

## Restricciones que no se rompen (de AGENTS.md)

1. **Nunca `PeekMessageW(..., PM_REMOVE)` en el bucle principal.** Con `hwnd = 0` roba los mensajes de ratón
   y teclado antes de que `winit` los procese y la interfaz queda congelada. `winit::run_app` ya gestiona la
   cola Win32.
2. **Desacoplar la destrucción de ventanas de las APIs OLE/clipboard/diálogos** (ver flujo de dos fases arriba).
3. **Botones de acción**: en reposo son cuadrados simétricos con el icono centrado; tras 500 ms de hover se
   despliegan a 60 FPS con el texto recortado limpiamente (`draw_consolas_bold_text_clipped`).

## Estado actual conocido

- El modo debug ya no está forzado: se activa solo con `--debug` (o `-d`). Enciende el HUD de telemetría
  verde sobre el overlay y los recuadros magenta alrededor de las cajas de clic de los botones.
- `config.rs` serializa/parsea el JSON a mano en lugar de con serde (decisión de tamaño de binario).
  Las dos mitades son `AppConfig::to_json` / `from_json`, separadas del disco para poder testearlas, y
  el token de cada valor sale de un par `config_key()` / `from_config_key()` por enum. **Añadir un
  ajuste es añadir ese par y tocar las dos mitades**; el test de round-trip recorre todas las
  combinaciones y lo caza si divergen.

  Antes esto se escribía con `{:?}` y se leía con `content.contains(...)`, y las dos mitades habían
  divergido en silencio: el escritor emitía `"Png"`, el lector buscaba `"PNG"`, y elegir PNG o JPEG en
  la bandeja se perdía en cada reinicio. `json_field` busca ahora la clave por nombre, no una subcadena
  suelta en cualquier parte del documento.

  `autostart` no se lee del fichero: la fuente de verdad es el registro, y `load()` lo consulta allí.
