# s
TMUX and DTACH Alternatif

[![Release and Deploy](https://github.com/n0z0/s/actions/workflows/release.yml/badge.svg)](https://github.com/n0z0/s/actions/workflows/release.yml)
[![Website](https://img.shields.io/badge/website-n0z0.if.co.id%2Fs-blue)](https://n0z0.if.co.id/s)

### ⚡ Quick Install (One-Liner)

Install langsung ke user path (`~/.local/bin` atau `/usr/local/bin`):

```bash
curl -fsSL https://n0z0.if.co.id/s/install.sh | sh
```

Atau menggunakan `wget`:

```bash
wget -qO- https://n0z0.if.co.id/s/install.sh | sh
```

---

PROJECT BRIEF: Aplikasi `s`

## 1. Ringkasan Proyek

**`s`** adalah sebuah aplikasi *terminal session manager* super ringan yang ditulis dalam bahasa Rust. Proyek ini lahir sebagai jalan tengah antara `tmux` (yang kaya fitur namun memakan banyak memori) dan `dtach` (yang sangat ringan namun tidak memiliki riwayat layar/*scrollback*).

Solusi inovatif yang ditawarkan `s` adalah penerapan arsitektur **File-Backed Buffer**. Aplikasi ini memisahkan tugas pengelolaan riwayat layar dari RAM memori ke penyimpanan disk, memastikan jejak sumber daya (CPU & RAM) tetap mendekati nol di sisi server.

## 2. Filosofi & Tujuan

* **Minimalis & Fokus:** Satu huruf, satu fungsi utama. Menjaga agar proses terminal tidak mati saat koneksi terputus dan memungkinkan *resume* secara mulus.
* **Efisiensi Sumber Daya Ekstrem:** Tidak ada *scrollback buffer* yang disimpan di RAM server.
* **Keamanan Memori:** Ditulis dalam Rust untuk menjamin *memory safety* (mencegah *buffer overflow* atau kebocoran memori) yang sering menjadi isu pada aplikasi sistem berbasis C.
* **Persistensi Data:** Tidak ada output yang hilang meskipun klien sedang tidak terhubung (*detached*).

## 3. Arsitektur Inti: File-Backed Buffer

Berbeda dengan `tmux` yang menahan ribuan baris teks di dalam RAM (stateful), `s` bertindak murni sebagai "pipa aliran" (stateless) di level memori, namun persisten di level *file system*.

Setiap sesi `s` akan memproduksi dua komponen di direktori `/tmp/` (atau direktori yang dikonfigurasi):

1. **Unix Domain Socket (`/tmp/s_<nama_sesi>.sock`):** Jalur komunikasi I/O *real-time* antara Server dan Client.
2. **Log File (`/tmp/s_<nama_sesi>.log`):** Tempat Server membuang (*dump*) semua output dari PTY secara langsung. File ini berfungsi sebagai *scrollback buffer* abadi.

## 4. Mode Operasi & Komponen

### A. Mode Server (Background / Daemon)

Dijalankan secara tidak terlihat (*forked*) jika sesi baru dibuat.

* **PTY Manager:** Melakukan *forking* dan membuat *pseudo-terminal* (PTY) baru untuk mengeksekusi *shell* target (mis. `bash` atau `zsh`).
* **File Logger:** Membaca aliran *stdout/stderr* dari PTY dan langsung melakukan operasi *append* ke file `.log`.
* **Socket Listener:** Mendengarkan koneksi masuk di `.sock`. Jika Client terhubung, Server akan melakukan *mirroring* output PTY ke socket dan meneruskan input dari socket ke PTY.

### B. Mode Client (Frontend / User Interface)

Dijalankan saat pengguna melakukan interaksi dengan terminal.

* **History Loader:** Saat pertama terhubung, Client akan membaca isi `/tmp/s_<nama_sesi>.log` dan mencetaknya ke layar lokal (memungkinkan pengguna melihat apa yang terjadi selama mereka *offline*).
* **Socket Connector:** Menyambung ke `/tmp/s_<nama_sesi>.sock` untuk mengambil alih aliran I/O *real-time*.
* **Raw Mode Forwarder:** Mengambil alih terminal lokal menjadi *Raw Mode* (menangkap setiap *keystroke* termasuk `Ctrl+C`, tombol panah) dan merutekannya langsung ke Server via socket. Client juga mendengarkan kombinasi tombol *detach* (misal: `Ctrl+\`) untuk keluar dengan aman.

## 5. Teknologi & *Dependencies* (Rust Ecosystem)

Proyek ini akan dikembangkan menggunakan ekosistem `cargo` dengan *library* utama berikut:

* **`tokio`**: Inti dari sistem I/O asinkron. Sangat penting untuk menangani aliran baca/tulis antara Socket, File, dan PTY tanpa *blocking*.
* **`nix`**: Interaksi sistem operasi tingkat rendah (POSIX) untuk pembuatan *Unix Domain Sockets*, pengaturan PTY, manajemen *signals*, dan *forking* proses.
* **`crossterm`**: Untuk memanipulasi terminal lokal pengguna (masuk dan keluar dari *Raw Mode* secara aman).
* **`clap`**: Untuk *parsing* argumen *command-line interface* (CLI).

## 6. Alur Kerja Pengguna (User Experience)

1. **Membuat/Masuk ke Sesi:**
```bash
s main

```


*Logika:* Jika sesi `main` belum ada, `s` membuat server di *background*, menginisialisasi `.log` dan `.sock`, lalu Client masuk. Jika sudah ada, Client membaca `.log` lama, lalu menempel (*attach*) ke `.sock`.
2. **Melepas Sesi (Detach):**
Pengguna menekan `Ctrl + \` (atau kombinasi lain yang disepakati). Client akan menghentikan *Raw Mode*, menutup koneksi ke `.sock`, dan mengembalikan pengguna ke terminal asli. Server tetap hidup.
3. **Mengintip Sesi (Eksternal):**
Karena riwayat disimpan sebagai *plain text*, pengguna bisa melihat log tanpa harus masuk ke sesi:
```bash
tail -f /tmp/s_main.log

```



## 7. Status Milestone & Fitur Selesai

* [x] **Fase 1 (Core PTY & TTY):** Pembuatan PTY di Rust (`nix::pty::openpty`), eksekusi shell (`$SHELL` / `/bin/bash`), penyesuaian ukuran terminal (`SIGWINCH` / `TIOCSWINSZ`), dan manipulasi Raw Mode (`crossterm`).
* [x] **Fase 2 (Client-Server IPC):** Pemisahan proses daemon di latar belakang dan jembatan komunikasi Unix Domain Socket (`/tmp/s_<sesi>.sock`) berbasis Tokio async.
* [x] **Fase 3 (File-Backed Buffer):** Penulisan log real-time ke `/tmp/s_<sesi>.log` tanpa RAM buffer di server, dan history replay otomatis saat klien attach.
* [x] **Fase 4 (Optimasi & Rilis):** Pembersihan graceful socket/pid pada shutdown/exit shell, unit test lengkap untuk parsing & protokol, serta konfigurasi profil rilis berukuran ultra-kecil (`opt-level = 'z'`, `lto = true`, `panic = 'abort'`, `strip = true`).

## 8. Panduan Kompilasi & Penggunaan

### Kompilasi

```bash
# Debug build
cargo build

# Release build (optimasi ukuran binary biner ultra-ringan)
cargo build --release

# Menjalankan unit test
cargo test
```

### Panduan Perintah CLI

```bash
# 1. Membuat atau masuk ke sesi baru/lama (default)
s main

# 2. Menggunakan shell tertentu (opsional)
s main --shell /bin/zsh

# 3. Menyesuaikan tombol detach (default: Ctrl+\)
s main -d "ctrl-a"

# 4. Melihat daftar sesi yang aktif
s list
# atau alias singkat:
s ls

# 5. Menghentikan sesi yang sedang berjalan
s kill main

# 6. Melihat letak path file scrollback log
s log main --path

# 7. Memantau output sesi dari terminal lain tanpa attach
tail -f /tmp/s_main.log
```

