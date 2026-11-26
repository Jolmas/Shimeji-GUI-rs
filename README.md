# shimeji-gui-rs

GUI de escritorio en Rust para gestionar personajes de `wl_shimeji` mediante `shimejictl`.  
Construida con `eframe/egui`, orientada a Linux Wayland/KDE Plasma.

![image](image.png)

## Funciones

### Personajes
- Lista de prototipos con búsqueda, miniatura y vista previa.
- **Invocar (Summon)**: cantidad, posición X/Y, comportamiento inicial y entorno destino.
- **Descartar (Dismiss)**: un personaje, mismo tipo, otros, pantalla actual o todos (con confirmación).
- **Comportamiento y rescate**: cambiar comportamiento del personaje seleccionado y restaurar ventanas.
- **Gestión**: agregar/importar `.wlshm`, exportar personaje a `.zip` (selección de carpeta destino), eliminar personaje (con confirmación).

### Conversor
- Convierte paquetes desde **ZIP** o **directorio** a formato `.wlshm`.
- Permite elegir directorio de salida.
- Opción de importar el paquete convertido al finalizar.

### Entornos / Inspector
- Visualiza entornos detectados y sus datos (ID, nombre, tamaño, mascotas).
- Permite seleccionar entorno objetivo para acciones.

### Ajustes
- **Idioma**: Español / English.
- **Tema**: Claro / Oscuro (se guarda y se carga al iniciar).
- Configuración de sesión/plugins e interacción con ventanas (IE mode), según soporte de `wl_shimeji`.
- Autostart y opciones de servicio.

### Sistema
- Icono de bandeja con acciones rápidas.
- Cerrar la ventana con la «X» (o Alt+F4) cierra de verdad el proceso de la GUI, así que su icono desaparece del dock/taskbar; el proceso anfitrión de la bandeja sigue vivo.
- **Mostrar / Ajustes** en la bandeja vuelve a lanzar la ventana; **Salir/Quit** termina la aplicación.
- Funciona con cualquier compositor (Wayland o X11): no depende de atajos propietarios ni de ventanas minimizadas.
- Registro/log de salida de comandos.
- Diálogos de archivo con soporte nativo en KDE (`kdialog`/`zenity`) y fallback.

## Requisitos

- Linux (probado para Wayland/KDE Plasma).
- `shimejictl` disponible en `PATH`.
- Rust estable para compilar (`edition 2024`).

## Compilar y ejecutar

```bash
cargo build --release
cargo run
```

Binario de release:

```bash
./target/release/shimeji-gui-rs
```

## Compatibilidad

La app mantiene compatibilidad con rutas y formatos existentes de `wl_shimeji`:

- `~/.config/wl_shimeji`
- `~/.local/share/wl_shimeji/shimeji-overlayd.conf`
- Archivos `*-support.conf`
- `ie_mode`, autostart/desktop, `shimejictl`, paquetes `.wlshm`
