# Arsitektur dan Panduan Teknis `s`

Dokumen ini menjelaskan arsitektur internal, alur proses PTY, protokol IPC, dan cara kompilasi aplikasi `s`.

## 1. Konsep Desain

`s` dirancang untuk menjaga proses terminal tetap berjalan saat koneksi terputus dengan konsumsi memori minimal.

Perbedaan utama dengan multiplexer seperti `tmux`:
- `tmux` menyimpan riwayat scrollback ribuan baris di memori RAM server.
- `dtach` tidak menyimpan scrollback sama sekali (hilang saat detach).
- `s` mengalirkan output PTY langsung ke file log di disk (`/tmp/s_<sesi>.log`) tanpa menyimpannya di RAM server. Saat klien tersambung kembali, isi file log dibaca dan dicetak ulang ke layar lokal.

## 2. Berkas Sesi

Secara default, berkas sesi disimpan di `/tmp` (dapat diubah via environment variable `S_SESSION_DIR`). Folder tersebut tidak dibuat otomatis, dan klien maupun server hanya mencari sesi di folder yang ditunjuk `S_SESSION_DIR` saat itu, jadi nilainya harus sama di semua terminal. Path di bawah memakai nilai default:

- `/tmp/s_<nama>.sock`: Unix domain socket untuk komunikasi I/O antara klien dan server.
- `/tmp/s_<nama>.log`: File log append-only tempat output PTY dibuang.
- `/tmp/s_<nama>.pid`: Process ID server untuk manajemen sinyal term.

Karena nama berkas selalu diturunkan dari nama sesi, `s list` tidak menampilkan path socket dan log. Perintah itu hanya menampilkan kolom `SESSION`, `STATUS`, `PID`, dan `LOG SIZE`:

- `STATUS` bernilai `Active` jika `.sock` ada dan server menerima koneksi (`is_socket_alive()` di `src/session.rs`), atau `Stale` jika `.sock` ada tapi koneksi gagal.
- `PID` dibaca dari `.pid` (`-` jika tidak ada), dan `LOG SIZE` dari ukuran `.log`.
- Path log tetap bisa dilihat lewat `s log <nama> -p`.

## 3. Alur Kerja Komponen

### Server
Server dijalankan di background secara otomatis saat sesi pertama kali dibuat:
1. Membuka pseudo-terminal (PTY) via `openpty`.
2. Menjalankan shell (`$SHELL` / `/bin/sh`) di slave PTY dengan session leader baru (`setsid`) dan controlling terminal (`TIOCSCTTY`).
3. Thread dedicated membaca output dari master PTY dan langsung melakukan write + flush ke file `.log`, lalu menyiarkan chunk data ke seluruh klien yang terhubung melalui channel Tokio broadcast.
4. Menerima input dari klien melalui socket dan meneruskannya ke master PTY.
5. Saat shell keluar atau menerima SIGTERM, file `.sock` dan `.pid` dihapus. File `.log` tetap dipertahankan.

