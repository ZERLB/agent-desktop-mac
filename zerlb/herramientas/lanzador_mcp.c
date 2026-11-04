/*
 * Lanzador fijo de `agent-desktop mcp` para clientes MCP que se desentienden de sus servidores.
 *
 * La app Claude de escritorio (y cualquier cliente que arranque sus servidores con la
 * responsabilidad «desentendida») hace que macOS cobre los permisos al programa que arranca, no
 * a la app. Si ese programa es un script, el permiso le toca al intérprete (sh, zsh o Python), y
 * si es el binario de agent-desktop, su firma cambia con cada compilación y el permiso se pierde.
 *
 * Este lanzador es ese programa: se queda vivo como padre y arranca, como proceso hijo, el
 * envoltorio que mide el uso (registro.sh, que pasa `mcp` a registro_mcp.py) con el binario
 * real. Así los permisos de Accesibilidad, Grabación de pantalla y carpetas se dan una sola vez a
 * este lanzador, que casi nunca se vuelve a compilar, y sirven aunque se actualice agent-desktop.
 *
 * Las rutas se fijan al compilar (ver compilar_lanzador_mcp.sh):
 *   REAL      binario real de agent-desktop
 *   REGISTRO  registro.sh, con registro_mcp.py a su lado
 * Los argumentos que reciba (opciones globales como --session) pasan detrás de `mcp`.
 */

#include <errno.h>
#include <signal.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>

#ifndef REAL
#error "Falta -DREAL=\"ruta del binario de agent-desktop\""
#endif
#ifndef REGISTRO
#error "Falta -DREGISTRO=\"ruta de registro.sh\""
#endif

extern char **environ;
static volatile pid_t hijo = 0;

static void reenviar(int senal) {
    if (hijo > 0) {
        kill(hijo, senal);
    }
}

int main(int argc, char **argv) {
    char **argumentos = calloc((size_t)argc + 4, sizeof(char *));
    if (argumentos == NULL) {
        return 70;
    }
    argumentos[0] = "zsh";
    argumentos[1] = REGISTRO;
    argumentos[2] = "mcp";
    for (int i = 1; i < argc; i++) {
        argumentos[i + 2] = argv[i];
    }
    if (setenv("AGENT_DESKTOP_REAL", REAL, 1) != 0) {
        return 70;
    }
    struct sigaction accion;
    memset(&accion, 0, sizeof(accion));
    accion.sa_handler = reenviar;
    sigaction(SIGTERM, &accion, NULL);
    sigaction(SIGINT, &accion, NULL);
    sigaction(SIGHUP, &accion, NULL);
    pid_t pid;
    int error = posix_spawn(&pid, "/bin/zsh", NULL, NULL, argumentos, environ);
    if (error != 0) {
        fprintf(stderr, "agent-desktop-mcp: no pude arrancar el envoltorio: %s\n", strerror(error));
        return 71;
    }
    hijo = pid;
    int estado = 0;
    while (waitpid(pid, &estado, 0) < 0) {
        if (errno != EINTR) {
            return 71;
        }
    }
    if (WIFEXITED(estado)) {
        return WEXITSTATUS(estado);
    }
    return 128 + WTERMSIG(estado);
}
