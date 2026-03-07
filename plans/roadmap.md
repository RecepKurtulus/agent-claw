# OpenClaw — Yol Haritası

## Vizyon
Vibe Kanban'ın üstüne oturan, düşünen bir PM katmanı.
İnsan sadece prompt yazar ve son onayı verir. Geri kalanı OpenClaw yönetir.

```
Sen → "Kullanıcı panelini yeniden yaz"
         ↓
OpenClaw → kodu analiz eder → ticket'ları üretir → dependency sırasını belirler
         ↓
Agent'lar sırayla çalışır → test döngüsü → hata varsa agent'a geri gider
         ↓
Sen → diff'i incele → onayla → merge
```

---

## Tamamlanan: Faz 0 — Temel Altyapı ✅

- `openclaw-planner`, `openclaw-orchestrator`, `openclaw-qa` crate'leri
- DB tabloları: oc_plans, oc_plan_tasks, oc_task_dependencies, oc_orchestration_runs, oc_task_run_state, oc_qa_runs, oc_qa_results
- REST API endpoint'leri (/api/openclaw/...)
- DAG (Directed Acyclic Graph) dependency mantığı
- Test komut tespiti (Rust/Node/Python/Go/Ruby/Java...)
- QA retry döngüsü yapısı

---

## Faz 1 — Event Wiring (Sistemi Canlı Hale Getir) ✅

**Hedef:** Orchestrator gerçek zamanlı çalışsın. Agent bitince zincirleme tepkiler otomatik tetiklensin.

### 1.1 ExecutionProcess Tamamlanma Hook'u ✅
- `services/container.rs` içinde EP status `Completed/Failed` olunca
- `openclaw-orchestrator::on_task_completed()` çağrılsın
- Unblock olan task'lar tespit edilsin ve loglanısn

### 1.2 QA Auto-Trigger ✅
- EP tamamlandığında (run_reason = CodingAgent) otomatik QA başlatılsın
- `openclaw-qa::start_qa()` çağrısı container service içine enjekte edilsin
- QA geçerse → workspace "InReview" statüsüne çekilsin
- QA başarısızsa → `CodingAgentFollowUpRequest` olarak agent'a geri fırlatılsın

### 1.3 Context Injection ✅
- Tamamlanan task'ın `execution_process_logs` sonunda özet üretilsin
- Bu özet `oc_task_run_state.context_summary`'ye yazılsın
- Bir sonraki task başlatılırken prompt'un başına eklensin:
  `"Önceki adımda şunlar yapıldı: [özet]. Şimdi senin görevin: [prompt]"`

### 1.4 Workspace-Task Bağlantısı ✅
- Workspace oluşturulunca `oc_task_run_state.workspace_id` set edilsin
- Workspace silinince / arşivlenince task state güncellenmeli

**Çıktı:** Tek bir `pnpm run dev` sonrası plan oluştur → çalıştır → agent bitince
otomatik unblock + QA döngüsü başlar.

---

## Faz 2 — Akıllı Planlayıcı (LLM Entegrasyonu) ✅

**Hedef:** "Şunu yap" dersen OpenClaw kodu okur, ne yapılacağını kendisi anlar.

### 2.1 Codebase Bağlamı Toplama ✅
- `FileSearchService` ile repo'daki anahtar dosyalar taranır
- Proje tipi tespit edilir (Rust/TS/Python/...)
- Mevcut issue'lar çekilir (duplikasyon engeli)
- Özet bağlam metni oluşturulur

### 2.2 LLM'e Structured Prompt ✅
- Mevcut executor (Claude Code, Codex vb.) seçilir
- Prompt şablonu:
  ```
  Sen bir yazılım PM'isin. Aşağıdaki kod tabanına ve isteğe bak.
  [kod tabanı özeti]
  İstek: [kullanıcı prompt'u]
  Mevcut issue'lar: [duplikasyon kontrolü]
  
  Cevabını JSON olarak ver:
  { tasks: [{title, description, complexity, depends_on: []}] }
  ```
- Executor'dan JSON çıktısı parse edilir

### 2.3 Otomatik Dependency Tespiti ✅
- LLM'den gelen `depends_on` alanları DAG'a eklenir
- Döngüsel bağımlılık kontrolü yapılır
- Tahminlenen sıra DB'ye yazılır

### 2.4 Duplikasyon ve Çatışma Kontrolü ✅
- Aynı proje'nin açık issue'larıyla semantic benzerlik karşılaştırması
- Benzer iş varsa kullanıcıya uyarı gösterilir

