# Clay UI — jakość wyszukiwania 0.2.1

**Stan: wydane w 0.2.1.** Ten katalog zawiera tylko raport. Pełne dowody (128 MB, w tym pobrany kod komponentów z cudzych bibliotek, pakiety sędziów i logi odbioru) zostają poza publicznym repozytorium, więc linki względne do nich tu nie działają.

Gałąź `t3/quality-brief-implementation`. Baza main/0.2.0: `4611e920db972845c1e5cdc5e9f4666b9784bdbf`. HEAD podczas generowania: `87d037cfa127be497deb1296cd87f350652ae7e5`. Wersji produktu nie podniesiono.

## Odpowiedź

Proza jest ręcznym opisem zamkniętego eksperymentu; tabele są generowane z JSON. Zmiana ocen lub konfiguracji wymaga ponownej oceny także prozy.
**Nowe źródła poprawiły potwierdzoną trafność BM25 na dev.** Success3@5 wzrósł z 64.8% do 81.0%, +16.2 pp, paired family 95% CI [+2.9; +30.5]. Gemini wzrósł z 69.5% do 79.0%, lecz CI zmiany [−0.9; +20.9] obejmuje zero; jego ContextSuccess poprawił się z 55.0% do 73.3%, CI [+6.0; +33.3] pp.
Sama ekspansja pogorszyła odrzucanie do 0/15. Przyjęta kalibracja przywraca **11/15 BM25 i 9/15 Gemini**, bez utraty Success ani Context w obu granicach. To eksploracyjny wynik dev, nie dowód 95% ani wynik zapieczętowanego testu.

## Zakres, wersje i ochrona danych

120 zapytań EN (105 answer, 15 reject), 24 rodziny. PL i EN są jedną potrzebą i obie wersje widzą sędziowie. Utrwalone krótkie zapytania i routing `--source base-ui` dla 15 przypadków są symulacją przygotowania zapytania przez agenta; nie są pełnym agentowym E2E.
Katalog 0.1.1: 13 źródeł, 678 wierszy / 677 unikalnych ID. Katalog 0.2.0: 36 źródeł, 5827 / 5826. RareUI ma błąd 404 w obu. Search/embeddings 0.1.1 i 0.2.0 są identyczne. Replay używa produkcyjnego Rust rankera i readera klucza, tego samego modelu gemini-embedding-001, 768 wymiarów, zachowanych prawdziwych wektorów. Nie mierzono kosztu pierwotnego wygenerowania wektorów.
W obu ramionach Map normalizuje istniejący duplikat Aceternity/background-lines, jak wcześniejszy harness. Pełne listy (depth 10000) są zachowane jako gzip; top5 wszystkich 480 referencyjnych zapytań zgadza się dokładnie z historycznym pomiarem (replay-check.json). To porównanie funkcji rankującej; osobny readback rzeczywistego CLI opisano poniżej.
Oryginalny eval jest wyłącznie odczytywany, a jego 17 zamrożonych wejść ponownie hashowanych. Nakładka ocen jest osobna. Nie otwierano, nie listowano ani nie hashowano holdoutu. Nie pushowano, nie tagowano i nie publikowano.

## Niezależne oceny i źródła

Pierwotne 1187 par dotyczyło 744 komponentów. Dla pełnego porównania wariantów dodano 547 par / 326 nowych komponentów: razem **1734 pary, 1070 unikalnych komponentów, 88 pakietów A/B**. Dowody pierwotnego pobrania: 993 z kodem, 77 unresolved. Źródła mogły zmienić się od snapshotu katalogu; zachowano dokładną pobraną treść.
Kod i dokumentacja pochodzą z `clayui show <id> --code --json`. Checkpoint i retry po komponencie; brak wykonywania kodu komponentów. Dziewięć komponentów Radix odzyskano po GitHub 403 przez istniejący token gh przekazany tylko środowiskiem, bez zapisu sekretu; 22 pary niezależnie oceniono ponownie na odzyskanej rewizji. Czterech dalszych niedostępnych komponentów nie udało się odzyskać. Pierwotne dowody i oceny zachowano.
A: gpt-6.1-sol. B: claude-opus-5-5. Świeży kontekst na pakiet, inny model dla A/B, niezależny arbiter w trzecim kontekście. Bez rankingu, score, metody, nowości źródła i cudzych ocen; sędzia widzi potrzeby PL/EN, fixture, kod i dokumentację. Nazwy bibliotek w samym kodzie są dowodem technicznym. Losowanie seed 20261115 (pierwotne) i 20261116 (dodatkowe); pakiety zapisane falami, następnie niezmienne.
Modele sprawdzono próbami JUDGE_A_READY / JUDGE_B_READY. CLI codex 0.169.0 i Claude Code 2.1.325 są dostępne, lecz nadrzędny runtime wymagał natywnych świeżych agentów GPT i delegacji T3 Claude zamiast bezpośredniego wywołania procesów modeli. Identyfikatory i ponowienia są w runner-*.json; oceny w judgments/.
Niepoprawne odczyty lub oceny bez kodu odrzucono, zachowując oryginały i świeże korekty. Dotyczy m.in. B batch036 oraz odczytów długich linii 039/041/046/055/056. Kolejne ograniczenia odczytu w 061–088 zastąpiono świeżymi próbami. Źródło bez niezbędnego prymitywu/importu może pozostać unresolved; drobny helper cn sam nie uzasadnia braku liczby (korekta arbitrażu005). Reguły precedencji i historia napraw harnessu są w AUDIT.md i test_reconcile.py.