### Klien
Klien menangani antarmuka terminal pengguna:
1. Memeriksa apakah socket sesi aktif. Jika belum, men-spawn proses server di background dan menunggu socket siap.
2. Membaca isi file `.log` secara instan menggunakan fast tail (default: ~256 KB terakhir dengan penyejajaran baris/newline baru, atau `-n <baris>`, atau `--all` untuk membaca penuh) dan mencetaknya ke stdout lokal.
3. Mengaktifkan raw mode pada terminal lokal pengguna via `crossterm`.
4. Mengirim dimensi terminal awal dan mendengarkan sinyal `SIGWINCH` untuk meneruskan resize ke server via ioctl `TIOCSWINSZ`. Saat attach, klien lebih dulu mengirim ukuran sementara (lebar dikurangi 1 kolom), lalu 50 ms kemudian ukuran sebenarnya. Ini perlu karena kernel hanya mengirim `SIGWINCH` ke aplikasi jika ukuran benar-benar berubah; tanpa trik ini aplikasi layar penuh (Claude Code, agy, vim) tidak menggambar ulang layarnya setelah re-attach.
5. Membaca keystroke pengguna dan mencari tombol detach (default `0x1C` / `Ctrl+\`) lewat `find_detach_key()` di `src/client.rs`. Jika ditemukan, byte sebelum tombol detach tetap diteruskan, pesan Detach dikirim, lalu koneksi socket ditutup tanpa mematikan server. Lihat [Deteksi Tombol Detach](#deteksi-tombol-detach).
6. Saat keluar (detach maupun sesi tertutup), klien menulis `TERMINAL_RESET` ke stdout sebelum mematikan raw mode, supaya terminal lokal kembali normal. Lihat [Reset Terminal](#reset-terminal).

### Deteksi Tombol Detach

Aplikasi TUI modern menyalakan mode keyboard yang lebih kaya. Setelah itu terminal tidak lagi mengirim `Ctrl+\` sebagai byte `0x1C`, sehingga klien mengenali tiga bentuk:

| Mode terminal | Byte yang dikirim untuk `Ctrl+\` |
|---|---|
| Normal | `0x1C` |
| kitty keyboard protocol | `ESC [ 92 ; 5 u` (bisa disertai alternate key / event type, mis. `ESC [ 92:124 ; 5:1 u`) |
| xterm `modifyOtherKeys` | `ESC [ 27 ; 5 ; 92 ~` |

Aturannya:
- Kode tombol diturunkan dari byte detach: `detach_byte + 0x40`, lalu huruf kecil (`0x1C` → `\` = 92, `0x01` → `a` = 97). Jadi tombol hasil `-d` juga dikenali.
- Nilai modifier dianggap Ctrl jika `(mods - 1)` hanya berisi bit Ctrl (4), boleh ditambah Caps Lock (64) atau Num Lock (128).
- Event key-release kitty (event type `3`) diabaikan.
- Keterbatasan: sequence yang terpotong di dua kali `read()` tidak terdeteksi; tekan sekali lagi.

### Reset Terminal

Aplikasi yang di-attach bisa mengubah state terminal lokal. Saat klien keluar, `TERMINAL_RESET` (di `src/client.rs`) mengembalikannya:

- Pop flag kitty keyboard (`CSI < 99 u`) dan matikan `modifyOtherKeys` (`CSI > 4 ; 0 m`).
- Matikan bracketed paste dan mouse reporting (`?2004`, `?1000`, `?1002`, `?1003`, `?1006`).
- Keluar dari alternate screen (`?1049l`).
- Reset atribut SGR, scroll region (`CSI r`), origin mode, autowrap, cursor keys dan keypad mode, lalu tampilkan kursor.
- Bersihkan layar dan pindahkan kursor ke pojok kiri atas.

Tanpa reset ini, shell bisa hanya memperbarui satu baris setelah detach (kemungkinan besar karena scroll region yang tertinggal). Efek sampingnya, tampilan terakhir aplikasi hilang dari layar (di kebanyakan terminal masih bisa di-scroll ke atas).

Karena reset ini, aplikasi tidak tahu mode keyboard-nya sudah dimatikan. Setelah re-attach, kombinasi seperti Shift+Enter mungkin belum jalan sampai aplikasi menyalakan ulang modenya (mis. setelah resize atau restart aplikasi).

Semua perilaku di atas ada di klien, jadi berlaku untuk sesi yang sudah berjalan begitu binary `s` diperbarui. Server lama tidak perlu di-restart karena format protokol tidak berubah.

## 4. Protokol Komunikasi (Klien ke Server)

Pesan dari klien ke server menggunakan framing sederhana:

- Data input (`0x01`): `[0x01, len_u16_be, payload...]`
- Resize terminal (`0x02`): `[0x02, rows_u16_be, cols_u16_be]`
- Detach (`0x03`): `[0x03]`

Output dari server ke klien dikirim dalam bentuk raw stream tanpa framing tambahan agar tidak ada overhead.

## 5. Kompilasi dan Build

### Build Lokal

```bash
cargo build --release
```

Binary hasil build release otomatis dioptimasi untuk ukuran file (`opt-level = "z"`, LTO, strip symbols).

### Menjalankan Test

```bash
cargo test
```

### Target Binary

- Linux x86_64: `cargo build --release --target x86_64-unknown-linux-musl`
- Linux aarch64: `cross build --release --target aarch64-unknown-linux-musl`
- macOS x86_64: `cargo build --release --target x86_64-apple-darwin`
- macOS aarch64: `cargo build --release --target aarch64-apple-darwin`
