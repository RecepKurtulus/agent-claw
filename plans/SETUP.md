# Projeyi Nasıl Çalıştırırsın

## 1. Önce bir kere kurulum yap (ilk seferde)

Terminali aç ve şunları sırayla çalıştır:

```bash
# 1. Rust cargo'yu PATH'e ekle (her yeni terminalde gerekli, ya da .zshrc'e zaten eklendi)
source ~/.zshrc

# 2. cargo-watch kur (bir kere yeter)
cargo install cargo-watch

# 3. Node bağımlılıklarını kur (bir kere yeter)
pnpm install
```

---

## 2. Projeyi başlat

```bash
# agent-claw klasörüne gir
cd ~/Desktop/vibeclaw/agent-claw

# PATH'i yükle (her yeni terminal açışında)
source ~/.zshrc

# Projeyi çalıştır
pnpm run dev
```

Bu komut aynı anda 2 şey başlatır:
- **[0] Backend** → Rust sunucusu (ilk çalışmada 2-5 dakika derleme yapar)
- **[1] Frontend** → React arayüzü (hemen hazır, `http://localhost:3000`)

---

## 3. İlk açılışta ne görürsün?

```
[1] Proxy connection closed, auto-reconnecting...
[1] Proxy connection closed, auto-reconnecting...
```

**Panikle!** 😄 Bu normal. Frontend hazır ama backend henüz derleniyor.  
`[0]` satırlarında `Finished` veya `Running 'server'` görünce backend de hazır demektir.  
Sonra `http://localhost:3000` açarsın, her şey çalışır.

---

## 4. Sonraki açılışlarda

İkinci açılıştan itibaren backend çok daha hızlı başlar (~10-15 saniye),  
çünkü Rust sadece değişen dosyaları derler.

---

## 5. Bir şeyler bozulursa

| Sorun | Çözüm |
|-------|-------|
| `cargo: command not found` | `source ~/.zshrc` çalıştır |
| `pnpm: command not found` | `npm install -g pnpm` çalıştır |
| `cargo watch: no such command` | `cargo install cargo-watch` çalıştır |
| Backend port hatası | Zaten açık bir terminal var mı? Kapat ve tekrar dene |
| DB hatası | `pnpm run prepare-db` çalıştır |

---

## 6. OpenClaw özelliğini kullanmak için

1. `ANTHROPIC_API_KEY` environment variable'ını set et:
   ```bash
   export ANTHROPIC_API_KEY="sk-ant-..."
   ```
2. Projeyi aç → bir project seç → header'da **🧠 beyin ikonuna** tıkla
3. Ne yapmak istediğini tek cümleyle yaz → "Analiz Et"
4. OpenClaw kodu okur, task'ları kendisi oluşturur
