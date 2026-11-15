# Integracja nowych źródeł Clay UI

Stan: 2026-11-15T22:04:38+00:00. Gałąź `t3/clay-ui-eval-integration`; baza `3b162ebff3a51d915e3550f1fe28d3c059f6d16b`. Lokalna zmiana do review, bez push, npm, PR i publikacji.

Dodano wszystkie **23 źródła etapu 1**, łącznie **5149 wpisów** po filtrach. Zrealizowano wczytanie, metadane licencji, bezpieczny zakres publiczny, pobieranie i testy. Etap 2 odłożono zgodnie z opcjonalnym zakresem briefu; konkretne powody poniżej. Ranking i wagi nie zostały zmienione. **Pomiar nie wykazuje udowodnionej poprawy relewancji.** Nowe źródła wypierają ocenione wyniki nieocenionymi, a rzeczywista metryka odrzucenia pogarsza się do 0%. Potrzebna jest niezależna anotacja przed oceną korzyści.

## Dodane źródła

| Źródło / ID | Status / liczba | Licencja z LICENSE | Typ i ograniczenie | Sync s |
| --- | --- | --- | --- | ---: |
| [Evil Charts](https://evilcharts.com/r/registry.json) / `evilcharts` | dodane / 57 | [MIT](https://github.com/legions-developer/evilcharts/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.274 |
| [Origin UI](https://raw.githubusercontent.com/cosscom/ui/main/apps/origin/registry.json) / `origin` | dodane / 640 | [MIT](https://github.com/cosscom/ui/blob/HEAD/apps/origin/LICENSE.md) | Registry; typy ui/component/block, bez jawnych demo i Pro; raw GitHub apps/origin; numeryczne comp-* opisane wyłącznie tagami upstream, oznaczone jako kompozycje | 0.494 |
| [Kibo UI](https://www.kibo-ui.com/r/registry.json) / `kibo` | dodane / 40 | [MIT](https://github.com/shadcnblocks/kibo/blob/HEAD/license.md) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.513 |
| [ReUI](https://reui.io/r/styles/base-nova/registry.json) / `reui` | dodane / 82 | [MIT](https://github.com/keenthemes/reui/blob/HEAD/LICENSE.md) | Registry; tylko UI, bez przykładów; 82 anonimowe payloady z kodem zweryfikowane podczas sync, 401/403/404 pomijane; premium blocks wykluczone | 5.495 |
| [blocks.so](https://blocks.so/r/registry.json) / `blocks-so` | dodane / 81 | [MIT](https://github.com/ephraimduncan/blocks/blob/HEAD/LICENSE.md) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.195 |
| [Motion Primitives](https://motion-primitives.com/c/registry.json) / `motion-primitives` | dodane / 33 | [MIT](https://github.com/ibelick/motion-primitives/blob/HEAD/LICENCE.md) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.133 |
| [SmoothUI](https://smoothui.dev/r/registry.json) / `smoothui` | dodane / 264 | [MIT](https://github.com/educlopez/smoothui/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.533 |
| [Fancy Components](https://fancycomponents.dev/r/registry.json) / `fancy` | dodane / 45 | [MIT](https://github.com/danielpetho/fancy/blob/HEAD/LICENSE) | Registry; tylko UI, bez przykładów; wykluczone 99 bloków/przykładów | 0.589 |
| [BeUI](https://beui.dev/r/registry.json) / `beui` | dodane / 158 | [MIT](https://github.com/starc007/ui-components/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.231 |
| [UIArc](https://uiarc.dev/r/registry.json) / `uiarc` | dodane / 143 | [MIT](https://github.com/kuratlielia/arc-library/blob/HEAD/LICENSE) | Registry; jawne meta.tier=free | 0.201 |
| [devl.dev](https://devl.dev/r/registry.json) / `devl` | dodane / 260 | [MIT](https://github.com/sean-brydon/devl.dev/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 1.045 |
| [Cult UI](https://www.cult-ui.com/r/registry.json) / `cult` | dodane / 154 | [MIT](https://github.com/nolly-studio/cult-ui/blob/HEAD/LICENSE.md) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.257 |
| [8bitcn](https://www.8bitcn.com/r/registry.json) / `8bitcn` | dodane / 118 | [MIT](https://github.com/TheOrcDev/8bitcn-ui/blob/HEAD/license.md) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.540 |
| [Shadcn Space](https://shadcnspace.com/r/registry.json) / `shadcn-space` | dodane / 562 | [MIT](https://github.com/shadcnspace/shadcnspace/blob/HEAD/LICENSE) | Registry; live isPro=false AND wpis w publicznym repo MIT | 0.572 |
| [UI Layouts](https://ui-layouts.com/r/registry.json) / `ui-layouts` | dodane / 327 | [MIT](https://github.com/ui-layouts/uilayouts/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.586 |
| [Spectrum UI](https://ui.spectrumhq.in/r/registry.json) / `spectrum` | dodane / 261 | [Apache-2.0](https://github.com/arihantcodes/spectrum-ui/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.442 |
| [Ruixen UI](https://ruixen.com/registry.json) / `ruixen` | dodane / 214 | [MIT](https://github.com/ruixenui/ruixen.com/blob/HEAD/LICENSE.md) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.269 |
| [Watermelon UI](https://registry.watermelon.sh/r/registry.json) / `watermelon` | dodane / 1170 | [MIT](https://github.com/WatermelonCorp/watermelon-platform/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.354 |
| [Avara UI](https://avaraui.com/r/registry.json) / `avara` | dodane / 206 | [MIT](https://github.com/AlgorizeTech/Avara-UI/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 1.250 |
| [HextaUI](https://hextaui.com/r/registry.json) / `hextaui` | dodane / 79 | [MIT](https://github.com/preetsuthar17/hextaui/blob/HEAD/LICENSE) | Registry; tylko UI, bez przykładów; wykluczone 812 demo | 0.397 |
| [Vengeance UI](https://vengenceui.com/r/registry.json) / `vengeance` | dodane / 135 | [MIT](https://github.com/Ashutoshx7/VengeanceUI/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.503 |
| [Componentry](https://componentry.dev/r/registry.json) / `componentry` | dodane / 56 | [MIT](https://github.com/harshjdhv/componentry/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro | 0.391 |
| [asciicn](https://asciicn.dev/r/registry.json) / `asciicn` | dodane / 64 | [MIT](https://github.com/opencoredev/asciicn/blob/HEAD/LICENSE) | Registry; typy ui/component/block, bez jawnych demo i Pro; własne zależności silnika pobierane z tego samego rejestru, konfiguracja CSS zachowana | 0.616 |

Dokładne URL-e indeksów i elementów: [sources.json](sources.json), takie same jak w `src/sources.rs` (testy sprawdzają oba). Weryfikacja LICENSE: [license-audit.json](license-audit.json), pełne teksty i SHA-256; wszystkie 23 pobrania HTTP 200 i zgodny plik MIT/Apache-2.0. [endpoint-checks.json](endpoint-checks.json) zawiera statusy, hashe i czasy próbek indeks/item/preview; nie jest audytem każdego elementu wszystkich rejestrów.

Filtry wykluczają theme/style/lib/hook/file/item/page/example z głównego katalogu, CLI SmoothUI i jawne nazwy/ścieżki demo. Zależności pozostają w metadanych; asciicn dodatkowo pobiera własny silnik (limit 16 elementów, deduplikacja/cykle, bez arbitralnych obcych hostów). Retencja bloków i wariantów nie oznacza liczby unikalnych zachowań; bloki i kompozycje Origin mają `example: true`. Nie generowano twierdzeń o funkcjach z samej nazwy.

## Sync i granice weryfikacji

Cache: `/private/tmp/clayui-new-sources-48c3rb0_` (kanoniczny katalog poza repo). Migawki do dokładnego replay: `/private/tmp/clayui-new-sources-48c3rb0_/snapshots/baseline-index.json` i `expanded-index.json`. Hashe: [measurement-hashes.json](measurement-hashes.json). [sync.json](sync.json) zapisuje liczbę, czas, status i błąd każdego źródła; suma raportowanych czasów źródeł 23.948 s. Origin i SmoothUI zostały ponownie zsynchronizowane po poprawieniu metadanych/wykluczeniu CLI, a pomiar używa końcowej migawki.

Wszystkie 23 nowe źródła: sync exit 0, bez błędów. Dotychczasowy **RareUI: HTTP 404**, 0 wpisów, exit 1. Ten sam stan bazowy zachowano w obu ramionach. Baseline: 678 rekordów / 677 unikalnych ID; expanded: 5827 / 5826. Powtórzony `aceternity/background-lines` występuje raz dodatkowo w obu surowych katalogach. Harness stosuje taki sam Map po pełnym ID jak poprzedni eksperyment (ostatnia wartość, pierwsza pozycja). Nie naprawiano CLI poza zakresem. Pierwsza próba cache przez `/tmp` została bezpiecznie odrzucona przez ochronę symlinków; wznowienie przez `/private/tmp` zakończyło się prawidłowo.

## Search / show / get

| Komponent | Search / show / get | Plików pobranych | Preview | Uwagi |
| --- | --- | ---: | --- | --- |
| `evilcharts/recharts-bar-chart` | 0 / 0 / 0 | 1 | none | no-preview, local-imports-missing |
| `reui/data-grid` | 0 / 0 / 0 | 12 | ok | local-imports-missing |
| `kibo/dropzone` | 0 / 0 / 0 | 1 | ok | local-imports-missing |
| `blocks-so/file-upload-03` | 0 / 0 / 0 | 1 | none | no-preview, local-imports-missing |
| `blocks-so/sidebar-01` | 0 / 0 / 0 | 7 | none | no-preview, local-imports-missing |
| `asciicn/miami` | 0 / 0 / 0 | 30 | none | no-preview, local-imports-missing |
| `uiarc/button` | 0 / 0 / 0 | 2 | ok | local-imports-missing |

Szczegóły, listy plików, zależności, sourceUrl, licencja i obserwowane braki: [smoke.json](smoke.json). Wszystkie próbki mają MIT. Kibo/ReUI/UIArc mają działającą stronę dokumentacji. Dla źródeł bez potwierdzonego wzorca podglądu pozostaje `previewUrl: null` / `no-preview`; nie wymyślano adresów. HTTP 200 oznacza obecność strony, a nie test wizualny. Nie uruchamiano kodu komponentów ani instalacji w projekcie użytkownika. `local-imports-missing` jest jawną informacją, bez zmiany rankingu; m.in. shadcn/aliasy i konfigurację należy dopasować przed użyciem. Asciicn/miami pobrał 30 plików wraz z rendererem/shaderem, lecz nie jest to dowód samodzielnej aplikacji.

## Benchmark dev

Użyto **120 wcześniej zapisanych zwięzłych zapytań EN**, 105 answer i 15 reject, 24 rodziny, z istniejącymi twardymi ograniczeniami source. Nie traktowano PL/EN jako niezależnych potrzeb; polska część nie była mierzona. Dane, pytania i scorer z `ui-component-finder/eval` pozostały niezmienione; nie odczytano katalogu zapieczętowanego. [input-hashes.json](input-hashes.json) identyfikuje wejścia. Wyniki są pomiarem funkcji retrieval na stałym katalogu: bez agenta, live 21st i instalacji kodu. Nie są bezpośrednio porównywalne z dawnym live baseline lub wzbogaconymi qrels.

W obu ramionach działa ten sam produkcyjny Rust BM25 / rank_semantic, te same limity, wagi, próg i model **gemini-embedding-001, 768**. Użyto istniejącego readera klucza i cache; zastany GOOGLE_API_KEY przekazano wyłącznie w środowisku procesu jako GEMINI_API_KEY. Zapis po każdej paczce embeddingów i każdym zapytaniu, bez klucza na stdout/plikach. Płatne embeddingi były wprost częścią briefu; nie mierzono kosztu faktury dostawcy.

| Tryb / katalog | Success3@5 dolna / optymistyczna | ContextSuccess3@5 dolna / opt. | CorrectRejection | Unjudged top5 / udział |
| --- | --- | --- | --- | --- |
| baseline-bm25 | 59.0% / 99.0% | 45.0% / 98.3% | 6.7% | 303 / 56.2% |
| expanded-bm25 | 27.6% / 100.0% | 33.3% / 100.0% | 0.0% | 501 / 90.8% |
| baseline-gemini | 65.7% / 94.3% | 48.3% / 90.0% | 20.0% | 191 / 38.0% |
| expanded-gemini | 31.4% / 100.0% | 38.3% / 100.0% | 0.0% | 489 / 88.3% |

| Różnica expanded − baseline | Δ Success dolna, pp [95% CI] | Δ Success opt., pp [95% CI] | Δ Context dolna, pp [95% CI] | Δ Rejection, pp [95% CI] |
| --- | --- | --- | --- | --- |
| bm25 | -31.4 [-47.0; -16.5] | +1.0 [0.0; 3.2] | -11.7 [-27.3; 0.0] | -6.7 [-20.0; 0.0] |
| gemini | -34.3 [-49.0; -20.0] | +5.7 [0.0; 13.3] | -10.0 [-22.2; 0.0] | -20.0 [-40.0; 0.0] |

Bootstrap: **10 000 wspólnych losowań rodzin**, seed 20261109, percentyle 2.5/97.5; różnice sparowane, nie porównanie marginalnych CI. Pełne marginalne CI dla dolnej i optymistycznej Success/Context/Rejection, Recall/MRR/nDCG, rodziny, fixture i wyniki każdego pytania zapisano w `*.metrics.json` i [summary.json](summary.json). Mała liczba rodzin reject ogranicza wnioskowanie; zerowe wyniki dają zdegenerowane CI. Brak poprawek na wielokrotne porównania. Nie było strojenia po tych wynikach.

**Interpretacja:** dolna granica spada o 31.4 pp w BM25 i 34.3 pp w Gemini, ponieważ nieocenieni kandydaci wypierają ocenionych. To rzeczywisty spadek wyniku konserwatywnego, ale nie pomiar rzeczywistej trafności nowych komponentów. Optymistyczne 100% jest scenariuszem unknown=3, a nie dowodem poprawy. Odrzucenie pogarsza się z 1/15 i 3/15 do 0/15 i nie zależy od brakujących ocen: poszerzony katalog zwraca wyniki również na potrzeby poza zakresem. Żadna bramka 95% nie jest spełniona; relewancja i korzyść wymagają niezależnych ocen.

## Pary do niezależnej oceny

Pełna lista: [pairs-to-judge.json](pairs-to-judge.json): **1187 unikalnych par intent × pełne ID**, w tym **751 z nowych źródeł**, 436 nieocenionych z dotychczasowych. Lista obejmuje unię top5 obu metod i ramion; `runs`, `family`, `fixture` i `expected` wskazują dokładne użycie. Jest to materiał do budowy puli; przed oceną należy usunąć ranking/proweniencję retrieval, losowo uporządkować i zebrać kod oraz kontekst. Dwa niezależne modele, świeże ślepe konteksty i osobny arbitraż; żadnych automatycznie dopisanych ocen w tym zadaniu.

| Rodzina z luką | Pary wszystkich źródeł | Pary nowych źródeł |
| --- | ---: | ---: |
| charts | 59 | 41 |
| records | 51 | 37 |
| upload | 53 | 36 |
| sidebar | 42 | 30 |
| pricing | 78 | 40 |
| hero | 72 | 32 |
| comparison | 53 | 29 |
| numbers | 57 | 28 |

Dokładne komponenty i intencje są w JSON; przykładowe priorytetowe nowe pary:

- charts: `d21-1` × `beui/line-chart`; `d21-1` × `cult/analytics-chart`; `d21-1` × `devl/sales-overview`.
- records: `d01-1` × `avara/invoice`; `d01-1` × `blocks-so/table-05`; `d01-1` × `reui/data-grid-table`.
- upload: `d07-1` × `avara/upload`; `d07-1` × `beui/attachment-upload`; `d07-1` × `blocks-so/file-upload-01`.
- sidebar: `d12-1` × `avara/sidebar`; `d12-1` × `blocks-so/sidebar-01`; `d12-1` × `shadcn-space/collapsible-02`.
- pricing: `d08-1` × `avara/pricing-plan`; `d08-1` × `ruixen/pricing-cards-tooltip`; `d08-1` × `ruixen/pricing-plans`.
- hero: `d09-1` × `avara/support-hero`; `d09-1` × `devl/sign-up`; `d09-1` × `ruixen/product-card-hero`.
- comparison: `d17-1` × `avara/image-compare`; `d17-1` × `beui/before-after-slider`; `d17-1` × `motion-primitives/image-comparison`.
- numbers: `d15-1` × `beui/animated-number`; `d15-1` × `cult/rolling-number`; `d15-1` × `vengeance/animated-number`.

## Etap 2 — pominięte w tej zmianie

Opcjonalne loadery odłożono, aby zakończyć i rzetelnie zweryfikować pełny etap 1. Poniżej konkretne brakujące kontrakty; liczby pochodzą z ocen A–E i nie oznaczają wyniku sync w tej zmianie.

| Biblioteka | Licencja ocenionego zakresu | Zakres / dlaczego nie dodano |
| --- | --- | --- |
| HeroUI | Apache-2.0 | 83 moduły, style i kontekst providerów; potrzebny loader per komponent z CSS, bez Pro i React Native. |
| Headless UI | MIT | 24 moduły; Single byłby możliwy, ale limit 120 plików obecnego downloadera nie gwarantuje kompletności szerokiego drzewa pakietu. Nie dodano całej biblioteki jako jednej pozornie kompletnej próbki. |
| Tremor | Apache-2.0 | 39 folderów copy/paste, własne utils i zależności lokalne; stary @tremor/react to inny artefakt. |
| HyperUI | MIT | 572 HTML z wariantami dark; wymaga loadera HTML, jawnej informacji o adaptacji do React i wyboru wariantów. |
| TailGrids | MIT repo | 61 wpisów ze ścieżkami raw bez content; potrzebne przypięcie repo i kompletu źródeł, bez rozciągania MIT na płatną witrynę. |
| Bencho | MIT kodu, wyłączone marki/zdjęcia | 47 folderów TSX+CSS; potrzeba loadera repo z CSS i THIRD_PARTY, nie 168 pozycji galerii/Pro. |
| EasyUI | MIT | 134 wpisy bez content i niedziałające itemy; własne pobieranie wszystkich ścieżek raw i zależności. |
| Animata | MIT | Brak indeksu publikowanych JSON, 215 plików repo nie jest listą opublikowanych komponentów; należy odtworzyć mapę docs i bundlowanie helperów/CSS. |
| Feral UI | MIT wybranych repo | Brak wspólnego registry; pullcord/playcaptcha/feral-blob wymagają oddzielnego zweryfikowania pakietu, katalogu kodu, stylów i USAGE. Nie indeksowano nieopublikowanych demonstracji. |
| Tailark | MIT repo | Live katalog miesza płatne/401; potrzebny loader wyłącznie publicznego repo, rozdzielenie Base/Radix i not upstream. |

## Walidacja i ryzyka

- `cargo test --locked`: 24 unit + 71 integration, wszystkie zielone; test stałego indeksu zachowuje dotychczasową kolejność i membership domyślnego wyszukiwania.
- `npm run test:node`: 27 testów, wszystkie zielone.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `python3 scripts/third-party-notices.py --check`, `git diff --check`: PASS.
- Mocki obejmują wszystkie 23 URL-e/specyfikacje, filtry typów/free/pro, ReUI 401 i bloki premium, przecięcie Shadcn Space, kontrolę paid przy get, Origin tags, silnik/cykle/CSS asciicn i brak pobierania obcego hosta.
- README, dystrybucyjny SKILL i docs/sources.md opisują bibliotekę i ograniczenia. Nie zmieniano globalnie zainstalowanych skilli. THIRD_PARTY_NOTICES i generator dotyczą Cargo; brak nowych zależności Cargo lub bundlowanego upstream kodu uzasadnia brak zmian ich zawartości.
- Nie wykonano browser/visual QA, buildów pobranych komponentów, audytu dostępności ani pełnego testu każdego nowego item endpointu. Źródła zdalne są zmienne, a brak opisu/preview/dependency nie jest ukrywany.
- Spectrum zawiera zaadaptowany kod innych bibliotek; Apache projektu nie zmienia praw upstream. devl.dev ma osobną notę visx MIT. Zależności, fonty, obrazy i płatne produkty zachowują własne warunki. Licencja źródłowego repo nie jest audytem praw każdego assetu.

## Poza zakresem

Naprawa dotychczasowego RareUI 404 i duplikatu Aceternity; zmiany rankingu/wag; anotacja i arbitraż par; sealed evaluation; instalacja komponentów; naprawy kodu upstream; push, publikacja, PR i deployment. Odrzucone w ocenach A–E źródła nie zostały dodane (w tym React Bits, Animate UI, shadcnblocks, shadcn studio, Prism, Scrolltide, transitions.dev, Skiper, ReverseUI, Obsidian, Bundui, Efferd, OriginKit, Uiverse, AI SDK Elements, Open Components, shadway i shsfui).

## Odtwarzanie

Komendy i granice pomiaru: [README.md](README.md). `probe.py`, `sync.py`, `smoke.py`, `retrieve.rs`, `score.mjs` i `write-report.py` są artefaktami eksperymentu. Wymagany zewnętrzny dev jest tylko czytany. Hash rankera jest taki sam jak w bazie; żadnego dostępu do ui-component-finder-eval-sealed.
