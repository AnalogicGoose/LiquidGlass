# Analogic Goose Presents: SPARK GLASS

SparkGlass is a GPU visual-material proof of concept by Analogic Goose. It
recreates Apple's Liquid Glass optical character in Rust and serves as the
visual reference for a future windowless, embeddable renderer.

Copyright © 2026 Analogic Goose. Licensed under the MIT License; see
[LICENSE](LICENSE).

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

## Arquitectura Liquid Glass

La apariencia existente se conserva: los shaders y sus parámetros calibrados
no se modificaron. Lo que cambió es el límite entre la UI y el renderizador.
La UI describe superficies semánticamente y un `GlassScene` de ventana las
compone en orden contra un único backdrop compartido:

```text
UI → GlassScene → shared sharp backdrop → shared downsampled blur
   → ping-pong glass stack → GlassSurface geometry/SDF
   → refraction + frost + tint + edge light → foreground
```

Cada superficie lee el resultado acumulado de las superficies que están debajo
y escribe en el otro render target del par. Antes de dibujarla, ese resultado
acumulado se reduce y desenfoca en los targets reutilizables. Así un cristal
inferior sigue visible, pero el frost del cristal superior vuelve a difundirlo,
como el apilado de materiales de macOS, sin capturas ni lecturas desde CPU.
Un segundo par de targets mantiene la cobertura del vidrio acumulado. Cuando
una superficie superior encuentra vidrio debajo, reduce gradualmente la doble
refracción, el doble frost, el tinte y la luz especular. Como esa máscara sigue
el orden del stack, traer otra superficie al frente cambia automáticamente la
calidad óptica de la intersección.
La cobertura y las transiciones de inner shadow también se desenfocan con el
frost de la superficie superior; por eso no quedan siluetas duras al usar frost
alto.

`GlassSurface` agrupa `GlassGeometry`, `GlassMaterial`, `GlassOptics`,
`GlassLighting`, estilo e interacción. `GlassGroup` permite que controles
relacionados compartan el mismo entorno de escena. Los estilos (`Thin`,
`Regular`, `Prominent`, `Control`, `Navigation`) son la API que debe usar una
UI de producto; no debe conocer uniforms ni shaders.

`MacroquadGlassRenderer` es la implementación funcional actual. El límite
`NativeGlassRenderer` define la integración posterior con un compositor WinUI
(backdrop/composition brushes) y GTK4 (snapshot/shader o degradación nativa),
sin transferir detalles de GPU al core de la aplicación. Las implementaciones
concretas de WinUI y GTK4 requieren sus respectivos crates/proyectos host;
este PoC no enlaza esos toolkits.

Calidad y accesibilidad se centralizan en `GlassQuality` y los flags de escena:
en `Low` se elimina refracción y en `Fallback`/reduced transparency se elimina
frost. `Q` recorre los niveles durante el desarrollo. `D` muestra límites y el
estado de la escena para inspeccionar coordenadas y el backdrop compartido.
Para capturas reproducibles del framebuffer, se puede definir
`SPARK_GLASS_CAPTURE` con una ruta PNG antes de ejecutar el binario; la demo
guarda el quinto frame y termina.
`SPARK_GLASS_TEST_OVERLAP=1` coloca la cápsula detrás del panel grande para
comprobar visualmente la composición y refracción glass-on-glass.
`SPARK_GLASS_TEST_FROST` permite fijar el frost de esa captura (por ejemplo,
`30`) para regresiones de blur e inner shadows.
Para medir el binario release durante un número fijo de frames, define
`SPARK_GLASS_BENCHMARK_FRAMES=180`. La salida incluye frame time/FPS y el
tiempo de envío CPU del pipeline (`render_cpu_ms`). Se puede combinar con
`SPARK_GLASS_TEST_OVERLAP=1` y `SPARK_GLASS_TEST_FROST=30`.

## Estructura

```
src/
  main.rs      demo, entrada y HUD de diagnóstico
  glass.rs     API semántica: escena, superficies, grupos, materiales y tokens
  renderer.rs  límite de renderer, pipeline Macroquad y contratos WinUI/GTK4
  glass.frag   material Liquid Glass (capas "Fill + Shadow" y "Glass Effect")
  glass_mask.frag  cobertura acumulada para respuesta glass-on-glass
  blur.frag    Gaussiano separable para el frost
assets/        fondos de prueba
```

## About / Créditos

**Analogic Goose Presents: SPARK GLASS**

SparkGlass is an independent project by Analogic Goose. “Liquid Glass” is
Apple's design/material terminology and is referenced here only to describe
the visual target. SparkGlass is not affiliated with or endorsed by Apple.

- Punto de partida y fondos de prueba:
  [archisvaze/liquid-glass](https://github.com/archisvaze/liquid-glass).
- Valores del material: componente "Liquid Glass" del archivo de Figma del
  equipo.
