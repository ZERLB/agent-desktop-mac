// Espera a que la persona lleve N segundos sin teclear ni hacer clic (máximo 60 s).
// Sirve para lanzar una prueba que puede traer una ventana al frente sin que le caiga encima
// de lo que está escribiendo. Sale con 0 si quedó ociosa y con 1 si siguió ocupada.
//
// Compilar: swiftc -O zerlb/herramientas/esperar_ocioso.swift -o /tmp/esperar_ocioso
// Usar:     /tmp/esperar_ocioso 2 && agent-desktop press --app "Mi App" cmd+n
import ApplicationServices
import Foundation

let need = Double(CommandLine.arguments.dropFirst().first ?? "2") ?? 2
let start = Date()
while Date().timeIntervalSince(start) < 60 {
    let key = CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .keyDown)
    let click = CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .leftMouseDown)
    if min(key, click) >= need {
        print("ocioso")
        exit(0)
    }
    usleep(100_000)
}
print("sigue ocupado")
exit(1)
