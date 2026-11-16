# Explorer — Núcleo (v1): diseño

Fecha: 2026-11-16 · Estado: pendiente de revisión

## 1. Objetivo

Un explorador de archivos para Linux escrito en Rust con GPUI (`gpui =0.2.2`, de crates.io),
construido por partes hasta llegar a un Explorer completo. Este documento cubre solo el
**sub-proyecto 1: el Núcleo**, un navegador de carpetas rápido, seguro y sólido que sirve de base
a todo lo demás.

Requisitos de calidad: seguro, performante, robusto, código limpio y SOLID, Rust idiomático.

### Decisiones tomadas

| Tema | Decisión |
|---|---|
| Framework | `gpui =0.2.2` de crates.io, versión fijada |
| Componentes | Todos propios, sin `gpui-component`. `exporer-vc` (en el Escritorio) solo es referencia |
| Vista | Lista detallada tipo `ls -l` (ver §6) |
| Tema | Sigue el modo claro u oscuro del sistema |
| Estilo | Técnico y monoespaciado (maqueta: `assets/explorer-mockup-tech.html`) |
| Iconos | Ninguno en v1. El tipo se distingue por color y por el sufijo `/` en carpetas. La decisión original de usar un set SVG se descarta al elegir este diseño |

## 2. Alcance de la v1

**Incluye**
- Ventana GPUI con tema claro/oscuro del sistema (cambia sola).
- Listado con permisos, tamaño, fecha de modificación y nombre.
- Carpetas primero. Orden por nombre, tamaño o fecha al hacer clic en el encabezado (asc/desc).
- Lista virtualizada: fluida con carpetas de decenas de miles de entradas.
- Navegación: entrar en carpeta, subir, atrás y adelante.
- Barra de ruta con segmentos clicables y modo edición (acepta `~`, `.`, `..`, rutas relativas).
- Selección de una fila con ratón y teclado.
- Errores claros que no cierran la app.
- Tests de la lógica, sin `unsafe`.

**No incluye (sub-proyectos siguientes)**
Abrir archivos con otros programas, copiar/mover/borrar/renombrar/crear, árbol lateral, pestañas,
búsqueda, vista previa, selección múltiple, arrastrar y soltar, refresco automático por cambios
en disco, archivos ocultos conmutables (en v1 se muestran siempre, atenuados), iconos.

## 3. Arquitectura

Workspace de Cargo con dos crates. La dependencia va en un solo sentido: **`app` usa
`explorer-core`, nunca al revés.** `explorer-core` no conoce GPUI.

```
Explorer/
├── Cargo.toml              workspace, lints, perfil release
├── Cargo.lock              partido del lockfile publicado con gpui 0.2.2
├── crates/
│   ├── explorer-core/      lógica pura, sin GPUI
│   │   └── src/ lib.rs  entry.rs  read.rs  error.rs  history.rs  sort.rs  path.rs
│   └── explorer-app/       ventana GPUI
│       └── src/ main.rs  state.rs  theme.rs  assets.rs  ui/{toolbar,pathbar,list,statusbar}.rs
├── assets/fonts/           JetBrains Mono (licencia OFL incluida)
└── docs/superpowers/specs/
```

El crate no se llama `core` porque chocaría con la biblioteca `core` de Rust.

### explorer-core (responsabilidades)

- `entry`: `Entry { name: OsString, kind: Kind, size: u64, modified: Option<SystemTime>, mode: u32 }`
  con `Kind { Dir, File, Symlink { broken: bool } }` y una clasificación por extensión
  (imagen, código, documento, audio/vídeo, archivo comprimido) usada solo para colorear.
- `read`: `read_dir(&Path) -> Result<Vec<Entry>, FsError>`. Una entrada ilegible no tumba el
  listado: se devuelve marcada.
- `error`: `FsError { NotFound, PermissionDenied, NotADirectory, Io(io::Error) }`, con el código
  de Linux asociado (`ENOENT`, `EACCES`, `ENOTDIR`) para mostrarlo.
