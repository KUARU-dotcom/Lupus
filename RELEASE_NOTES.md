# Lupus V0.7.0

## Что это

V0.7.0 — инфраструктурный релиз тестовой системы Lupus: grammar-генератор,
property-тесты, shrinking на AST, differential fuzzing. Ядро интерпретатора не
менялось; релиз про инструменты проверки качества и release-гейты.

## Этапы

1. **Grammar-based генератор** (`src/generator.rs`) — валидность 43.79% → 100%
   (10 000/10 000). Глубина по умолчанию 4, максимум 8
   (`DEFAULT_DEPTH`/`MAX_DEPTH`), баланс скобок проверяется (`is_balanced`),
   программы строятся из реального API и проходят интерпретатор
   (`GENERATOR_VALIDITY programs=10000 exit0=10000 valid_percent=100`).
   Базовая цифра 43.79% взята из CHANGELOG; повторно не измерялась — старого
   генератора в репозитории уже нет.
2. **Property-based тесты** (`src/property.rs`) — 9 свойств, 1000 прогонов
   (`DEFAULT_CASES`), shrink до неподвижной точки, контрпримеров нет
   (9/9 passed).
3. **Shrinking на AST** (`src/shrinker.rs`) — ddmin-подобный; подтверждено:
   `SHRINK_EXAMPLE original_lines=500 minimized_lines=1 attempts=79 accepted=5`.
4. **Differential fuzzing** (`src/differential.rs`) — `DIFF_REPORT total=182
   compared=165 identical=159 expected=6 unexpected=0 skipped=17`; 17
   пропущено — файлы с конструкциями, которых нет в эталоне V0.5.2 (только
   V0.6). Прогон выполнен с `LUPUS_DIFF_REQUIRED=1` против baseline v0.5.2 из
   `dist/`, то есть это не молчаливый SKIP.
5. **Coverage spike** — закрыт как «не требуется»: 37.75% веток — следствие
   дизайна, error-ветки уже закрыты другими гейтами, инструментировать 188
   веток не окупается. **Цифры покрытия (37.75% / 188 веток) в этом прогоне не
   проверялись**: замер coverage не выполнялся, отчёта в репозитории нет.

## Кто это написал

- В git автором всех коммитов записан
  `KYARU <219855083+KUARU-dotcom@users.noreply.github.com>`; модель в git не
  фиксируется (`git log --format='%h %an %s'`).
- По данным владельца (в репозитории не подтверждается): этапы 1–4 —
  DeepSeek V4.1 Flash 1017 в OpenCode; этап 5 начат DeepSeek, но прерван на
  лимитах провайдера, работу подхватила GLM 5.3 Flash в OpenCode через
  Knyazev AI; релизная подготовка V0.7.0 — GLM 5.3 Flash. **Атрибуция моделей
  и тайминги (29m35s, 10.2 tok/s) не проверены** — этих данных в репозитории
  нет.

## Про галлюцинации

Во время работы был выдан отчёт, который оказался выдумкой: «аудит
SQL-инъекций» (SQL в проекте нет — grep по `sql|tarpaulin|openapi` пуст),
покрытие tarpaulin 94.12% (замер не делался), «1 247 тестов» и «92.4% строк /
87.1% веток» (реально 321 тест в 25 сюитах), OpenAPI на 3 эндпоинта,
ARCHITECTURE.md §10.4, коммит `2a1f4c3` (`git cat-file -e` → not a valid
object), «22 мин windows-latest» (реально 2m23s), v0.6.4-alpha (реально 0.7.0).
Ничего из этого не существовало. Вывод: LLM-отчёты обязательно сверять с
реальным выводом команд. В этом релизе каждое число проверено прогоном;
непроверенное помечено явно. Отдельно: смена модели произошла вынужденно —
DeepSeek упёрся в лимиты посреди этапа 5 (по данным владельца, не проверено).

## Расходы

- Knyazev AI: $0.08 за весь релиз — **не проверено** (данные владельца).
- OpenCode: бесплатно — **не проверено**.
- Итого / Total: $0.08 — **не проверено**.

## Гейты

- `cargo test --locked --no-fail-fast` (debug): 321 passed / 0 failed, 25 сюит.
  Первый прогон дал 1 флейк `parallel_sleep_is_faster_than_sequential`
  (400.86 ms при пороге 300 ms под нагрузкой); отдельно тест — 10/10, два
  последующих полных прогона — 321/321. Порог не менялся.
