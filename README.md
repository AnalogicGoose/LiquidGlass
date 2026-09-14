# LiquidGlass

Prueba de concepto del material **Liquid Glass** de Apple (iOS 26) en Rust con
[macroquad](https://github.com/not-fl3/macroquad) y shaders GLSL.

El material replica el componente de Figma **"Liquid Glass - Regular - Large"**
(y su variante **Dark**) capa por capa, con los valores leídos del propio
archivo de Figma y calibrados comparando píxeles contra su render.

## Ejecutar

```sh
cargo run --release
```

En debug también funciona, pero decodificar los JPEG de `assets/` es bastante
más lento.

## Controles

| Entrada | Acción |
|---|---|
| Arrastrar con el ratón | Mover un panel |
| `P` | Cambiar de perfil (Clear → Tinte blanco → Tinte negro) |
| `1` – `4` | Cambiar el fondo |
| `T` | Tinte blanco (Regular) / negro (Dark) sin cambiar el resto de valores |
| `↑` / `↓` | Elegir parámetro |
| `←` / `→` | Ajustar el parámetro elegido |
| `H` | Ocultar/mostrar el panel de parámetros |

## Cómo funciona

Cada frame se hace en tres pasos:

1. **Escena.** El fondo se dibuja (sin deformar, como `object-fit: cover`) en
   un render target a resolución completa.
2. **Frost.** La escena se reduce a media resolución y se desenfoca con un
   Gaussiano separable de dos pasadas (`src/blur.frag`). Desenfocar a baja
   resolución da un blur suave con cualquier radio y cuesta poco.
3. **Paneles.** Cada panel es un quad con `src/glass.frag`, que lee la escena
   nítida y la desenfocada y compone las dos capas del componente de Figma:

   - **"Fill + Shadow"**: sombra proyectada, contorno fino en Linear Burn y el
     tinte (blanco en Lighten + gris en Darken, o `#1A1A1A` en Luminosity +
     Lighten para la variante Dark).
   - **"Glass Effect"**: el efecto GLASS de Figma (refracción, profundidad,
     dispersión, frost, luz y splay) y dos inner shadows en Linear Dodge que
     iluminan los bordes superior e inferior.

La forma es un rectángulo con esquinas continuas (corner smoothing de Figma)
aproximadas con una superelipse. La refracción usa un perfil de bisel
"squircle": la superficie sube hacia dentro a lo largo de `depth`, se refracta
un rayo vertical con `refract()` y se desplaza el muestreo de la escena.

### Parámetros

| Parámetro | Uniform | Figma |
|---|---|---|
| Refracción | `u_refraction` | Glass → Refraction |
| Profundidad | `u_depth` | Glass → Depth |
| Dispersión | `u_dispersion` | Glass → Dispersion |
| Frost | `u_frost` | Glass → Frost |
| Luz | `u_light_intensity` | Glass → Light intensity |
| Ángulo luz | `u_light_angle` | Glass → Light angle |
| Splay | `u_splay` | Glass → Splay |
| Tinte | `u_tint` | Multiplicador de la opacidad de los rellenos (1 = Figma) |
| Sombra | `u_shadow` | Multiplicador de la opacidad de la sombra (1 = Figma) |

### Perfiles

Los valores de cada parámetro se agrupan en perfiles (`PROFILES` en
`src/main.rs`). Se arranca con **Clear** y se cambia con `P`; al cambiar de
perfil se sobrescriben los ajustes hechos con las flechas.

| Perfil | Refracción | Profundidad | Dispersión | Frost | Luz | Ángulo | Splay | Tinte | Sombra | Variante |
|---|---|---|---|---|---|---|---|---|---|---|
| Clear | 2 | 30 | 0.2 | 6 | 0.25 | 0° | 0.2 | 0.15 | 1 | blanca |
| Tinte blanco | 2 | 30 | 0.2 | 16 | 0.25 | 0° | 0.2 | 1 | 1 | blanca |
| Tinte negro | 2 | 30 | 0.2 | 16 | 0.25 | 0° | 0.2 | 1 | 1 | oscura |

Figma no documenta cómo traduce refracción, profundidad, dispersión, ángulo de
luz y splay a píxeles, así que esa parte es una aproximación. El tinte, la
sombra, el contorno y las franjas de luz de los bordes coinciden con su render
(diferencia media < 1/255).

## Estructura

```
src/
  main.rs      ventana, entrada, render targets y panel de parámetros
  glass.frag   material Liquid Glass (capas "Fill + Shadow" y "Glass Effect")
  blur.frag    Gaussiano separable para el frost
assets/        fondos de prueba
```

## Créditos

- Punto de partida y fondos de prueba:
  [archisvaze/liquid-glass](https://github.com/archisvaze/liquid-glass).
- Valores del material: componente "Liquid Glass" del archivo de Figma del
  equipo.
