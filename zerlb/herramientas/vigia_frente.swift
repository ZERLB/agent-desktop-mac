// Anota cada cambio de la app que está al frente, según el servidor de ventanas, durante N segundos.
// Sirve para medir si una prueba le quitó el frente a la persona y por cuánto tiempo.
//
// Compilar: swiftc -O zerlb/herramientas/vigia_frente.swift -o /tmp/vigia_frente
// Usar:     /tmp/vigia_frente 8        (deja correr la prueba mientras tanto)
//
// Cada renglón: segundos desde el arranque, pid de la app al frente y hace cuánto fue el último
// clic y la última tecla de la persona, para distinguir un cambio suyo de uno de la prueba.
import ApplicationServices
import Foundation

typealias FrontFn = @convention(c) (UnsafeMutableRawPointer) -> Int32
typealias PidFn = @convention(c) (UnsafeRawPointer, UnsafeMutablePointer<pid_t>) -> Int32

setvbuf(stdout, nil, _IOLBF, 0)
guard let sky = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY),
      let services = dlopen("/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices", RTLD_LAZY),
      let frontSymbol = dlsym(sky, "_SLPSGetFrontProcess"),
      let pidSymbol = dlsym(services, "GetProcessPID")
else {
    print("Este macOS no ofrece las funciones que usa el vigilante.")
    exit(1)
}
let front = unsafeBitCast(frontSymbol, to: FrontFn.self)
let pidOf = unsafeBitCast(pidSymbol, to: PidFn.self)
let seconds = Double(CommandLine.arguments.dropFirst().first ?? "3") ?? 3
let start = Date()
var last: pid_t = -1
while Date().timeIntervalSince(start) < seconds {
    var serial = [UInt32](repeating: 0, count: 2)
    _ = serial.withUnsafeMutableBytes { front($0.baseAddress!) }
    var pid: pid_t = 0
    if serial.withUnsafeBytes({ pidOf($0.baseAddress!, &pid) }) != 0 { pid = -2 }
    if pid != last {
        let click = CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .leftMouseDown)
        let key = CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: .keyDown)
        print(String(format: "%.3f s  pid %d  (último clic hace %.2f s, última tecla hace %.2f s)",
                     Date().timeIntervalSince(start), pid, click, key))
        last = pid
    }
    usleep(5000)
}