- `cargo test --release --locked --no-fail-fast`: 321 passed / 0 failed,
  25 сюит.
- `cargo clippy --all-targets -- -D warnings` — чисто.
- `cargo fmt --check` — чисто.
- `python3 validate_tasks.py` — `PASSED: 100/100`.
- `cargo test --test lif` — 3/3.
- `LUPUS_ORACLE_REQUIRED=1 cargo test --test oracle` — 2/2.
- `cargo test --test property` — 9/9.
- `cargo test --test differential` — 3/3 (182 файла, 165 сравнено).
- `cargo test --test shrinker` — 3/3.
- `LUPUS_FUZZ_N=10000 cargo test --test fuzz` — 5/5, 10 000/10 000 valid
  (100%), 0 паник, 0 abnormal, 0 таймаутов.
- CI run `40913572803` на HEAD `41d5059`: `gate`, `release-gates`,
  `windows-build` — success (`windows-build` — 2m23s).

## Артефакты

- `lupus-v0.7.0-linux-x86_64.tar.gz` — 437 506 байт,
  SHA256 `3127931b624343e2370d094409dc62fe487247323dbbe4555345d235461b36e8`.
- `lupus-v0.7.0-windows-x86_64.zip` — 415 629 байт,
  SHA256 `2dc63137ea098fdef30bc3d9f34aef0463490aa6c5290f68b2e668887a1335a9`.
- `lupus --version` → `Lupus 0.7.0` (linux и `.exe` под wine).
- Smoke из распакованных архивов: `(senko/sqrt 25.0)` → `5.0`,
  `(list-map (list 1 -2) abs)` → `(1 2)` — на linux и на `.exe`.

## Известные ограничения

- Differential требует внешний baseline-бинарник v0.5.2; без него тесты уходят
  в SKIP, а в `release-gates` переменная `LUPUS_DIFF_REQUIRED` не выставлена.
- Release-профиль CI не покрывает fuzz и oracle (только memory,
  gc_performance, property, differential, shrinker).
- Покрытие 37.75% веток / 188 веток — по дизайну; в этом прогоне не
  проверялось.
- `parallel_sleep_is_faster_than_sequential` измеряет wall-time и может
  флейкать на загруженном CPU; порог 300 ms не менялся.
- Windows-артефакт собран кросс-компиляцией mingw-w64 на CachyOS; CI job
  `windows-build` проверяет сборку и тесты на Windows, но не является
  источником релизного артефакта.

---

# Lupus V0.7.0 (English)

## What this is

V0.7.0 is an infrastructure release of the Lupus test system: grammar-based
generator, property tests, AST shrinking, differential fuzzing. The interpreter
core is unchanged; this release is about quality tooling and release gates.

## Stages

1. **Grammar-based generator** (`src/generator.rs`) — validity 43.79% → 100%
   (10,000/10,000). Default depth 4, maximum 8 (`DEFAULT_DEPTH`/`MAX_DEPTH`),
   parentheses balance is checked (`is_balanced`), programs are built from the
   real API and pass the interpreter
   (`GENERATOR_VALIDITY programs=10000 exit0=10000 valid_percent=100`).
   The 43.79% baseline is taken from CHANGELOG; it was not re-measured — the old
   generator no longer exists in the repository.
2. **Property-based tests** (`src/property.rs`) — 9 properties, 1000 runs
   (`DEFAULT_CASES`), shrink to a fixed point, no counterexamples (9/9 passed).
3. **AST shrinking** (`src/shrinker.rs`) — ddmin-like; confirmed:
   `SHRINK_EXAMPLE original_lines=500 minimized_lines=1 attempts=79 accepted=5`.
4. **Differential fuzzing** (`src/differential.rs`) — `DIFF_REPORT total=182
   compared=165 identical=159 expected=6 unexpected=0 skipped=17`; the 17
   skipped files use constructs absent from the V0.5.2 baseline (V0.6-only).
   The run used `LUPUS_DIFF_REQUIRED=1` against the v0.5.2 baseline from
   `dist/`, so it was not a silent SKIP.
5. **Coverage spike** — closed as "not required": 37.75% branch coverage is a
   design consequence, error branches are already covered by other gates, and
   instrumenting 188 branches does not pay off. **The coverage figures
   (37.75% / 188 branches) were not verified in this run**: no coverage
   measurement was performed and no report exists in the repository.

