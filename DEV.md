# Dokumentasi Pengembangan & Arsitektur `s`

Dokumen ini memuat detail arsitektur teknis, protokol komunikasi antarmuka, alur PTY, dan panduan pengembangan internal untuk aplikasi **`s`**.

---

## 1. Filosofi & Tujuan Desain

* **Minimalis & Fokus:** Satu huruf, satu fungsi utama. Menjaga agar proses terminal tidak mati saat koneksi terputus dan memungkinkan *resume* secara mulus.
* **Efisiensi Sumber Daya Ekstrem:** Tidak ada *scrollback buffer* yang disimpan di RAM server (berbeda dari `tmux` yang menyimpan ribuan baris di memori).
* **Keamanan Memori:** Ditulis dalam Rust untuk menjamin *memory safety* (mencegah *buffer overflow* atau kebocoran memori).
* **Persistensi Data:** Seluruh output terminal langsung di-dump ke disk (*file-backed buffer*), sehingga riwayat sesi tidak pernah hilang meski klien sedang *detached*.

---

## 2. Arsitektur Inti: File-Backed Buffer

Secara internal, `s` bertindak murni sebagai "pipa aliran" (*stateless*) di level memori, namun persisten di level *file system*.

Setiap sesi `s` memproduksi dua komponen utama di direktori `/tmp/` (dapat dikonfigurasi melalui `$S_SESSION_DIR`):

1. **Unix Domain Socket (`/tmp/s_<nama_sesi>.sock`):** Jalur komunikasi I/O *real-time* asinkron antara Server daemon dan Client frontend.
2. **Log File (`/tmp/s_<nama_sesi>.log`):** Tempat Server membuang (*dump & flush*) semua output dari PTY secara langsung. File ini berfungsi sebagai *scrollback buffer* abadi tanpa menyita RAM server.
3. **PID File (`/tmp/s_<nama_sesi>.pid`):** Menyimpan Process ID dari Server daemon untuk keperluan manajemen sinyal proses (`SIGTERM`).

---

## 3. Komponen Sistem

### A. Mode Server (Background Daemon)
Dijalankan di latar belakang (*background process*) saat sesi baru dibuat:
* **PTY Manager:** Menggunakan `nix::pty::openpty` untuk membuat pasangan master/slave pseudo-terminal, memanggil `setsid()` dan mengakuisisi controlling terminal (`TIOCSCTTY`) sebelum mengeksekusi shell (`$SHELL` / `/bin/bash` / `/bin/sh`).
* **File Logger:** Dedicated thread membaca master PTY, langsung menulis ke file `.log` (dengan `flush` seketika), dan menyebarkan (*broadcast*) chunk data ke channel Tokio.
* **Socket Listener:** Menerima koneksi masuk di socket `.sock`. Setiap klien yang terhubung akan mendapatkan salinan output PTY dan dapat mengirimkan input serta sinyal resize ke master PTY.
* **Cleanup Guard:** Membersihkan file `.sock` dan `.pid` saat proses shell selesai atau menerima sinyal interupsi.

