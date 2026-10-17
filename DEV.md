# Arsitektur dan Panduan Teknis `s`

Dokumen ini menjelaskan arsitektur internal, alur proses PTY, protokol IPC, dan cara kompilasi aplikasi `s`.

## 1. Konsep Desain

`s` dirancang untuk menjaga proses terminal tetap berjalan saat koneksi terputus dengan konsumsi memori minimal.

Perbedaan utama dengan multiplexer seperti `tmux`:
- `tmux` menyimpan riwayat scrollback ribuan baris di memori RAM server.
- `dtach` tidak menyimpan scrollback sama sekali (hilang saat detach).
- `s` mengalirkan output PTY langsung ke file log di disk (`/tmp/s_<sesi>.log`) tanpa menyimpannya di RAM server. Saat klien tersambung kembali, isi file log dibaca dan dicetak ulang ke layar lokal.

## 2. Berkas Sesi

Secara default, berkas sesi disimpan di `/tmp` (dapat diubah via environment variable `S_SESSION_DIR`):

- `/tmp/s_<nama>.sock`: Unix domain socket untuk komunikasi I/O antara klien dan server.
- `/tmp/s_<nama>.log`: File log append-only tempat output PTY dibuang.
- `/tmp/s_<nama>.pid`: Process ID server untuk manajemen sinyal term.

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
4. Mengirim dimensi terminal awal dan mendengarkan sinyal `SIGWINCH` untuk meneruskan resize ke server via ioctl `TIOCSWINSZ`.
5. Membaca keystroke pengguna: jika mendeteksi byte detach (default `0x1C` / `Ctrl+\`), raw mode dimatikan dan koneksi socket ditutup tanpa mematikan server.

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
