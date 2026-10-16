# s — Terminal Session Manager

[![Release and Deploy](https://github.com/n0z0/s/actions/workflows/release.yml/badge.svg)](https://github.com/n0z0/s/actions/workflows/release.yml)
[![Latest Release](https://img.shields.io/github/v/release/n0z0/s?color=blue)](https://github.com/n0z0/s/releases/latest)
[![Website](https://img.shields.io/badge/website-n0z0.if.co.id%2Fs-blue)](https://n0z0.if.co.id/s)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**`s`** adalah *terminal session manager* super ringan yang ditulis dalam bahasa Rust. Proyek ini hadir sebagai jalan tengah antara `tmux` (yang kaya fitur namun memakan banyak memori) dan `dtach` (yang ringan namun kehilangan riwayat layar saat *detach*).

Dengan arsitektur **File-Backed Buffer**, riwayat layar (*scrollback*) langsung disimpan ke disk (`/tmp/s_<nama_sesi>.log`) alih-alih menumpuk di RAM server. Proses di server tetap hidup saat koneksi terputus dan dapat di-*resume* secara mulus dengan penggunaan memori mendekati nol.

---

## ⚡ Instalasi Cepat (One-Liner)

Pasang langsung ke *user path* Anda (`~/.local/bin` atau `/usr/local/bin`):

```bash
curl -fsSL https://n0z0.if.co.id/s/install.sh | sh
```

Atau menggunakan `wget`:

```bash
wget -qO- https://n0z0.if.co.id/s/install.sh | sh
```

---

## 🚀 Panduan Cepat (Quickstart)

### 1. Membuat atau Masuk ke Sesi
Jalankan `s` diikuti nama sesi:
```bash
s main
```
* Jika sesi `main` belum ada, `s` otomatis membuat server di latar belakang dan memasukkan Anda ke dalam sesi.
* Jika sesi sudah ada, `s` akan memuat riwayat layar lama lalu menyambungkan Anda kembali (*attach*).

### 2. Melepas Sesi (Detach)
Untuk keluar dari sesi tanpa mematikan proses/pekerjaan yang sedang berjalan di dalamnya:
```
Tekan tombol: Ctrl + \
```
Terminal lokal Anda akan kembali normal, sementara proses di dalam server tetap berjalan.

### 3. Memantau Output dari Luar Sesi
Karena riwayat disimpan langsung di filesystem, Anda dapat mengintip output secara langsung dari terminal lain tanpa perlu masuk (*attach*):
```bash
tail -f /tmp/s_main.log
```

### 4. Melihat Daftar Sesi Aktif
```bash
s list
# atau alias singkat:
s ls
```

### 5. Menghentikan Sesi
Untuk mematikan sesi terminal secara permanen:
```bash
s kill main
```

---

## 📖 Perintah & Opsi CLI

| Perintah | Contoh | Deskripsi |
| :--- | :--- | :--- |
| `s <nama>` | `s dev` | Membuat sesi baru atau masuk ke sesi yang sudah ada (default) |
| `s list` / `s ls` | `s list` | Menampilkan seluruh sesi aktif beserta PID, status, dan ukuran log |
| `s kill <nama>` | `s kill dev` | Menghentikan sesi yang sedang berjalan dan membersihkan socket |
| `s log <nama>` | `s log dev` | Mencetak isi file scrollback log dari sesi |
| `s log <nama> -p` | `s log dev -p` | Menampilkan lokasi path file log (`/tmp/s_dev.log`) |

### Opsi Tambahan

* **Menentukan Shell Khusus:**
  ```bash
  s main --shell /bin/zsh
  ```
* **Menyesuaikan Tombol Detach:**
  Default kombinasi tombol adalah `Ctrl + \`. Anda bisa mengubahnya sesuai kebutuhan:
  ```bash
  s main -d "ctrl-a"
  s main -d "ctrl-]"
  ```

---

## 🛠️ Dokumentasi Pengembang & Arsitektur

Untuk detail arsitektur internal *File-Backed Buffer*, spesifikasi protokol biner IPC, panduan kompilasi dari sumber (*source code*), dan alur CI/CD, silakan baca [DEV.md](DEV.md).

---

## Lisensi

Proyek ini dilisensikan di bawah lisensi [MIT](LICENSE).