- `history`: pila atrás/adelante.
- `sort`: `SortKey { Name, Size, Modified }`, dirección, carpetas primero, nombre con
  comparación insensible a mayúsculas y numérica.
- `path`: `resolve(input, cwd, home) -> PathBuf` normaliza `~`, `.`, `..` y rutas relativas.

### explorer-app (responsabilidades)

- `state`: modelo GPUI que envuelve la lógica de `explorer-core`. Guarda carpeta actual, listado,
  historial, orden, fila seleccionada, error vigente y el número de la carga en curso.
- `ui`: `toolbar` (atrás/adelante/subir), `pathbar` (prompt + segmentos + edición), `list`
  (encabezados + filas virtualizadas con `uniform_list`), `statusbar`.
- `theme`: tokens de color para claro y oscuro (§6). Se elige con la apariencia de la ventana y
  se reactiva si el sistema cambia.
- `assets`: carga de las fuentes incluidas en el binario.

Los traits solo aparecerán cuando exista una segunda implementación que los justifique.

## 4. Flujo de datos

1. El usuario navega (doble clic, Enter, ruta escrita, atrás/adelante/subir).
2. `state` incrementa un contador de carga (`generation`) y marca "cargando".
3. `read_dir` se ejecuta en el executor en segundo plano de GPUI. La ventana sigue respondiendo.
   El ordenado también ocurre allí.
4. Al terminar, `state` compara el `generation` recibido con el vigente. Si no coincide, **descarta**
   el resultado. Si coincide, guarda el listado, actualiza historial y selección, y notifica a la UI.
5. Si la carga tarda más de ~150 ms, la barra de estado muestra "cargando…".

Al subir, el cursor queda sobre la carpeta de la que se viene. Al recargar, se conserva el nombre
seleccionado si sigue existiendo.

## 5. Errores y seguridad

- Sin `unwrap`, `expect` ni `panic!` en código de producción (lints `deny` en el workspace).
- `#![forbid(unsafe_code)]` en ambos crates.
- Nombres como `OsString`/`OsStr`: nunca se asume UTF-8. Para pintar se usa conversión con pérdida.
- Un error de carga (`EACCES`, `ENOENT`, `ENOTDIR`) se muestra dentro de la lista como
  `error[EACCES]` con la ruta, y el usuario **se queda en la carpeta anterior**. La app no se cierra.
- Enlaces simbólicos: se muestran como enlaces (`l…`, sufijo `@`). Los rotos se pintan con el color
  de error. Entrar en un enlace a carpeta navega a su destino. No hay recorrido recursivo en v1,
  así que no hay riesgo de bucles.
- No se ejecuta ningún comando de shell ni se abre ningún archivo en v1.

## 6. Diseño visual (referencia: `assets/explorer-mockup-tech.html`)

Todo en **JetBrains Mono**, 12,5 px, filas de 24 px.

**Estructura de la ventana, de arriba abajo**
1. **Línea de ruta (38 px):** botones `←` `→` `↑`, prompt `❯`, segmentos clicables
   (`~` / `Desktop` / `Explorer`), pista `i editar ruta`. Clic o `i` la convierte en un campo de texto.
2. **Encabezado:** `#`, `permisos`, `tamaño`, `modificado`, `nombre`. Tamaño, modificado y nombre
   ordenan al hacer clic y muestran `▲`/`▼` en la columna activa.
3. **Lista:** columnas `# (44) · permisos (92) · tamaño (84) · modificado (142) · nombre (resto)`.
   - `#` son números relativos a la fila activa, como `relativenumber` de vim. La fila activa
     muestra su posición real en el color de acento.
   - Permisos tipo `drwxr-xr-x`. Tamaño en B/KiB/MiB/GiB. Fecha `YYYY-MM-DD HH:MM` en hora local.
   - Carpetas con `/` al final y en negrita. Archivos ocultos (nombre con `.`) atenuados.
   - Fila activa: fondo `cursor` y barra de 2 px de acento a la izquierda.
4. **Barra de estado (24 px):** chip `NORMAL`, permisos + bytes exactos + nombre del archivo
   activo, posición `n/total`, suma de tamaños `Σ` y `UTF-8`.

