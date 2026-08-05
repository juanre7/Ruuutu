# AGENTS.md

El nombre del proyecto es Ruuutu porque en finés significa Pantalla.

# Pilares fundamentales
- Bajo uso de RAM
- Alto rendimiento y optimización incluso para equipos con bajos recursos
- Código de calidad, mantenible y principalmente\mayoritariamente en Rust
- Proyecto Open Source que apoye a la comunidad
- Usar la skill de ponytail

# Lecciones técnicas y arquitectura crítica (NUNCA ROMPER)
1. **NUNCA usar `PeekMessageW(..., PM_REMOVE)` en el bucle principal de `winit`**:
   - `PeekMessageW` con `hwnd = 0` roba y elimina de la cola los mensajes del ratón (`WM_LBUTTONDOWN`, `WM_LBUTTONUP`) y teclado de Windows antes de que `winit` los procese, dejando la interfaz congelada y sin responder a los clics. `winit` administra la cola de mensajes Win32 automáticamente en `run_app`.
2. **Desacoplar la destrucción de ventanas de las APIs bloqueantes OLE/Clipboard/Dialogs**:
   - Las llamadas a `arboard::Clipboard` o `rfd::FileDialog` NUNCA deben ejecutarse dentro de `window_event` mientras el callback del evento de ratón de la ventana está activo. La ventana overlay debe destruirse primero (`overlay = None`), y la acción (copiar/guardar) se ejecuta inmediatamente después en `about_to_wait`.
3. **Botones de acción de la selección**:
   - En reposo son cuadrados simétricos con el icono centrado. Al hacer hover (tras 500 ms de delay), se despliegan suavemente a 60 FPS con el texto recortado limpiamente.