# agent-desktop · copia de ZERLB

Esta es una copia de [vercel-labs/agent-desktop](https://github.com/vercel-labs/agent-desktop), la herramienta que deja que un agente de IA lea y maneje las apps de escritorio por la accesibilidad del sistema. La mantenemos para arreglar lo que falla en el uso real (sobre todo macOS en español, macOS 26 y páginas web) y para recibir cada versión nueva del original.

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

Archivos cambiados respecto del original: `crates/macos/src/notifications/{nc_session.rs,actions.rs,mod.rs,close_action.rs}`, `crates/macos/src/system/window/window_postcondition.rs`, `crates/macos/src/tree/{web_content.rs,mod.rs}`, `crates/macos/src/actions/select/select_menu.rs`, `crates/macos/src/actions/chain/chain_value_write.rs` y `crates/macos/src/actions/type_text.rs`. El aviso de cambios que pide la licencia Apache-2.0 es esta lista y el historial de Git.

## Banco de pruebas web

`zerlb/banco/` tiene una página con un formulario (texto, correo, lista, casilla, opciones, área de texto), una ventanita y una lista larga, y `recorrido.py`, que la llena y la comprueba paso a paso, primero sin tocar el mouse ni el teclado.

```sh
cd zerlb/banco && python3 -m http.server 8799 --bind 127.0.0.1 &
open -a Safari http://127.0.0.1:8799/          # o Edge, Chrome, cualquier navegador
python3 zerlb/banco/recorrido.py Safari --bin target/release/agent-desktop
```

Imprime cada paso con el método que funcionó, lo que se leyó después, cuánto tardó, y si el puntero o la app al frente cambiaron. Resultado con esta copia (2-nov-2026, macOS 26, sin modo físico): 12 de 12 pasos en Safari, Microsoft Edge y Chrome for Testing, con el puntero quieto; en Chrome for Testing también con la ventana en segundo plano. Con el original 1.0.2, en el mismo banco: 11 de 12 en Chrome for Testing (falla la lista) y 8 de 12 en Safari (fallan la lista y los tres campos de texto).

## Compilar e instalar

Necesita Rust (la versión la fija `rust-toolchain.toml`) y las herramientas de línea de comandos de Xcode.

```sh
sh zerlb/herramientas/compilar.sh                       # compila la CLI y el ayudante de macOS
sh zerlb/herramientas/compilar.sh --instalar ~/bin/ad   # además los instala y guarda los anteriores
```

Las pruebas del proyecto se corren como dice `AGENTS.md`. Al 2-nov-2026 pasan las 2,910 del equipo anfitrión macOS.

## Medir el uso para saber qué mejorar

`zerlb/herramientas/registro.sh` es un envoltorio: se instala con el nombre `agent-desktop`, apunta al binario real con `AGENT_DESKTOP_REAL` y anota por cada llamada el comando, si fue en modo físico, si salió bien, el código de error, los milisegundos y la app de `--app`. **No guarda textos, referencias, valores, capturas ni la salida.** El registro queda en el equipo (`~/.agent-desktop-uso/registro.jsonl`) y nunca se sube.

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

## Licencia

Apache-2.0, como el original de Vercel Labs. Ver `LICENSE`.