### B. Mode Client (Frontend UI)
Dijalankan langsung di terminal pengguna saat berinteraksi:
* **History Loader:** Saat pertama terhubung, membaca isi file `/tmp/s_<nama_sesi>.log` dan langsung mencetaknya ke layar terminal lokal agar pengguna dapat melihat output selama sesi ditinggalkan (*offline*).
* **Socket Connector:** Menghubungkan stdin/stdout lokal ke Unix Domain Socket server.
* **Raw Mode Forwarder:** Mengaktifkan *Raw Mode* melalui `crossterm` dengan proteksi RAII (*Drop Guard*) agar terminal pengguna tidak rusak saat keluar. Menangkap tombol detach (default `Ctrl + \` atau ASCII `0x1C`) untuk keluar secara aman tanpa mematikan server.
* **Dynamic Window Resizing:** Menerima sinyal `SIGWINCH` dari sistem operasi dan mengirimkan ukuran terminal terbaru ke server untuk dieksekusi via `ioctl(master_fd, TIOCSWINSZ, &ws)`.

---

## 4. Spesifikasi Protokol IPC (Binary Framing)

Komunikasi antara Klien dan Server melalui Unix Domain Socket menggunakan protokol framing biner ringan:

| Tipe Pesan | Header (Byte) | Format Payload | Deskripsi |
| :--- | :---: | :--- | :--- |
| **`MSG_DATA`** | `0x01` | `[len: u16 BE] [payload: bytes]` | Input keyboard pengguna dari klien ke PTY |
| **`MSG_RESIZE`** | `0x02` | `[rows: u16 BE] [cols: u16 BE]` | Sinyal perubahan dimensi terminal (SIGWINCH) |
| **`MSG_DETACH`** | `0x03` | *Tidak ada payload* | Klien memberi tahu server bahwa ia melepas sesi |

*Catatan: Aliran data dari Server ke Klien dikirimkan sebagai raw byte stream langsung dari PTY master untuk performa maksimal tanpa overhead framing.*

---

## 5. Struktur Direktori Kode Sumber

```
s/
├── .github/workflows/
│   └── release.yml     # Workflow auto-tag, build matrix (Linux & macOS), rilis GitHub, deploy Pages
├── src/
│   ├── main.rs         # Entry point CLI & orkestrasi perintah
│   ├── cli.rs          # Parser argumen clap & helper tombol detach
│   ├── client.rs       # Client frontend, raw mode, history replay, SIGWINCH
│   ├── server.rs       # Server daemon, PTY openpty, file logger, socket server
│   ├── session.rs      # Pengelolaan path file, deteksi socket hidup, sanitasi nama
│   └── protocol.rs     # Definisi framing pesan biner (encode/decode) & unit test
├── web/
│   └── index.html      # Landing page untuk n0z0.if.co.id/s
├── install.sh          # One-liner installer shell script
├── Cargo.toml          # Konfigurasi dependensi & profil optimasi binary
├── README.md           # Panduan pengguna (End-User Quickstart)
└── DEV.md              # Dokumentasi arsitektur internal & developer (file ini)
```

---

## 6. Kompilasi & Pengujian

### Prasyarat
* Rust toolchain (stable) 1.70+
* Target POSIX/Unix (Linux atau macOS)

### Perintah Pembangunan

```bash
# Debug build lokal
cargo build

# Release build (dioptimalkan untuk ukuran biner kecil: opt-level='z', lto=true, strip=true)
cargo build --release

# Menjalankan seluruh pengujian unit
cargo test
```

### Cross-Compilation Target
Proyek ini mendukung kompilasi binary mandiri (*static binary*) tanpa dependensi:
* **Linux x86_64 musl:** `cargo build --release --target x86_64-unknown-linux-musl`
* **Linux aarch64 musl:** `cross build --release --target aarch64-unknown-linux-musl`
* **macOS x86_64:** `cargo build --release --target x86_64-apple-darwin`
* **macOS aarch64:** `cargo build --release --target aarch64-apple-darwin`

---

## 7. Status Milestone Implementasi

* [x] **Fase 1 (Core PTY & TTY):** Pembuatan PTY di Rust (`nix::pty::openpty`), eksekusi shell target (`$SHELL` / `/bin/bash`), penyesuaian ukuran terminal (`SIGWINCH` / `TIOCSWINSZ`), dan manipulasi Raw Mode (`crossterm`).
* [x] **Fase 2 (Client-Server IPC):** Pemisahan proses daemon latar belakang dan jembatan komunikasi Unix Domain Socket (`/tmp/s_<sesi>.sock`) berbasis Tokio async.
* [x] **Fase 3 (File-Backed Buffer):** Penulisan log real-time ke `/tmp/s_<sesi>.log` tanpa RAM buffer di server, dan history replay otomatis saat klien attach.
* [x] **Fase 4 (Optimasi & Rilis):** Pembersihan graceful socket/pid pada shutdown/exit shell, unit test lengkap untuk parsing & protokol, serta konfigurasi profil rilis berukuran ultra-kecil (`opt-level = 'z'`, `lto = true`, `panic = 'abort'`, `strip = true`).