## Zgodność sędziów

A=B=1734 ocen. Pary z liczbą u obu: 1347; zgodność dokładna 74.7%; ±1 98.7%; kappa kwadratowa 0.888877.
Rozbieżności 480, arbitraż ukończony (41 pakietów), oczekujące 0. Końcowo 1338 ocen liczbowych, 396 unresolved, pokrycie liczbowe 77.2%. Complete oznacza zakończoną procedurę, nie zastąpienie null zerem.

| A \ B | 0 | 1 | 2 | 3 |
| --- | ---: | ---: | ---: | ---: |
| 0 | 379 | 7 | 0 | 0 |
| 1 | 36 | 142 | 0 | 0 |
| 2 | 3 | 172 | 233 | 37 |
| 3 | 0 | 15 | 71 | 252 |

## Wyniki przed i po

Unknown to 0 tylko w zachowawczej granicy obliczeń i 3 w optymistycznej; nie zmienia to qrels. Context ma mianownik 60 potrzeb z kontekstem. Rejection nie zależy od ocen. Wszystkie nowe pary wariantów otrzymały dwie oceny i arbitraż, włącznie z jawnymi unresolved.

| Ramię | Success dolna / opt. | Context dolna / opt. | Rejection | Unjudged top5 |
| --- | ---: | ---: | ---: | ---: |
| baseline-bm25 | 64.8% / 93.3% | 53.3% / 91.7% | 6.7% | 142 |
| expanded-bm25 | 81.0% / 95.2% | 73.3% / 93.3% | 0.0% | 101 |
| baseline-gemini | 69.5% / 80.0% | 55.0% / 70.0% | 20.0% | 24 |
| expanded-gemini | 79.0% / 88.6% | 73.3% / 83.3% | 0.0% | 69 |
| bm25-count-678-idf-ratio-0.25 | 81.0% / 95.2% | 73.3% / 93.3% | 73.3% | 98 |
| gemini-count-678-threshold-0.66 | 79.0% / 88.6% | 73.3% / 83.3% | 60.0% | 69 |

