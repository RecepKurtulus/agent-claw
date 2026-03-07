# OpenClaw — Otonom Orkestrasyon Katmanı

## Problem
Vibe Kanban agent'ları çalıştırmak için harika bir altyapı sunuyor ama "düşünen" bir katman yok.
Tüm planlama, ticket oluşturma, dependency yönetimi ve QA döngüsü hâlâ insana bağlı.

## Yaklaşım
Mevcut Vibe Kanban kodebase'ine 3 yeni Rust crate ekleyerek sisteme bir "PM beyin" katmanı ekliyoruz.
Mevcut servisler (`ContainerService`, `EventService`, `ExecutorAction` zinciri) dokunulmadan kalıyor.
OpenClaw bunların üstüne oturuyor — bir orkestrasyon wrapper'ı.

---

## Yeni Crate'ler

### 1. `crates/openclaw-planner`
**Görev:** Kodu okur, bağlamı analiz eder, ticket'ları otomatik üretir.

- Proje repo'sunu tarar (mevcut `FileSearchService` + `RepoService` kullanır)
- LLM'e (Claude veya seçili executor) codebase bağlamı verir
- Cevap olarak yapılandırılmış issue listesi + dependency graph döner
- Issue'ları ve dependency edge'lerini DB'ye yazar
- Mevcut Kanban issue'larıyla duplikasyon kontrolü yapar

**Yeni DB tabloları:**
```sql
oc_plans (id, project_id, prompt, status, created_at)
oc_plan_tasks (id, plan_id, issue_id, title, description, estimated_complexity)
```

### 2. `crates/openclaw-orchestrator`
**Görev:** Dependency graph'ı yönetir, agent'ları doğru sırada tetikler, blokları çözer.

- Her task için bağımlılık kenarlarını takip eder (DAG — Directed Acyclic Graph)
- `ExecutionProcess` olaylarını dinler (mevcut SQLite hook sistemi)
- Bir task tamamlandığında bağımlı task'ları "unblock" eder
- Blocked task'lar için Workspace oluşturulmasını geciktirir
- Inter-agent context köprüsü: tamamlanan task'ın özetini bir sonraki task'ın prompt'una enjekte eder

**Yeni DB tabloları:**
```sql
oc_task_dependencies (id, task_id, depends_on_task_id, created_at)
oc_orchestration_runs (id, plan_id, status, started_at, completed_at)
oc_task_run_state (id, task_id, run_id, status, blocked_by, context_summary)
```

**Mevcut hook noktaları:**
- `ContainerService::start_execution()` — başlatmadan önce dependency check
- `EventService::create_hook()` — ExecutionProcess tamamlandığında tetikle
- `ExecutorAction::next_action` zinciri — otomatik sıralı aksiyon zinciri

### 3. `crates/openclaw-qa`
**Görev:** Agent kodu bitirdiğinde testleri otomatik çalıştırır, hata varsa geri fırlatır.

- Workspace'in repo'suna göre test komutunu tespit eder (`package.json`, `Cargo.toml`, etc.)
- `ExecutionProcessRunReason::CodingAgent` tamamlandığında tetiklenir
- Ayrı bir `ExecutionProcess` olarak test scriptini çalıştırır (mevcut `ScriptRequest` executor action)
- Test başarısız → agent'a hata çıktısını `CodingAgentFollowUpRequest` olarak geri fırlatır
- Test başarılı → Workspace'i "InReview" statüsüne geçirir (insan onayı için)
- Max retry sayısı konfigüre edilebilir (varsayılan: 3)

**Yeni DB tabloları:**
```sql
oc_qa_runs (id, execution_process_id, workspace_id, status, test_command, retry_count, max_retries)
oc_qa_results (id, qa_run_id, exit_code, output, created_at)
```

---

## Entegrasyon Noktaları (Mevcut Kod)

| Bileşen | Nasıl Kullanılıyor |
|---------|-------------------|
| `ContainerService::start_execution()` | Orchestrator burada araya girerek dependency check yapar |
| `EventService` SQLite hooks | ExecutionProcess status değişimlerini dinlemek için |
| `ExecutorAction::next_action` | QA test → follow-up zinciri kurmak için |
| `FileSearchService` | Planner'ın codebase'i taraması için |
| `Session` | Her agent için zaten izole session var — aynen kullanılıyor |
| `Scratch` | Plan taslakları için (DraftPlan tipi eklenecek) |
| Mevcut SSE `/api/events` | OpenClaw orchestration event'leri de buradan akar |

---

## Yeni API Endpoint'leri (`crates/server` router'a eklenir)

```
POST /api/openclaw/plan          — Planner'ı tetikle (prompt + project_id)
GET  /api/openclaw/plans         — Mevcut planları listele
GET  /api/openclaw/plans/:id     — Plan detayı + task durumları
POST /api/openclaw/plans/:id/run — Planı orchestrate et (tüm task'ları başlat)
GET  /api/openclaw/runs/:id      — Orchestration run durumu
POST /api/openclaw/qa/configure  — QA ayarları (max retry, test command override)
```

---

## DB Migration Sırası

1. `oc_plans` + `oc_plan_tasks`
2. `oc_task_dependencies` + `oc_orchestration_runs` + `oc_task_run_state`
3. `oc_qa_runs` + `oc_qa_results`
4. `scratch_type` enum'una `OcPlanDraft` eklenir

---

## TypeScript Tipleri

Rust struct'larına `#[derive(TS)]` eklenir, `pnpm run generate-types` ile `shared/types.ts`'e eklenir:
- `OcPlan`, `OcPlanTask`, `OcTaskDependency`, `OcOrchestrationRun`, `OcTaskRunState`
- `OcQaRun`, `OcQaResult`

---

## Todo Listesi (Sıralı)

1. DB migrations (3 migration dosyası)
2. `crates/openclaw-planner` iskeleti + Cargo.toml
3. Planner servis implementasyonu (codebase tarama + LLM çağrısı + issue üretimi)
4. `crates/openclaw-orchestrator` iskeleti + Cargo.toml
5. Dependency graph logic (DAG, block/unblock)
6. ExecutionProcess event listener (mevcut hook sistemine bağlanma)
7. Inter-agent context injection (tamamlanan task özeti → sonraki prompt)
8. `crates/openclaw-qa` iskeleti + Cargo.toml
9. Test komut tespiti (repo tipine göre)
10. QA döngüsü (test → fail → follow-up → retry)
11. Server route'ları + api-types
12. TypeScript type generation
13. Temel Rust unit testleri
14. Format + lint
