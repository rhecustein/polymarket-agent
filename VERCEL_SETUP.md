# 🚀 Panduan Setup Vercel - Polymarket Agent

## Langkah 1: Persiapan Repository

Pastikan file-file ini ada di **root directory** (sudah ✅):
- `index.html` - Landing page
- `vercel.json` - Konfigurasi Vercel

## Langkah 2: Import Project ke Vercel

### A. Login ke Vercel
1. Buka [vercel.com](https://vercel.com)
2. Login dengan akun GitHub Anda

### B. Import Repository
1. Klik tombol **"Add New..."** → **"Project"**
2. Pilih repository: `rhecustein/polymarket-agent`
3. Klik **"Import"**

## Langkah 3: Configure Project Settings

### Framework Preset
- **Framework Preset**: `Other` atau `Static Site`
- **Jangan** pilih Next.js, React, atau framework lainnya

### Build & Output Settings

**PENTING**: Kosongkan semua build commands!

```
Build Command: (kosongkan atau isi: echo "No build needed")
Output Directory: . (titik saja, artinya root)
Install Command: (kosongkan)
```

### Root Directory
- **Root Directory**: `.` (titik saja, bukan `landing-page`)
- Pastikan TIDAK mengarah ke subdirectory

## Langkah 4: Environment Variables (Opsional)

Jika Anda ingin menambahkan environment variables untuk backend (nanti):
1. Klik tab **"Environment Variables"**
2. Tambahkan:
   - `CLAUDE_API_KEY`
   - `GEMINI_API_KEY`
   - `GAMMA_API`
   - dll.

⚠️ **JANGAN** commit `.env` ke repository!

## Langkah 5: Deploy

1. Klik tombol **"Deploy"**
2. Tunggu proses deployment selesai (1-2 menit)
3. Vercel akan memberikan URL deployment

## Langkah 6: Verifikasi

Setelah deployment selesai:
1. Buka URL yang diberikan Vercel
2. Landing page seharusnya muncul (tidak ada error 404)

## Troubleshooting Error 404

Jika masih mendapat error 404:

### Cek 1: Root Directory
1. Buka **Settings** → **General**
2. Scroll ke **Root Directory**
3. Pastikan nilai: `.` (kosongkan atau isi titik)
4. **Save** dan re-deploy

### Cek 2: Build Settings
1. Buka **Settings** → **General**
2. Scroll ke **Build & Development Settings**
3. Pastikan:
   - **Framework Preset**: Other
   - **Build Command**: (kosongkan)
   - **Output Directory**: `.` atau kosongkan
4. **Save** dan re-deploy

### Cek 3: Vercel.json
Pastikan `vercel.json` di root menggunakan format ini:
```json
{
  "rewrites": [
    {
      "source": "/(.*)",
      "destination": "/index.html"
    }
  ]
}
```

### Cek 4: File Structure
Pastikan struktur di GitHub:
```
polymarket-agent/
├── index.html          ← Harus ada di root!
├── vercel.json         ← Harus ada di root!
├── README.md
├── agent/
├── proxy/
└── ...
```

## Langkah Manual Re-Deploy

Jika perubahan tidak terdeteksi:

### Via Vercel Dashboard:
1. Buka project Anda di Vercel Dashboard
2. Klik tab **"Deployments"**
3. Klik titik tiga (•••) di deployment terakhir
4. Pilih **"Redeploy"**

### Via Git:
```bash
# Buat perubahan kecil untuk trigger deployment
git commit --allow-empty -m "Trigger Vercel deployment"
git push origin main
```

## Konfigurasi Advanced (Opsional)

### Custom Domain
1. Buka **Settings** → **Domains**
2. Klik **"Add"**
3. Masukkan domain Anda
4. Ikuti instruksi DNS

### Branch Deployment
- **Production Branch**: `main`
- **Preview Branches**: semua branch lain otomatis mendapat preview URL

## Hasil Akhir

Setelah setup berhasil:
- ✅ URL: https://polymarket-agent-three.vercel.app
- ✅ Landing page muncul
- ✅ Tidak ada error 404
- ✅ Auto-deploy setiap push ke GitHub

## Support

Jika masih ada masalah:
1. Cek **Deployment Logs** di Vercel Dashboard
2. Pastikan commit terbaru sudah di-push ke GitHub
3. Coba redeploy manual

---

**Dibuat**: 2026-02-13
**Last Updated**: 2026-02-13