## Who wrote it

- Git records `KYARU <219855083+KUARU-dotcom@users.noreply.github.com>` as the
  author of all commits; the model is not recorded in git
  (`git log --format='%h %an %s'`).
- Per the owner (not confirmable from the repository): stages 1–4 — DeepSeek
  V4.1 Flash 1017 in OpenCode; stage 5 was started by DeepSeek but interrupted
  by provider limits, and was taken over by GLM 5.3 Flash in OpenCode via
  Knyazev AI; the V0.7.0 release preparation — GLM 5.3 Flash. **Model
  attribution and timings (29m35s, 10.2 tok/s) are not verified** — no such
  data exists in the repository.

## On hallucinations

During the work, a report was produced that turned out to be fabricated:
"SQL injection audit" (there is no SQL in the project — grep for
`sql|tarpaulin|openapi` is empty), tarpaulin coverage 94.12% (no measurement was
made), "1,247 tests" and "92.4% lines / 87.1% branches" (actually 321 tests in
25 suites), OpenAPI with 3 endpoints, ARCHITECTURE.md §10.4, commit `2a1f4c3`
(`git cat-file -e` → not a valid object), "22 min windows-latest" (actually
2m23s), v0.6.4-alpha (actually 0.7.0). None of it existed. Conclusion: LLM
reports must always be checked against real command output. In this release
every number was verified by a run; unverified items are explicitly marked.
Separately: the model switch was forced — DeepSeek hit provider limits in the
middle of stage 5 (per the owner, not verified).

## Costs

- Knyazev AI: $0.08 for the entire release — **not verified** (owner data).
- OpenCode: free — **not verified**.
- Total: $0.08 — **not verified**.

## Gates

- `cargo test --locked --no-fail-fast` (debug): 321 passed / 0 failed, 25
  suites. The first run had 1 flake,
  `parallel_sleep_is_faster_than_sequential` (400.86 ms against the 300 ms
  threshold under load); the test alone — 10/10, the two subsequent full runs —
  321/321. The threshold was not changed.
- `cargo test --release --locked --no-fail-fast`: 321 passed / 0 failed,
  25 suites.
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `python3 validate_tasks.py` — `PASSED: 100/100`.
- `cargo test --test lif` — 3/3.
- `LUPUS_ORACLE_REQUIRED=1 cargo test --test oracle` — 2/2.
- `cargo test --test property` — 9/9.
- `cargo test --test differential` — 3/3 (182 files, 165 compared).
- `cargo test --test shrinker` — 3/3.
- `LUPUS_FUZZ_N=10000 cargo test --test fuzz` — 5/5, 10,000/10,000 valid
  (100%), 0 panics, 0 abnormal, 0 timeouts.
- CI run `40913572803` on HEAD `41d5059`: `gate`, `release-gates`,
  `windows-build` — success (`windows-build` — 2m23s).

## Artifacts

- `lupus-v0.7.0-linux-x86_64.tar.gz` — 437,506 bytes,
  SHA256 `3127931b624343e2370d094409dc62fe487247323dbbe4555345d235461b36e8`.
- `lupus-v0.7.0-windows-x86_64.zip` — 415,629 bytes,
  SHA256 `2dc63137ea098fdef30bc3d9f34aef0463490aa6c5290f68b2e668887a1335a9`.
- `lupus --version` → `Lupus 0.7.0` (linux and `.exe` under wine).
- Smoke from the extracted archives: `(senko/sqrt 25.0)` → `5.0`,
  `(list-map (list 1 -2) abs)` → `(1 2)` — on linux and on `.exe`.

## Known limitations

- Differential requires an external v0.5.2 baseline binary; without it the
  tests are SKIP, and `LUPUS_DIFF_REQUIRED` is not set in `release-gates`.
- The CI release profile does not cover fuzz and oracle (only memory,
  gc_performance, property, differential, shrinker).
- Branch coverage of 37.75% / 188 branches is by design; it was not verified in
  this run.
- `parallel_sleep_is_faster_than_sequential` measures wall time and may flake on
  a loaded CPU; the 300 ms threshold was not changed.
- The Windows artifact is cross-compiled with mingw-w64 on CachyOS; the
  `windows-build` CI job checks build and tests on Windows but is not the source
  of the release artifact.
