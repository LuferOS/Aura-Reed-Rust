<div align="center">

<img src="https://readme-typing-svg.demolab.com?font=JetBrains+Mono&weight=800&size=34&duration=2800&pause=1000&color=A855F7&center=true&vCenter=true&multiline=true&width=740&height=115&lines=%E2%9A%A1+AURA+REED+RUST+v2.5;%7E21.7+MB+RAM+%E2%80%A2+0.002+ms+%E2%80%A2+100%25+Nativo" alt="Aura Reed Rust" />

<p>
  <a href="https://github.com/LuferOS/Aura-Reed-Rust"><img src="https://img.shields.io/badge/Rust-2021-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" /></a>
  <a href="https://github.com/LuferOS/Baileys-Rust"><img src="https://img.shields.io/badge/Powered%20By-Baileys--Rust-A855F7?style=for-the-badge&logo=whatsapp&logoColor=white" alt="Baileys-Rust" /></a>
  <img src="https://img.shields.io/badge/RAM-%7E21.7%20MB-22C55E?style=for-the-badge&logo=speedtest&logoColor=white" alt="RAM" />
  <img src="https://img.shields.io/badge/Latencia%20Local-0.002%20ms-06B6D4?style=for-the-badge&logo=lightning&logoColor=white" alt="Latency" />
  <img src="https://img.shields.io/badge/SQLite-WAL%20Mode-3B82F6?style=for-the-badge&logo=sqlite&logoColor=white" alt="SQLite" />
  <img src="https://img.shields.io/badge/Termux%20%7C%20Linux-ARM64%20%2F%20x86__64-111827?style=for-the-badge&logo=linux&logoColor=white" alt="Platforms" />
</p>

<p>
  <b>Bot de WhatsApp de alto rendimiento escrito completamente en Rust.</b><br/>
  Diseñado para ofrecer velocidad instantánea, soporte Multi-Bot (SubBots), economía, gacha, descargas multimedia H.264, IA y moderación avanzada consumiendo una fracción mínima de memoria.
</p>

<p>
  <a href="#-benchmark-real-rust-vs-nodejs">Benchmark</a> •
  <a href="#-motor-recomendado-baileys-rust">Baileys-Rust</a> •
  <a href="#-m%C3%B3dulos-y-comandos">Comandos</a> •
  <a href="#-instalaci%C3%B3n-r%C3%A1pida-termux--linux">Instalación</a> •
  <a href="#-arquitectura">Arquitectura</a>
</p>

</div>

---

## ⚡ Benchmark Real: Rust vs Node.js

Mediciones reales ejecutando el mismo bot en producción con la misma base de datos y grupos activos:

| Métrica | 🦀 Aura Reed Rust | 🟨 Aura Bot (Node.js) | Mejora |
| :--- | :---: | :---: | :---: |
| **Consumo de RAM (`VmRSS`)** | **~21.7 MB** | ~1,290 MB (1.29 GB) | **~59x menos RAM** |
| **Latencia de procesamiento local** | **0.002 ms** | ~35–120 ms | **Instantáneo** |
| **Garbage Collector / Picos** | **Ninguno (Zero-GC)** | Picos frecuentes de V8 | **Estabilidad total** |
| **Base de datos** | SQLite WAL + índices + caché acotada | SQLite + cachés en heap JS | **Cero fugas de RAM** |
| **Ideal para** | **Termux, VPS 256MB–512MB, Raspberry Pi** | Servidores dedicados 2GB+ | **Corre en cualquier equipo** |

---

## 🔌 Motor Recomendado: Baileys-Rust

