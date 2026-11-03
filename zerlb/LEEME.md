# agent-desktop para Mac · copia de ZERLB

Esta es la versión **para Mac** de [vercel-labs/agent-desktop](https://github.com/vercel-labs/agent-desktop), la herramienta que deja que un agente de IA lea y maneje las apps de escritorio por la accesibilidad del sistema. La mantenemos para arreglar lo que falla en el uso real (sobre todo macOS en español, macOS 26 y páginas web) y para recibir cada versión nueva del original.

Se compila, se prueba y se usa en macOS (Apple Silicon). El código de Windows y Linux del original se conserva sin cambios, sólo para poder juntar cada versión nueva de Vercel sin conflictos; esta copia no lo prueba ni lo da por bueno.

Todo lo demás es igual que en el original: mismos comandos, mismo JSON, misma licencia (Apache-2.0). Lo propio de esta copia vive en esta carpeta `zerlb/` y en los commits que se listan abajo.

## Qué arregla esta copia

Cada arreglo salió de una prueba real en una Mac con macOS 26 en español, y tiene su commit, su prueba automática y su prueba con la app de verdad.

| Falla en el original | Qué pasaba | Arreglo |
| --- | --- | --- |
| Centro de notificaciones en una Mac que no está en inglés | `list-notifications --headed` y `dismiss-notification` buscaban el reloj de la barra de menú por su nombre en inglés, «Clock», y fallaban con el error -1719 de System Events | Se busca por su identificador, `com.apple.menuextra.clock`, igual en todos los idiomas |
| Descartar notificaciones en macOS 26 | El botón de cerrar ahora es una acción interna con nombre en el idioma del sistema («Cerrar»); fallaban los cuatro métodos | Se usa esa acción, reconociendo «Close», «Cerrar», «Fermer» y otros, y nunca «Borrar todo» |
| `minimize` y `restore` | Respondían `TIMEOUT` aunque la ventana sí se minimizaba, porque leían el estado en plena animación | La lectura «ocupado» se repite hasta el límite de tiempo |
| `select` en una página web | En Chrome, Edge y Electron abría el menú del clic derecho, esperaba 5 s y lo dejaba abierto | En páginas web, la lista se pone a la vista y se abre con un clic; un menú que quede abierto se cierra |
| `type` y `set-value` en una página web | Safari y Chrome aceptaban el texto y lo borraban; el campo quedaba vacío | El campo se enfoca dentro de su app (sin pasarla al frente) y, si hace falta, el texto se mete en la posición del cursor |
| `type` en editores web (chats, documentos) | En Edge, Chrome y Safari, un editor `contenteditable` como los de los chats web o el cuerpo de un documento no recibía el texto: `type` decía «entregado» y la página no se enteraba, o lo borraba al instante (en el uso real, `ACTION_FAILED`) | El texto llega como teclas dirigidas sólo al proceso de esa app, sin pasarla al frente ni mover el puntero, de una en una: Chromium descarta en segundo plano las teclas que traen más de un carácter |
| Referencias escritas en zsh | `"@$SNAP:e5"` llegaba como `@5`, porque zsh lee `:e` como modificador de la variable, y el error sólo pedía la forma completa | El error explica la causa y da la forma con llaves, `"@${SNAP}:e5"`; la guía para agentes también lo dice |
| `--app` con el nombre en inglés | En una Mac en español, `--app Calculator` respondía `APP_NOT_FOUND` porque la app se llama «Calculadora» | Desde la 1.1.0 lo arregla también el original (nombre del ejecutable, sólo si ninguna app abierta tiene ese nombre visible), y esta copia usa ya esa versión |
| Botones de las notificaciones | La lista no mostraba ninguna acción y `notification-action` no encontraba «Responder» ni «Archivar», que en macOS 26 son acciones internas | La lista muestra las acciones internas (menos las de cerrar) y `notification-action` las ejecuta |
| `type` en buscadores nativos | En Ajustes o Mensajes, `type` respondía `POLICY_DENIED`, o aceptaba el texto y lo descartaba si el campo no tenía el foco | En un campo de una sola línea sin foco, el texto se agrega al final del valor; con foco se sigue escribiendo en el cursor. Nunca en áreas de texto de varias líneas, para no perder su formato |
| Búsquedas con acentos | `find --name "Sin título"` no encontraba el título de TextEdit (macOS lo entrega con el acento separado), y «Bateria» no encontraba «Batería» | Las búsquedas comparan sin acentos y aceptan las dos formas de escribirlos |

Archivos cambiados respecto del original: `crates/macos/src/notifications/{nc_session.rs,actions.rs,mod.rs,scan.rs,custom_actions.rs}`, `crates/macos/src/system/window/window_postcondition.rs`, `crates/macos/src/tree/{web_content.rs,mod.rs,adapter.rs}`, `crates/macos/src/tree/properties/action_list.rs`, `crates/macos/src/actions/select/select_menu.rs`, `crates/macos/src/actions/chain/chain_value_write.rs`, `crates/macos/src/actions/{type_text.rs,physical_keyboard.rs}`, `crates/macos/src/input/keyboard/{keyboard_event.rs,mod.rs}`, `crates/core/src/tree/{search_text.rs,search_text_tests.rs}`, `crates/core/src/refs/{ref_token.rs,ref_token_tests.rs}`, `crates/core/src/actions/post_action.rs`, `skills/agent-desktop/SKILL.md`, `skills/agent-desktop-macos/SKILL.md` y `README.md`. El aviso de cambios que pide la licencia Apache-2.0 es esta lista y el historial de Git.

## Banco de pruebas web

`zerlb/banco/` tiene una página con un formulario (texto, correo, lista, casilla, opciones, área de texto), una ventanita y una lista larga, y `recorrido.py`, que la llena y la comprueba paso a paso, primero sin tocar el mouse ni el teclado.

```sh
cd zerlb/banco && python3 -m http.server 8799 --bind 127.0.0.1 &
open -a Safari http://127.0.0.1:8799/          # o Edge, Chrome, cualquier navegador
python3 zerlb/banco/recorrido.py Safari --bin target/release/agent-desktop
```

`editores.html` y `editores.py` prueban `type` en tres campos como los de las apps web: un editor `contenteditable` simple, uno controlado por la página (como Lexical, ProseMirror o Draft.js, que descartan lo que no llega como evento) y un campo de texto controlado (como en React). La página anota lo que de verdad recibió, no sólo lo que se ve:

```sh
open -a Safari http://127.0.0.1:8799/editores.html
python3 -I zerlb/banco/editores.py Safari --bin target/release/agent-desktop
```

Resultado del 3-nov-2026: con la 1.1.0 y los arreglos anteriores, en Chrome for Testing `type` respondía bien en los tres pero la página no recibía nada en el editor simple y borraba el texto del controlado; con esta copia, los tres reciben «Hola, ¿qué tal? ñ 😀» en Chrome for Testing (ventana abierta detrás de otras, sin pasar al frente), en Microsoft Edge (instancia aparte con perfil de prueba) y en Safari, con el texto al final del campo y `delivered_verified` (10 de 10 seguidos en el editor controlado). El banco de formularios sigue en 12 de 12 en los tres.

`recorrido.py` imprime cada paso con el método que funcionó, lo que se leyó después, cuánto tardó, y si el puntero o la app al frente cambiaron. Resultado con esta copia (2-nov-2026, macOS 26, sin modo físico): 12 de 12 pasos en Safari, Microsoft Edge y Chrome for Testing, con el puntero quieto; en Chrome for Testing también con la ventana en segundo plano. Con el original 1.0.2, en el mismo banco: 11 de 12 en Chrome for Testing (falla la lista) y 8 de 12 en Safari (fallan la lista y los tres campos de texto).

## Apps probadas

Prueba del 2-nov-2026 en macOS 26 en español, con la app abierta en segundo plano: leer la ventana, los menús y una captura, y escribir en el buscador cuando no cambia nada. Las apps se leen sin pasarlas al frente.

| Resultado | Apps |
| --- | --- |
| Se leen bien y se manejan sin pantalla | Finder, Ajustes, Calculadora, Teléfono, FaceTime, Mensajes, Mail, Notas, Recordatorios, Calendario, Fotos, Vista Previa, QuickTime, Monitor de Actividad, App Store, TextEdit, Safari, Google Chrome, Microsoft Edge, Word, Excel, Obsidian, Claude, ChatGPT, Canva, una app web de Safari, un cliente de bases de datos hecho en Java y un descompresor |
| Necesitan unos segundos después de abrirse | ChatGPT, Canva y las apps web: al principio sólo muestran el marco de la ventana (3 a 10 elementos) y a los 10 s muestran su contenido. Esperar con `snapshot --wait-for` o volver a leer |
| Se leen, pero escribir pide el modo físico | WhatsApp: sus campos no aceptan texto ni muestran su valor por accesibilidad; hace falta `--headed type` con la app al frente |
| No exponen su contenido | Spotify (ni macOS ve sus botones; se controla con AppleScript: `tell application "Spotify" to playpause`) y Warp (expone sólo el marco) |
| Sin ventana | Apps que viven sólo en la barra de menú (VPN, utilidades): `snapshot` responde `WINDOW_NOT_FOUND`. Desde la 1.1.0 sus íconos se leen con `snapshot --surface menu-bar-extras` (con `--app`, el de esa app); el 3-nov se leyeron los del sistema y el de Hidden Bar |

Paneles de Guardar y Abrir (TextEdit, macOS 26): Guardar funciona sin pantalla (nombre con `set-value` y el botón Guardar). En el panel de Abrir la lista de archivos de iCloud se redibuja todo el tiempo y el archivo elegido queda `STALE_REF`; para abrir un archivo es mejor `open -a App archivo`. Los atajos de teclado van con `press --app App`, que los manda a esa app sin pasarla al frente; un `press` sin `--app` va a la app que esté al frente.

## Apps ocultas o en segundo plano

Prueba del 3-nov-2026 con Chrome for Testing y Safari, para trabajar sin estorbar a quien usa la Mac:

- **Lo que sí funciona siempre:** la ventana abierta detrás de otras (o en una esquina), sin pasar al frente. `snapshot`, `click`, `scroll-to` y `type` la manejan sin moverle el puntero a nadie, en unos 270 ms por campo.
- **Con la app oculta (⌘H) funciona a ratos.** La app sigue oculta y sin pasar al frente, pero Chromium (Chrome, Edge, apps Electron) atrasa sus actualizaciones de accesibilidad: el foco y los valores llegan tarde, y `type` en un editor enriquecido falló en las últimas pruebas aunque los campos normales siguieron funcionando.
- En Chromium, una página que **carga o se recarga** con la app oculta o la ventana minimizada no muestra su contenido por accesibilidad hasta que la ventana se ve una vez; ni las opciones de arranque contra el ahorro en segundo plano lo cambian.
- `screenshot` de una ventana oculta devuelve el último cuadro de antes de ocultarla, no lo que pasó después: el estado se lee con `get` o `snapshot`.
- En Safari, dar el foco a un campo web puede pasar Safari al frente (pasaba también antes de estos arreglos).

## Compilar e instalar

Necesita Rust (la versión la fija `rust-toolchain.toml`) y las herramientas de línea de comandos de Xcode. La compilación ocupa unos 2 GB: si la copia vive en una carpeta que se respalda, compila fuera con `CARGO_TARGET_DIR` (por ejemplo `~/.cargo-target/agent-desktop`). Para una copia liviana sin las imágenes de la documentación: `git clone --filter=blob:none --no-checkout …` y `git sparse-checkout set --no-cone '/*' '!/assets/'`.

```sh
sh zerlb/herramientas/compilar.sh                       # compila la CLI y el ayudante de macOS
sh zerlb/herramientas/compilar.sh --instalar ~/bin/ad   # además los instala y guarda los anteriores
```

Las pruebas del proyecto se corren como dice `AGENTS.md`. Al 3-nov-2026 pasan las 2,921 del equipo anfitrión macOS.

## Medir el uso para saber qué mejorar

`zerlb/herramientas/registro.sh` es un envoltorio: se instala con el nombre `agent-desktop`, apunta al binario real con `AGENT_DESKTOP_REAL` y anota por cada llamada el comando, si fue en modo físico, si salió bien, el código de error, los milisegundos, la app (la de `--app` o, en una acción por referencia, la de la lectura de donde salió), los nombres de las opciones usadas, la forma de la referencia (`completa`, `corta` o `vaciada-por-zsh`) y si fue una consulta de ayuda, que no cuenta como error. En un `INVALID_ARGS` guarda el mensaje sin lo que iba entre comillas ni números. **No guarda textos, referencias, valores, capturas ni la salida.** El registro queda en el equipo (`~/.agent-desktop-uso/registro.jsonl`) y nunca se sube.

```sh
python3 zerlb/herramientas/uso.py --dias 7          # resumen en texto
python3 zerlb/herramientas/uso.py --dias 30 --md    # en Markdown, para un reporte
```

El resumen ordena los comandos por uso y dice cuántos fallan y con qué código: lo que más se usa y más falla es lo siguiente que hay que arreglar, con una prueba en el banco o en la app donde falló.

## Recibir las actualizaciones del original

```sh
sh zerlb/herramientas/novedades.sh
```

Consulta `vercel-labs/agent-desktop` y dice cuántos commits nuevos tiene el original, cuál es su último lanzamiento y qué versión está instalada. Para juntarlos: `git merge upstream/main` (sin reescribir historia), compilar, correr las pruebas y el banco web, y subir.

## Probar todas las apps

`zerlb/banco/apps.py` repite esa prueba con una lista de apps (una ruta por renglón en un archivo de texto) y deja un renglón JSON por app. No lee contenido: sólo cuenta elementos y anota si cada paso funcionó.

```sh
python3 zerlb/banco/apps.py lista-de-apps.txt resultados.jsonl
```

## Licencia

Apache-2.0, como el original de Vercel Labs. Ver `LICENSE`.
