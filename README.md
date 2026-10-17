# s

Session manager terminal sederhana. Mirip `dtach`, tapi output PTY otomatis dicatat ke file (`/tmp/s_<sesi>.log`) biar riwayat terminal nggak hilang waktu detach dan server nggak boros RAM.

## Install

```bash
curl -fsSL https://n0z0.if.co.id/s/install.sh | sh
```

Atau download binary langsung di [Releases](https://github.com/n0z0/s/releases).

## Contoh Pakai

Bikin atau masuk ke sesi:
```bash
s kerja
```

Detach (keluar tanpa matiin proses):
Tekan `Ctrl+\`

Intip output sesi dari terminal lain:
```bash
tail -f /tmp/s_kerja.log
```

Cek sesi yang lagi jalan:
```bash
s list
```

Matiin sesi:
```bash
s kill kerja
```

## Opsi CLI

```
s <nama>                 Bikin atau sambung ke sesi (replay instan ~64KB terakhir)
s <nama> -n <baris>      Sambung ke sesi dengan N baris riwayat terakhir (0 = tanpa replay)
s <nama> --all           Sambung ke sesi dengan memuat seluruh log dari awal
s list (atau s ls)       List sesi aktif
s kill <nama>            Stop sesi
s log <nama> [-p]        Lihat isi log (atau path filenya)
-s, --shell <shell>      Pilih shell (default: $SHELL / /bin/sh)
-d, --detach-key <key>   Ganti tombol detach (default: ctrl-\)
-n, --lines <baris>      Jumlah baris riwayat saat attach
--all                    Muat seluruh isi log riwayat tanpa batas
```

Detail teknis dan arsitektur ada di [DEV.md](DEV.md).

## Lisensi

MIT
