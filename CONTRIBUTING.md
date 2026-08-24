# Contribuir a Ruuutu

Gracias por pasarte. Ruuutu lo mantiene una sola persona, así que estas normas son cortas
a propósito: describen lo que hay, no lo que a un proyecto le gustaría tener.

## Informar de un fallo

Abre un [issue](https://github.com/juanre7/Ruuutu/issues). Incluye la versión de Ruuutu y
de Windows, qué esperabas y qué pasó. Si el fallo tiene que ver con una captura concreta o
con un `config.json` raro, adjúntalo.

**Excepción importante:** si el fallo es una vulnerabilidad de seguridad, no abras un issue.
Sigue [SECURITY.md](SECURITY.md), que explica cómo avisar en privado y en cuánto tiempo se
responde.

## Proponer un cambio

1. Haz un fork y una rama a partir de `main`.
2. Escribe el cambio junto con sus tests (ver abajo).
3. Comprueba en local que pasa lo mismo que comprueba el CI:

   ```bash
   cargo test --lib --bin ruuutu
   cargo clippy --all-targets --features devtools -- -D warnings
   ```

4. Abre un pull request describiendo **qué problema resuelve**, no solo qué toca.

El CI corre en Windows —que es la única plataforma en la que Ruuutu se ejecuta— y también
en Linux para el núcleo portable. Un pull request con el CI en rojo no se revisa hasta que
esté en verde.

## Tests: qué se espera

**Toda funcionalidad nueva llega con tests, y todo arreglo de un fallo llega con un test que
falla sin el arreglo.** No es una aspiración: es la condición para que un pull request entre.

La parte razonable es reconocer qué se puede testear y qué no. Ruuutu es una aplicación de
escritorio y buena parte de su código habla con GDI, con la bandeja del sistema, con un hook
de teclado de bajo nivel y con diálogos COM. Eso no se ejecuta en un runner sin sesión
gráfica, y fingir lo contrario solo produce tests que no prueban nada.

Así que la regla se aplica donde el test significa algo:

| Dónde | Qué se espera |
|---|---|
| `src/config.rs`, `src/storage.rs` (el núcleo, en `src/lib.rs`) | Tests unitarios obligatorios. Corren en cualquier plataforma. |
| Geometría de `src/overlay.rs` | Tests unitarios obligatorios para la lógica de cálculo. |
| Parseo de entrada y codificación de imagen | Además de los tests, valora añadir o ampliar un target en [`fuzz/`](fuzz/). |
| `capture.rs`, `tray.rs`, `hotkey.rs`, `clipboard.rs`, `save_dialog.rs`, `console.rs` | Sin tests automáticos: necesitan escritorio real. Se comprueban a mano con `cargo run --features devtools --bin test_bench`, y el pull request debe decir qué se probó. |

Si un cambio en uno de los módulos de la última fila puede separarse en una parte pura y una
parte que toca Win32, sepáralas y testea la pura. Es como llegaron `config` y `storage` a
donde están.

## Estilo

- Los comentarios y la documentación del código van **en inglés**; el README, este fichero y
  los mensajes de la interfaz, **en español**. Es lo que ya hay: mantenlo.
- `cargo clippy` con `-D warnings` es una puerta del CI, no una sugerencia.
- Explica en un comentario el *porqué* de lo que no es obvio. El código ya dice el *qué*.
- Cada fichero fuente lleva su cabecera `SPDX-License-Identifier: GPL-3.0-or-later`.

## Licencia

Al contribuir aceptas que tu código se publique bajo la **GNU General Public License v3.0 o
posterior**, igual que el resto del proyecto.