**Çıktı:** Kullanıcı sadece tek cümle yazar → 5-10 saniye içinde sıralı,
bağımlılıklı ticket listesi çıkar.

---

## Faz 3 — Frontend UI

**Hedef:** Kullanıcı planı görsün, yönetsin, onaylasın.

### 3.1 Plan Oluşturma Ekranı ✅
- Mevcut Kanban UI'ına "OpenClaw" butonu eklenir
- Prompt input + proje seçimi
- "Analiz Et" → yükleniyor → task listesi önizlemesi
- Kullanıcı task'ları düzenleyebilir / silebilir / ekleyebilir

### 3.2 Dependency Graph Görselleştirmesi
- Task kartları arası ok çizgileri (DAG vizualizasyonu)
- Sürükle-bırak ile bağımlılık ekle/çıkar
- Döngü tespitinde kırmızı vurgu

### 3.3 Orchestration Run Paneli
- Her task'ın durumu: Pending / Blocked / Running / QA / Done / Failed
- Hangi task hangi agent'ı çalıştırıyor
- QA retry sayısı ve son hata mesajı
- "Durdur" / "Yeniden Başlat" butonları

### 3.4 QA Sonuç Ekranı
- Test çıktısı syntax-highlighted
- "Agent'a geri gönder" veya "Elle düzelt" seçimi
- Max retry dolunca insan müdahale akışı

**Çıktı:** Kullanıcı tarayıcıdan tüm akışı takip eder ve sadece
kritik noktalarda dahil olur.

---

## Faz 4 — Otonom Mod ve İnce Ayarlar

**Hedef:** Sistemi gerçek anlamda otonom hale getir, güvenli sınırlar çiz.

### 4.1 Onay Kapıları (Human-in-the-Loop)
- Kullanıcı "tam otonom" veya "her adımda onayla" seçebilir
- Kritik işlemler (DB migration, dosya silme) her zaman onay ister
- Mevcut `ExecutorApprovalService` entegre edilir

### 4.2 Paralel Agent Çalıştırma
- Bağımlılığı olmayan task'lar eş zamanlı başlatılır
- Resource limit: max N eş zamanlı agent (konfigüre edilebilir)
- Çakışan dosya değişikliklerini tespit et, uyar

### 4.3 Geri Alma ve Checkpoint
- Her task başlamadan önce git snapshot alınır
- Bir task başarısız olursa tüm run geri alınabilir
- "Checkpoint'e dön" butonu

### 4.4 Öğrenen Sistem
- Her başarılı plan → template olarak kaydedilebilir
- "Benzer bir plan daha önce yapıldı, onu kullanayım mı?" önerisi
- Hangi agent hangi task tipinde daha başarılı → istatistik

### 4.5 Bildirimler
- QA başarısız oldu → bildirim
- Tüm run tamamlandı → bildirim
- Mevcut `NotificationService` kullanılır

**Çıktı:** Sabah "şunu yap" diyorsun, akşam PR hazır — sen sadece merge ediyorsun.

---

## Teknik Borç ve Dikkat Edilecekler

### Güvenlik
- LLM'e repo'nun tamamını değil özet/anahtar dosyalar gönderin
- Executor'a gönderilen prompt'larda credential/secret olmadığından emin ol
- QA scriptleri sandbox'ta çalıştırılmalı

### Performans
- Büyük repo'larda codebase tarama cache'lenmeli (moka kullanılabilir)
- LLM çağrısı async, UI'da progress gösterilmeli
- Dependency graph büyüyünce (50+ task) DAG algoritması optimize edilmeli

### Hata Yönetimi
- Agent çöktüğünde orchestration run otomatik `Failed` olmayabilir → timeout mekanizması
- QA komutu bulunamazsa graceful fallback (kullanıcıya sor)
- Network hataları için retry with backoff (backon crate zaten var)

---

## Öncelik Sırası (Önerilen)

```
Faz 1 (Event Wiring)     ← ŞİMDİ  — iskelet canlı hale geliyor
Faz 2 (LLM Planlayıcı)  ← SONRA  — zeka ekleniyor
Faz 3 (Frontend)         ← SONRA  — görünür hale geliyor
Faz 4 (Otonom Mod)       ← İLERİ  — tam güç
```

Faz 1 olmadan diğerleri havada kalır.
Faz 2 olmadan sistem hâlâ yarı manuel.
Faz 3 olmadan sadece API üzerinden kullanılabilir.
Faz 4 ise asıl hedef — gerçek otonom yazılım ekibi.
