# Ruuutu (Pantalla en Finés) 🖥️📸

Software minimalista, ultra-ligero y de alto rendimiento para captura de pantalla de área seleccionable estilo Lightshot, escrito íntegramente en **Rust**.

---

## 🚀 Pilares Fundamentales
- **Bajo consumo de RAM**: Consumo en reposo < 10 MB.
- **Alto Rendimiento**: Cero sobrecarga de GPU/frameworks pesados mediante software rendering puro (`softbuffer`).
- **Formato WebP por defecto**: Guarda automáticamente imágenes comprimidas de alta calidad con sello de tiempo (`Ruuutu_YYYY-MM-DD_HHMMSS.webp`).
- **Tipografía Consolas Bold Mono**: Estética de desarrollador de alta precisión y máxima legibilidad.
- **Iconografía Gráfica Integrada**: Botones con iconos vectoriales de mapa de bits (Portapapeles, Disco, Rayo, Cancelar).

---

## 🎹 Atajos de Teclado y Uso

### Modo Servicio en Segundo Plano (Bandeja del Sistema)
Al ejecutar `ruuutu.exe`, el programa se minimiza en la bandeja del sistema (System Tray) con su icono interactivo.

- **`PrtScn` (Impr Pant)** o **`Alt + A`**: Activa la superposición de captura de pantalla.
- **Menú contextual de Bandeja**:
  - `⚡ Tomar Captura`
  - `🎨 Formato de Imagen`: WebP, PNG o JPEG.
  - `⚙️ Calidad` / `🗜️ Nivel de Compresión`: se adapta al formato elegido (ver más abajo).
  - `📐 Escala de Guardado`: 100 %, 75 %, 50 % o 25 %.
  - `⌨️ Atajo de Teclado`: PrtScn/Alt+A, Ctrl+Shift+S, Alt+PrtScn o Shift+PrtScn.
  - `🚀 Iniciar con Windows`
  - `🔄 Restaurar Ajustes por Defecto`, `📁 Abrir Carpeta de Capturas`, `❌ Salir`

### Calidad y tamaño de archivo

El submenú de calidad cambia según el formato, porque "calidad" no significa lo mismo en los tres:

| Formato | Qué controla el ajuste |
|---|---|
| **WebP** | `Sin pérdidas (VP8L)` o compresión con pérdida al 90 / 75 / 50 %. |
| **JPEG** | Calidad de 100 / 90 / 75 / 50 %. |
| **PNG** | Nivel DEFLATE (9 / 7 / 4 / 1). PNG siempre es sin pérdidas: cambia el tamaño y el tiempo de guardado, nunca la imagen. |

**Escala de Guardado** reescala la captura con Lanczos3 antes de comprimirla. Es la forma más efectiva de
reducir el peso del archivo (una captura de 1080p guardada al 50 % ocupa una fracción y sigue siendo
perfectamente legible), y la única que reduce de verdad un PNG. Se aplica por igual al guardar y al copiar:
lo que va al portapapeles tiene exactamente los mismos píxeles que el fichero.

### Modo Directo (One-Shot CLI)
```bash
ruuutu.exe --capture
```

### Interacción en la Selección (Estilo Lightshot)
1. **Crear Selección**: Arrastra el ratón sobre cualquier área de la pantalla. La etiqueta muestra el tamaño de la selección, el tamaño final si tienes escalado activo, y el formato de destino (`1920 x 1080 -> 960 x 540 px · WEBP`).
2. **Mover Selección**: Si haces clic dentro del recuadro de selección ya creado y arrastras el ratón, **moverás la selección completa manteniendo sus dimensiones y proporción**.
3. **Botones con Iconos e Información de Atajo**:
   - **`📋 Copiar (C)`** *(o tecla `C` / `Ctrl + C`)*: Copia la imagen **únicamente al portapapeles** (sin guardar en disco) y sale.
   - **`💾 Guardar (S)`** *(o tecla `S` / `Ctrl + S`)*: Abre el diálogo nativo "Guardar como", donde además de la carpeta y el nombre puedes cambiar sobre la marcha el **formato, la calidad y la escala** de esta captura concreta. Una casilla `Recordar estos ajustes como predeterminados` permite además fijarlos en el menú del tray.
   - **`⚡ Ambos (Enter)`** *(o tecla `Enter`)*: Guarda en disco y copia al portapapeles simultáneamente.
   - **`✕ Cancelar (Esc)`** *(o tecla `Esc`)*: **Cancela la captura de inmediato**: no guarda archivo en disco ni copia nada al portapapeles.

---

## 🛠️ Compilación desde el código fuente

Asegúrate de tener instalado Rust (cargo 1.85+):

```bash
git clone https://github.com/tu-usuario/Ruuutu.git
cd Ruuutu
cargo build --release --bin ruuutu
```

El ejecutable optimizado estará listo en `target/release/ruuutu.exe` con un tamaño aproximado de **~3.3 MB**.
La mayor parte del crecimiento sobre las versiones iniciales (~1.4 MB) es **libwebp** enlazado estáticamente,
necesario para poder ofrecer WebP con pérdida y calidad ajustable.

---

## 📄 Licencia
Proyecto Open Source bajo licencias MIT u Apache-2.0.