**Colores por tipo:** carpeta, imagen, código, documento, audio/vídeo, comprimido (+ `danger`).

**Tokens** (ambos temas, tomados de la maqueta):

| Token | Claro | Oscuro |
|---|---|---|
| bg | `#fbfbf9` | `#0d1117` |
| bar | `#f1f1ee` | `#161b22` |
| border | `#d9d9d3` | `#252b34` |
| text | `#24292f` | `#c9d1d9` |
| muted | `#6a717b` | `#8b949e` |
| faint | `#9aa0a8` | `#566070` |
| cursor | `#dfeaf9` | `#172338` |
| accent | `#0969da` | `#58a6ff` |
| dir | `#0550ae` | `#79b8ff` |
| image | `#8250df` | `#d2a8ff` |
| code | `#116329` | `#7ee2a8` |
| doc | `#bc4c00` | `#ffa657` |
| media | `#9a6700` | `#f2cc60` |
| archive | `#cf222e` | `#ff7b72` |
| danger | `#cf222e` | `#ff7b72` |

**Ventana estrecha:** por debajo de ~720 px se ocultan `permisos` y `modificado`.

## 7. Entrada de usuario

| Entrada | Acción |
|---|---|
| `j` / `↓`, `k` / `↑` | Mover la selección |
| `l` / `→` / `Enter` | Entrar en la carpeta seleccionada |
| `h` / `←` / `Backspace` | Subir (el cursor queda en la carpeta de origen) |
| `gg` / `G` | Primera / última fila |
| `i` | Editar la ruta. `Enter` confirma, `Esc` cancela |
| `Alt+←` / `Alt+→` | Atrás / adelante |
| Clic en fila | Seleccionar |
| Doble clic en carpeta | Entrar |
| Clic en encabezado | Ordenar; otra vez invierte |
| Clic en segmento de ruta | Ir a ese nivel |
| Rueda | Desplazar la lista |
| `Ctrl+Q` | Salir |

## 8. Pruebas y calidad

- **`explorer-core`:** tests unitarios de `read_dir` (con `tempfile` como dev-dependency),
  errores, ordenado, historial y `resolve`. Se cubren: nombres no UTF-8, enlace roto, carpeta
  sin permiso, carpeta vacía, y orden carpetas-primero.
- **`explorer-app`:** la lógica con decisiones (descarte de cargas obsoletas, selección tras
  subir/recargar) vive en tipos sin GPUI dentro de `explorer-core` o en funciones puras, para
  poder probarla sin abrir ventana.
- **Puertas de calidad:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  (más los lints `unwrap_used`/`expect_used`/`panic`) y `cargo test`, en cada paso.
- **Perfil release:** `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.

## 9. Dependencias previstas

`gpui =0.2.2` (con el `Cargo.lock` publicado para esa versión: con `libc` reciente `xattr` no
compila), `jiff` (fecha en hora local), `unicode-segmentation` (campo de la ruta, mover por
grafema), `thiserror` (errores) y `tempfile` (solo tests). Cualquier otra dependencia debe
justificarse. El plan de implementación verificará que las versiones encajan con el lockfile fijado.

## 10. Orden de construcción

1. Workspace, lockfile, lints y perfil de release.
2. Ventana GPUI vacía: valida que compila y renderiza en esta máquina (Vulkan).
3. `explorer-core`: `Entry`, `read_dir`, `FsError`, con tests.
4. `explorer-core`: orden, historial, `resolve`, con tests.
5. `state` y carga en segundo plano con descarte de cargas obsoletas.
6. `ui`: lista virtualizada, encabezados, barra de estado.
7. `ui`: línea de ruta y edición.
8. Tema claro/oscuro del sistema y fuentes.
9. Teclado y ratón, ajuste final y pasada de calidad.

## 11. Fuera de este documento

Los sub-proyectos 2–6 (operaciones, árbol lateral, pestañas, búsqueda/atajos, vista previa) tendrán
cada uno su propio diseño. Este diseño no los anticipa más allá de mantener `explorer-core` sin
GPUI y la lista de columnas como datos.