Bootstrap: 10 000 losowań rodzin, seed 20261109; percentyle 2.5/97.5. Dla rejection są tylko trzy rodziny, część replik bez reject jest pomijana (9594 ważne). Zdegenerowany CI [0;0] nie dowodzi pewności odrzucania. Wszystkie marginalne CI i per-query: metrics/*.json.

| Ramię | 95% CI Success | 95% CI Context | 95% CI Rejection |
| --- | ---: | ---: | ---: |
| baseline-bm25 | [+47.8; +81.0] | [+28.0; +80.0] | [+0.0; +20.0] |
| expanded-bm25 | [+70.9; +90.0] | [+58.0; +87.1] | [+0.0; +0.0] |
| baseline-gemini | [+53.3; +84.4] | [+31.7; +78.5] | [+0.0; +40.0] |
| expanded-gemini | [+68.6; +88.7] | [+56.9; +88.6] | [+0.0; +0.0] |
| bm25-count-678-idf-ratio-0.25 | [+70.9; +90.0] | [+58.0; +87.1] | [+40.0; +100.0] |
| gemini-count-678-threshold-0.66 | [+68.6; +88.7] | [+56.9; +88.6] | [+20.0; +80.0] |

Zmiany sparowane względem poprzedniego katalogu lub niezmienionego expanded rankera:

- expanded-bm25, conservative: success3at5 +16.2 pp [+2.9; +30.5]; contextSuccess3at5 +20.0 pp [+0.0; +42.2]; correctRejection -6.7 pp [-20.0; +0.0]
- expanded-bm25, optimistic: success3at5 +1.9 pp [-5.3; +10.5]; contextSuccess3at5 +1.7 pp [-10.8; +16.7]; correctRejection -6.7 pp [-20.0; +0.0]
- expanded-gemini, conservative: success3at5 +9.5 pp [-0.9; +20.9]; contextSuccess3at5 +18.3 pp [+6.0; +33.3]; correctRejection -20.0 pp [-40.0; +0.0]
- expanded-gemini, optimistic: success3at5 +8.6 pp [+1.0; +18.9]; contextSuccess3at5 +13.3 pp [+0.0; +30.0]; correctRejection -20.0 pp [-40.0; +0.0]
- bm25-count-678-idf-ratio-0.25, conservative: success3at5 +0.0 pp [+0.0; +0.0]; contextSuccess3at5 +0.0 pp [+0.0; +0.0]; correctRejection +73.3 pp [+40.0; +100.0]
- bm25-count-678-idf-ratio-0.25, optimistic: success3at5 +0.0 pp [+0.0; +0.0]; contextSuccess3at5 +0.0 pp [+0.0; +0.0]; correctRejection +73.3 pp [+40.0; +100.0]
- gemini-count-678-threshold-0.66, conservative: success3at5 +0.0 pp [+0.0; +0.0]; contextSuccess3at5 +0.0 pp [+0.0; +0.0]; correctRejection +60.0 pp [+20.0; +80.0]
- gemini-count-678-threshold-0.66, optimistic: success3at5 +0.0 pp [+0.0; +0.0]; contextSuccess3at5 +0.0 pp [+0.0; +0.0]; correctRejection +60.0 pp [+20.0; +80.0]

## Strojenie i decyzja

43 warianty początkowe + 9 follow-up, osobno dla obu metod. Jedna reguła zmieniana naraz. Wyniki i CI wszystkich wariantów: summary.json, adaptive-summary.json, metrics/. Zmiany wybierano na dev; brak korekty wielokrotnych porównań i adaptacyjny dobór hipotez ograniczają interpretację CI.
**Przyjęto** dwa progi wyłącznie dla eligible pool >678 wierszy (rozmiar dawnego katalogu, bez nazwy/popularności biblioteki): raw cosine >=.66 w Gemini, raw BM25 >=.25 sum(IDF deduplikowanych rozszerzonych terminów). Mała pula zachowuje .60 / dodatni score. IDF nadal liczone przed filtrowaniem. Zaokrąglenie do .001 jest wyłącznie wyjściowe; bardzo częsty dokładny termin nie znika przez wynik wyświetlany jako 0.
Follow-up wynikał z pięciu utraconych gotowych trafień przy jednolitym .66, wszystkich w małym source-scope. Żadnej utraty obserwowanej przy progach zależnych od wielkości. Ratio 1/3 i .375 poprawiają rejection bardziej, ale tracą po jednym gotowym trafieniu, więc odpadają mimo formalnej reguły braku regresji ponad szum. Cel briefu preferuje zero utraty Success.
**Odrzucono** source cap 1/2/3 (wyższe surowe Success, bez istotnego wspólnego zysku), exact-code dedup (brak przewagi), wyłączenie lub karę przykładów i blokujących gaps (brak przewagi), brakujące importy i licencje (brak dowodu korzyści). Niepełne cechy z 744 pierwotnych komponentów zamrożono; wzrost dowodów nie zmienił żadnego z 47 zestawów wyników. Ten częściowy feature map nie trafia do produktu. Brak licencji/importów pozostaje nieblokujący, poza już istniejącym jawnym available-only dla statycznych gaps.

| Wariant | Success dolna | Context dolna | Rejection | Unknown top5 | Decyzja |
| --- | ---: | ---: | ---: | ---: | --- |
| bm25-min-score-1 | 81.0% | 73.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-2 | 81.0% | 73.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-4 | 81.0% | 73.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-6 | 73.3% | 60.0% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-8 | 72.4% | 58.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-10 | 66.7% | 50.0% | 33.3% | 96 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-12 | 64.8% | 48.3% | 60.0% | 84 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-16 | 41.0% | 26.7% | 93.3% | 36 | odrzucony: brak przewagi ponad szum |
| bm25-min-score-20 | 19.0% | 10.0% | 100.0% | 17 | odrzucony: brak przewagi ponad szum |
| bm25-source-cap-1 | 83.8% | 76.7% | 0.0% | 91 | odrzucony: brak przewagi ponad szum |
| bm25-source-cap-2 | 81.9% | 73.3% | 0.0% | 100 | odrzucony: brak przewagi ponad szum |
| bm25-source-cap-3 | 81.0% | 73.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-without-examples | 76.2% | 68.3% | 0.0% | 106 | odrzucony: brak przewagi ponad szum |
| bm25-without-code-blocking | 81.0% | 73.3% | 0.0% | 80 | odrzucony: brak przewagi ponad szum |
| bm25-exact-code-dedup | 81.0% | 73.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-penalty-example | 79.0% | 71.7% | 0.0% | 112 | odrzucony: brak przewagi ponad szum |
| bm25-penalty-missing-local-imports | 73.3% | 63.3% | 0.0% | 128 | odrzucony: brak przewagi ponad szum |
| bm25-penalty-missing-license | 81.0% | 73.3% | 0.0% | 101 | odrzucony: brak przewagi ponad szum |
| bm25-term-coverage-0.5 | 57.1% | 41.7% | 100.0% | 62 | odrzucony: brak przewagi ponad szum |
| bm25-term-coverage-0.75 | 16.2% | 10.0% | 100.0% | 5 | odrzucony: brak przewagi ponad szum |
| bm25-term-coverage-1 | 0.0% | 0.0% | 100.0% | 1 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.62 | 78.1% | 71.7% | 20.0% | 68 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.64 | 78.1% | 71.7% | 26.7% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.66 | 74.3% | 65.0% | 60.0% | 69 | odrzucony: utrata trafień |
| gemini-min-similarity-0.68 | 69.5% | 58.3% | 86.7% | 67 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.7 | 60.0% | 45.0% | 100.0% | 58 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.72 | 42.9% | 25.0% | 100.0% | 37 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.74 | 21.9% | 6.7% | 100.0% | 25 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.76 | 9.5% | 3.3% | 100.0% | 13 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.78 | 4.8% | 1.7% | 100.0% | 1 | odrzucony: brak przewagi ponad szum |
| gemini-min-similarity-0.8 | 0.0% | 0.0% | 100.0% | 0 | odrzucony: brak przewagi ponad szum |
| gemini-source-cap-1 | 82.9% | 73.3% | 0.0% | 52 | odrzucony: brak przewagi ponad szum |
| gemini-source-cap-2 | 79.0% | 73.3% | 0.0% | 63 | odrzucony: brak przewagi ponad szum |
| gemini-source-cap-3 | 79.0% | 73.3% | 0.0% | 67 | odrzucony: brak przewagi ponad szum |
| gemini-without-examples | 80.0% | 73.3% | 0.0% | 61 | odrzucony: brak przewagi ponad szum |
| gemini-without-code-blocking | 79.0% | 73.3% | 0.0% | 68 | odrzucony: brak przewagi ponad szum |
| gemini-exact-code-dedup | 79.0% | 73.3% | 0.0% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-penalty-example | 80.0% | 73.3% | 0.0% | 62 | odrzucony: brak przewagi ponad szum |
| gemini-penalty-missing-local-imports | 72.4% | 65.0% | 0.0% | 81 | odrzucony: brak przewagi ponad szum |
| gemini-penalty-missing-license | 79.0% | 73.3% | 0.0% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-no-curated-bonus | 77.1% | 70.0% | 0.0% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-blocking-gap-0.16 | 79.0% | 73.3% | 0.0% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-blocking-gap-0.24 | 79.0% | 73.3% | 0.0% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-count-678-threshold-0.64 | 79.0% | 73.3% | 26.7% | 69 | odrzucony: brak przewagi ponad szum |
| gemini-count-678-threshold-0.66 | 79.0% | 73.3% | 60.0% | 69 | przyjęty |
| bm25-count-678-idf-ratio-0.25 | 81.0% | 73.3% | 73.3% | 98 | przyjęty |
| bm25-count-678-idf-ratio-0.3333333333333333 | 80.0% | 71.7% | 93.3% | 94 | odrzucony: utrata trafień |
| bm25-count-678-idf-ratio-0.375 | 80.0% | 71.7% | 100.0% | 93 | odrzucony: utrata trafień |
| bm25-count-678-idf-ratio-0.5 | 64.8% | 58.3% | 100.0% | 74 | odrzucony: brak przewagi ponad szum |
| bm25-count-678-idf-ratio-0.625 | 57.1% | 51.7% | 100.0% | 50 | odrzucony: brak przewagi ponad szum |
| bm25-count-678-idf-ratio-0.75 | 35.2% | 35.0% | 100.0% | 22 | odrzucony: brak przewagi ponad szum |
| bm25-count-678-idf-ratio-1 | 17.1% | 26.7% | 100.0% | 4 | odrzucony: brak przewagi ponad szum |

## Implementacja i odbiór

Minimalna zmiana src/search.rs i src/cli.rs. Referencyjne rank/RANKING pozostają do replay; CLI używa kalibrowanych defaultów. Bez nowego API, query-time LLM, nowych zależności, flag ani pól JSON. e2e/parity zachowuje wszystkie porównania małego katalogu i dodaje dwa jawne testy różnicy Rust-only na dużym katalogu. Cztery nowe Rust testy wykonują reguły graniczne, puste wejścia, raw cosine przed bonusem i raw BM25 przed zaokrągleniem.
Proces lokalnego odbioru, trace wszystkich wymagań, role, wyniki review i testów: acceptance/journal.md i .agents/task-batch-delivery.md. Brak UI uzasadnia rzeczywiste procesy CLI zamiast przeglądarki. Usługa live 21st jest jawnie wyłączona lokalną pustą odpowiedzią, jak w pomiarze; wszystkie inne brakujące wywołania sieciowe są odrzucane. Katalog i wektory są prawdziwymi utrwalonymi danymi, nie mockami rankera. Nie dowodzi to dostępności live usług.

Rzeczywisty CLI readback: complete=True, 240 procesów, różnice od przyjętej polityki=0, nieoczekiwane cache misses=0. SHA binarium i cache oraz pełne stdout/exit: acceptance/cli-author/.
Pełna bramka techniczna complete=True: cargo fmt --check (exit 0), cargo clippy --locked --all-targets -- -D warnings (exit 0), cargo test --locked (exit 0), npm run test:node (exit 0), cargo build --locked (exit 0), npm run parity (exit 0), python3 scripts/third-party-notices.py --check (exit 0), python3 -m unittest discover -s experiments/quality-0.2.1 -p test_*.py (exit 0). Logi gate-*.log.

Status końcowy odbioru: PASS: review ready/assessed; niezależny tester wykonał replay 240 i 29 dodatkowych procesów; weryfikator potwierdził Q1–Q8, ponownie wykonał pełną bramkę (99 Rust, 27 Node, 886 parity, 10 integrity), odtworzył 6 ramion z CI i powtórzył 29 procesów CLI. Parent sprawdził 41 faktycznych wywołań narzędzi. Szczegóły: [raport weryfikatora](acceptance/verification/report.md) i [ocena dowodów](acceptance/verification/assessment.md).

## Ryzyka i poza zakresem

- 396 par unresolved i 98/69 unknown top5 utrzymują szerokie granice; nie wolno twierdzić, że katalog nie ma rozwiązania. Brak potwierdzonego dopasowania nie jest potwierdzeniem braku dopasowania.
- 15 reject / trzy rodziny, statystyczny wybór na tym samym dev, skok progu przy 678/679 i snapshot sprzed pobrania kodu ograniczają uogólnienie. Nowe biblioteki oraz zapytania naturalnym językiem mogą wymagać nowej niezależnej oceny.
- Cached Gemini replay potwierdza rankowanie i reader klucza, nie świeży koszt/czas API ani dostępność usług. Baseline nie obejmuje live 21st, a pobranego kodu nie uruchamiano w docelowej aplikacji. To nie certyfikuje integracji agenta.
- Poza zakresem: holdout, zmiany oryginalnego eval, naprawa upstreamów/premium, push/tag/publikacja, wdrożenie TEST/PROD, paczki innych OS i smoke po wydaniu. Wersja nadal 0.2.0; wydanie prowadzi agent nadrzędny.
