# Política de seguridad

## Versiones con soporte

Ruuutu se mantiene en una sola línea: **la última versión publicada**. Los arreglos de
seguridad salen como una versión nueva, no como parche de una anterior.

| Versión | Soporte |
|---|---|
| Última release | ✅ |
| Anteriores | ❌ |

## Cómo informar de una vulnerabilidad

**No abras un issue público.** Un issue deja el fallo a la vista de cualquiera desde el
minuto cero, que es justo lo que hay que evitar mientras no exista arreglo.

Usa el aviso privado de GitHub:

**[Report a vulnerability](https://github.com/juanre7/Ruuutu/security/advisories/new)**
(pestaña *Security* → *Advisories* → *Report a vulnerability*)

Si no puedes usar esa vía, escribe a **juanre7jj@gmail.com** con `[Ruuutu][security]` en el
asunto.

### Qué incluir

Cuanto más concreto, antes se arregla:

- Versión de Ruuutu (la del menú de la bandeja o el nombre del `.exe` descargado) y de Windows.
- Qué hace el fallo y qué consigue quien lo explota.
- Pasos para reproducirlo. Si hay un fichero implicado —un `config.json` corrupto, una
  imagen que rompe el codificador—, adjúntalo.

### Plazos

Son compromisos de un proyecto mantenido por una sola persona, no de un equipo con turnos:

| Momento | Plazo |
|---|---|
| Acuse de recibo | **72 horas** |
| Valoración inicial (si es válido, y qué gravedad tiene) | **7 días** |
| Arreglo publicado, o un plan con fechas si es complicado | **90 días** |

Se te menciona en el aviso publicado salvo que prefieras que no.

### Divulgación coordinada

Se pide esperar a que haya versión con el arreglo, o a que se agoten esos 90 días, lo que
llegue antes. Si el plazo vence sin arreglo, publica: el aviso es tuyo.

## Superficie de ataque

Ruuutu no abre puertos, no habla con ningún servidor y no tiene actualizador automático.
Lo que sí procesa, y por tanto donde tiene sentido buscar:

- **`config.json`** (`%APPDATA%\Ruuutu\config.json`): lo lee un parser de JSON escrito a
  mano. Es un target de fuzzing permanente (`fuzz/fuzz_targets/config_from_json.rs`).
- **Codificación de imagen**: el camino WebP entra en **libwebp**, que es C. También se
  fuzzea (`fuzz/fuzz_targets/encode_image.rs`).
- **Hook de teclado de bajo nivel** (`WH_KEYBOARD_LL`) y **autoarranque** en
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

## Verificar una descarga

Cada release lleva, además del `.exe`:

- `ruuutu.exe.sha256`, para comprobar que el fichero no se ha corrompido.
- `ruuutu.exe.sigstore.json`, la procedencia firmada con Sigstore, que dice que ese binario
  concreto lo compiló este repositorio en este workflow.

```powershell
# Que el fichero es el que se publicó
Get-FileHash ruuutu.exe -Algorithm SHA256

# Que lo construyó este repositorio y no otra cosa
gh attestation verify ruuutu.exe --repo juanre7/Ruuutu
```