> **¿Vas a desarrollar tu propio bot de WhatsApp en Rust o migrar desde Node.js?**  
> Recomendamos usar **[Baileys-Rust](https://github.com/LuferOS/Baileys-Rust)**, la librería nativa que impulsa este proyecto.

<div align="center">
  <a href="https://github.com/LuferOS/Baileys-Rust">
    <img src="https://img.shields.io/badge/GitHub-LuferOS%2FBaileys--Rust-181717?style=for-the-badge&logo=github&logoColor=white" alt="Baileys-Rust Repo" />
  </a>
</div>

<br/>

* **Protocolo WhatsApp Web nativo en Rust:** sin Node.js, sin Chromium y sin sobrecarga de runtime.
* **Cifrado Signal & Media:** manejo completo de pre-keys, sesiones multi-archivo, carga/descarga cifrada de imágenes, videos, audios, documentos y stickers.
* **Soporte Multi-Instancia (`SubBotManager`):** vinculación por **QR** o **Pairing Code** en tiempo real desde un solo binario.
* **Gestión completa de Grupos y Chats:** metadatos, permisos de admin, enlaces de invitación, mensajes temporales y fotos de perfil.

---

## 🚀 Módulos y Comandos

| Categoría | Comandos principales | Descripción |
| :--- | :--- | :--- |
| **🤖 AI** | `.gpt`, `.gemini`, `.deepseek`, `.grok`, `.copilot` | Consultas rápidas a múltiples modelos de IA sobre HTTP/2 |
| **📥 Download** | `.play`, `.play2`, `.docplay`, `.tiktok`, `.dtk`, `.ig`, `.fb`, `.spotify`, `.sc`, `.x`, `.apk`, `.mf`, `.ssweb` | Descargas con conversión automática a **H.264 + faststart + miniatura** y soporte en documento |
| **🔍 Search** | `.yts`, `.tts`, `.sps`, `.scsearch`, `.pin`, `.lrc` | Búsquedas en YouTube, TikTok, Spotify, SoundCloud, Pinterest y letras |
| **🎨 Sticker** | `.s`, `.brat`, `.bratv`, `.emojimix`, `.emojivid`, `.qc`, `.spack`, `.setmeta`, `.toimg` | Stickers estáticos y animados, citas, packs y conversión a imagen |
| **🛡️ Group** | `.kick`, `.add`, `.promote`, `.demote`, `.open`, `.close`, `.link`, `.tagall`, `.tag`, `.warn`, `.mute`, `.antilink`, `.antitoxic`, `.antispam`, `.onlyadmin`, `.tempmsg`, `.restrictcmd`, `.disable`/`.enable` | Moderación automática en tiempo real y administración total |
| **💰 Economy** | `.bal`, `.baltop`, `.dep`, `.with`, `.pay`, `.daily`, `.weekly`, `.work`, `.mine`, `.hunt`, `.crime`, `.adventure`, `.cf`, `.ruleta`, `.ppt`, `.steal` | Economía completa en SQLite con rachas, banco y cooldowns |
| **🎴 Gacha** | `.rw`, `.claim`, `.harem`, `.winfo`, `.wimage`, `.vote`, `.sell`, `.wshop`, `.buyc`, `.trade`, `.accepttrade`, `.givechar`, `.robwaifu` | Catálogo de personajes embebido en binario (0 KB heap) + tienda y tradeo |
| **👤 Profile** | `.perfil`, `.pfp`, `.lvl`, `.eboard`, `.setmyname`, `.setdesc`, `.setgenre`, `.cumple`, `.idioma`, `.marry`, `.divorce` | Perfiles con foto, edad automática, nivel Aura y matrimonios |
| **🎮 Games & Fun** | `.ahorcado`, `.ruletarusa`, `.aviator`, `.slots`, `.dado`, `.8bal`, `.ship`, `.demandar`, `.top`, `.cuernos` | Juegos interactivos en el chat y comandos sociales |
| **✨ Interacción & NSFW** | `.hug`, `.kiss`, `.slap`, `.pat`, `.bonk` (+35), `.r34`, `.xnxx`, `.xvideos` | Reacciones anime SFW/NSFW (controladas por `.nsfw on/off`) |
| **⚙️ System & Socket** | `.menu`, `.info`, `.ram`, `.ping`, `.snipe`, `.code`, `.bots`, `.logout`, `.setbanner`, `.setbotname`, `.bc`, `.autoadmin`, `.restart` | Diagnóstico real de RAM/hilos, gestión de SubBots y personalización |

---

## 🛠️ Instalación Rápida (Termux / Linux)

### 1. Requisitos previos

En **Termux (Android)**:
```bash
pkg update && pkg upgrade -y
pkg install rust git ffmpeg clang pkg-config -y
```

En **Ubuntu / Debian**:
```bash
sudo apt update && sudo apt install -y build-essential git ffmpeg pkg-config curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Clonar `Baileys-Rust` y `Aura-Reed-Rust`

Ambos repositorios deben quedar en el mismo directorio padre (`$HOME`):

```bash
cd "$HOME"
git clone https://github.com/LuferOS/Baileys-Rust.git
git clone https://github.com/LuferOS/Aura-Reed-Rust.git
```

### 3. Compilar en modo Release

```bash
cd "$HOME/Aura-Reed-Rust"
cargo build --release
```

### 4. Ejecutar el Bot

Con **Código QR**:
```bash
cargo run --release -- --debug
```

Con **Código de Vinculación (Pairing Code)**:
```bash
cargo run --release -- --pair 573001234567 --debug
```

Opciones disponibles:
* `--pair <numero>`: vincula usando código de 8 dígitos sin escanear QR.
* `--threads <n>`: ajusta el número de workers de Tokio (por defecto `2` para mínimo consumo de RAM).
* `--auth <ruta>`: directorio personalizado para la sesión principal.
* `--debug`: activa telemetría de tiempos, errores y llamadas en consola.

---

## 📂 Arquitectura

```text
Aura-Reed-Rust/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── context.rs
│   ├── db.rs
│   ├── handler.rs
│   ├── ui.rs
│   ├── lang.rs
│   ├── snipe.rs
│   ├── debug.rs
│   └── cmd/
│       ├── ai.rs
│       ├── download.rs
│       ├── economy.rs
│       ├── funy.rs
│       ├── gacha.rs
│       ├── games.rs
│       ├── group.rs
│       ├── interaccion.rs
│       ├── profile.rs
│       ├── search.rs
│       ├── socket.rs
│       ├── stalk.rs
│       ├── sticker.rs
│       ├── system.rs
│       └── utils.rs
├── data/
│   └── Aura.db
├── sessions/
└── cache/
```

---

## 📄 Licencia y Créditos

* **Desarrollador:** [LuferOS](https://github.com/LuferOS)
* **Motor WhatsApp:** [Baileys-Rust](https://github.com/LuferOS/Baileys-Rust)
* **Licencia:** Apache-2.0

<div align="center">
  <img src="https://img.shields.io/badge/Hecho%20con-Rust%20%26%20Baileys--Rust-A855F7?style=flat-square" alt="Made with Rust" />
</div>