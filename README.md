# s
TMUX and DTACH Alternatif

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



## 7. Fokus Fase Pengembangan (Milestones)

* **Fase 1 (Core PTY & TTY):** Berhasil membuat PTY di Rust, menjalankan `bash`, dan menghubungkannya dengan terminal lokal (*Raw Mode*).
* **Fase 2 (Client-Server IPC):** Memisahkan proses menjadi daemon dan mengimplementasikan jembatan komunikasi menggunakan *Unix Domain Socket*.
* **Fase 3 (File-Backed Buffer):** Mengimplementasikan penulisan log secara *real-time* di sisi Server dan pembacaan log awal di sisi Client saat *attach*.
* **Fase 4 (Optimasi & Rilis):** Pembersihan kode, penanganan interupsi sinyal (memastikan socket file terhapus jika server benar-benar dimatikan), dan optimasi ukuran *binary* (`opt-level = 'z'`, `lto = true`).
