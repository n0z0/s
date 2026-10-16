# s

Terminal session manager alternatif tmux dan dtach dengan arsitektur file-backed buffer.

Riwayat output terminal langsung dialirkan ke file log di disk (`/tmp`), sehingga proses tetap berjalan di background tanpa memakan RAM untuk riwayat scrollback.

## Instalasi

```bash
curl -fsSL https://n0z0.if.co.id/s/install.sh | sh
```

Atau menggunakan wget:

```bash
wget -qO- https://n0z0.if.co.id/s/install.sh | sh
```

Binary mandiri juga tersedia di [GitHub Releases](https://github.com/n0z0/s/releases).

## Penggunaan

Masuk atau buat sesi baru:

```bash
s main
```

Keluar dari sesi (detach) tanpa mematikan proses:

Tekan `Ctrl + \`

Melihat output log langsung dari terminal lain:

```bash
tail -f /tmp/s_main.log
```

Melihat daftar sesi aktif:

```bash
s list
```

Mematikan sesi:

```bash
s kill main
```

## Opsi

- `s <nama>`: Buat atau sambung ke sesi.
- `s list` / `s ls`: Tampilkan daftar sesi yang aktif.
- `s kill <nama>`: Hentikan sesi.
- `s log <nama>`: Tampilkan isi log sesi (tambahkan `-p` untuk melihat path filenya saja).
- `-s, --shell <path>`: Shell yang digunakan (default: `$SHELL` atau `/bin/sh`).
- `-d, --detach-key <key>`: Ganti kombinasi tombol detach (default: `ctrl-\`, bisa `ctrl-a`, `ctrl-]`, dll).

Untuk dokumentasi teknis dan arsitektur internal, lihat [DEV.md](DEV.md).

## Lisensi

MIT
