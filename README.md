# ShieldExplorer

Un explorador de archivos para Linux hecho en Rust con [GPUI](https://www.gpui.rs/), el framework
de interfaz acelerado por GPU de [Zed](https://zed.dev). Está pensado para **desarrolladores y
gente técnica** que quieren ir rápido y **sin tocar el ratón**: atajos de teclado estilo vim,
aspecto de terminal y la información que importa a la vista (permisos, tamaño exacto, fecha).

> **Proyecto vibecodeado.** Lo estoy haciendo con ayuda de IA, por diversión, para tener un
> explorador de archivos a mi gusto. Una persona decide qué se construye y lo prueba; el código lo
> escribe una IA. Tenlo en cuenta si lo lees o si piensas usarlo: es un proyecto personal en
> evolución, no un producto terminado.

![ShieldExplorer en tema oscuro](docs/images/screenshot-dark.png)

## Estado

Versión temprana. Ya sirve para **navegar por carpetas**. Todavía **no** abre archivos ni los
copia, mueve, borra o renombra (ver la [hoja de ruta](#hoja-de-ruta)).

La lógica (`shieldexplorer-core`) tiene tests automáticos. La interfaz se prueba a mano: las
teclas, los clics y la edición de la ruta no tienen todavía pruebas automatizadas.

## Qué tiene hoy

- **Listado detallado** estilo `ls -l`: permisos (`drwxr-xr-x`), tamaño, fecha de modificación y
  nombre. Las carpetas van primero y llevan `/` al final; los enlaces simbólicos llevan `@`.
- **Números de línea relativos**, como `relativenumber` de vim, para saltar con `5j` en el futuro.
  La fila activa muestra su posición real.
- **Orden por columna** (nombre, tamaño, fecha), ascendente o descendente, con orden natural
  (`f2` va antes que `f10`) y sin distinguir mayúsculas.
- **Lista virtualizada**: solo se dibujan las filas visibles, así que una carpeta enorme no frena
  la ventana.
- **Carga en segundo plano**: la ventana nunca se congela leyendo el disco. Si navegas rápido, se
  descartan las cargas viejas.
- **Línea de ruta** con segmentos clicables y modo edición (acepta `~`, `.`, `..` y rutas
  relativas).
- **Historial** atrás y adelante.
- **Errores que no rompen nada**: una carpeta sin permiso o inexistente muestra `error[EACCES]` o
  `error[ENOENT]` y te quedas donde estabas.
- **Tema claro y oscuro** que sigue al del sistema y cambia solo.
- **Barra de estado** con el modo, los permisos y los bytes exactos del archivo activo, la
  posición y el total de la carpeta.

## Atajos

### Lista

| Tecla                   | Acción                                              |
| ----------------------- | --------------------------------------------------- |
| `j` / `↓`               | Bajar                                               |
| `k` / `↑`               | Subir                                               |
| `l` / `→` / `Enter`     | Entrar en la carpeta seleccionada                   |
| `h` / `←` / `Backspace` | Subir a la carpeta padre (el cursor queda en la de origen) |
| `gg` / `Home`           | Primera fila                                        |
| `G` / `End`             | Última fila                                         |
| `i`                     | Editar la ruta                                      |
| `Alt+←` / `Alt+→`       | Atrás / adelante en el historial                    |
| `Esc`                   | Cerrar un mensaje de error                          |
| `Ctrl+Q`                | Salir                                               |

### Editando la ruta

`Enter` va a esa ruta, `Esc` cancela. Se puede mover con `←` `→` `Home` `End`, borrar con
`Backspace` y `Delete`, y pegar con `Ctrl+V`.

### Ratón (opcional)

Clic selecciona, doble clic entra en una carpeta, clic en un encabezado ordena, clic en un
segmento de la ruta salta a ese nivel, y la rueda desplaza la lista.

## Seguridad y robustez

- `unsafe_code` está **prohibido** en el código del proyecto (las dependencias sí lo usan).
- No ejecuta comandos de shell ni abre archivos.
- Los nombres de archivo se tratan como lo que son en Linux, bytes que no tienen por qué ser UTF-8.
- Sin `unwrap`, `expect` ni `panic!` en el código: los errores son tipos propios.
- Los enlaces simbólicos se muestran como tales y los rotos se marcan. No hay recorrido
  recursivo, así que no hay riesgo de bucles.

## Compilar y ejecutar

Necesitas:

- Rust con soporte para la edición 2024 (1.85 o posterior). Probado con 1.92.
- Linux con Wayland o X11 y un driver gráfico con Vulkan.
- Las cabeceras de desarrollo de xkbcommon, Wayland, xcb y fontconfig. En la máquina de desarrollo
  (Ubuntu) están instalados `libxkbcommon-dev`, `libxkbcommon-x11-dev`, `libwayland-dev`,
  `libxcb1-dev` y `libfontconfig-dev`; si la compilación falla por una librería, empieza por esas.

```sh
# Abrir en la carpeta actual, o en la que indiques
cargo run -p shieldexplorer-app
cargo run -p shieldexplorer-app -- ~/proyectos

# Binario optimizado en target/release/shieldexplorer
cargo build --release
```

Si pasas la ruta de un archivo, se abre su carpeta con el archivo seleccionado.

### Desarrollo

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

La primera compilación tarda porque GPUI trae muchas dependencias.

## Estructura

```
ShieldExplorer/
├── crates/
│   ├── shieldexplorer-core/   lógica sin interfaz: leer carpetas, orden, historial, rutas, formato
│   └── shieldexplorer-app/    ventana GPUI: estado, lista, línea de ruta, barra de estado, tema
├── assets/fonts/              JetBrains Mono (licencia OFL)
└── docs/superpowers/specs/    documento de diseño y maqueta visual
```

La dependencia va en un solo sentido: `shieldexplorer-app` usa `shieldexplorer-core` y nunca al
revés. Así la lógica se prueba sin abrir ninguna ventana. El diseño completo está en
[`docs/superpowers/specs/2026-11-16-explorer-nucleo-design.md`](docs/superpowers/specs/2026-11-16-explorer-nucleo-design.md).

`gpui` está fijado a `=0.2.2` porque su API es 0.x y cambia entre versiones. El `Cargo.lock` parte
del que publica esa versión: con un `libc` reciente, `xattr 0.2.3` no compila.

## Hoja de ruta

Sin fechas ni promesas, ordenada por lo que más encaja con el enfoque de teclado y productividad:

1. **Modo vim completo:** contadores (`5j`), búsqueda incremental con `/` y `n`/`N`, línea de
   comandos con `:`, marcas, `Ctrl+D`/`Ctrl+U`, `H`/`M`/`L`, mostrar y ocultar archivos ocultos.
2. **Operaciones con el teclado:** copiar (`yy`), cortar, pegar (`p`), borrar a la papelera,
   renombrar, crear archivos y carpetas, selección visual (`v`).
3. **Abrir y salir al entorno de desarrollo:** abrir con el programa por defecto o con `$EDITOR`,
   copiar la ruta al portapapeles, abrir una terminal en la carpeta.
4. **Búsqueda rápida** tipo fzf que respete `.gitignore`, y marcadores de carpetas.
5. **Más contexto para devs:** estado de git en la lista y vista previa de archivos de texto.
6. **Configuración:** atajos y colores propios.
7. Árbol lateral y pestañas.

## Agradecimientos

- [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), de Zed Industries.
- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), con licencia SIL OFL 1.1.

## Licencia

Por definir.
