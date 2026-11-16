# Spec 20 — Afterbattle v2.0

- **Status: APPROVED (2026-11-09)**: zaakceptowane przez autora; implementacja od E0. Zastępuje wersję z 2026-11-08, która opisywała archiwalny `e0982a7`.
- Baza kodu: `main` @ `8ba16b0` (decyzja autora z 2026-11-09). README z `877ad59` (gałąź `feature/replay-analytics-alpha-0.2`) jest odłożone jako decyzja redakcyjna.
- Podstawa: wewnętrzny audyt autora z 2026-11 (poza repozytorium publicznym). Identyfikatory w nawiasach (M1, N2, D2…) to znaleziska z audytu, a D1–D16 w §4 to decyzje tego speca.
- Wykonawca: Sonnet. Autor speca: architekt/recenzent.

### Zmiany 2026-11-11, zatwierdzone przez autora

Podstawa: wewnętrzne badanie podwalin autora (poza repozytorium publicznym; dalej „badanie”). Status speca bez zmian: APPROVED.

- **A. Odpowiedzi na §11 badania** (§9 „Rozstrzygnięte”): P1 tak (anonimowy skład bitwy); `marks_on_gun` innych graczy nie; tier przed E2 jako osobny etap E1b; ścieżka B tylko prywatnie (D18); archiwizacja `entity_defs` tak, lokalnie u autora (D18); pamięć podręczna `battle_results` nie teraz; nazwa funkcji modelu „Co różni Twoje wygrane od przegranych” (D20); sprzęt do LLM przy specu debriefu; ekonomia trwale poza zakresem; premium nie teraz.
- **B. D17:** anonimowy skład w tabeli `battle_roster` (bez `marks_on_gun`) i agregaty drużyn w `battle_team_stats`, wykluczone z eksportu; jawnie zastępuje zakazy z kontraktu team-relative §20 i §36 w zakresie opisanym w D17.
- **C. Nowy etap E1b** „Tier i role w katalogu pojazdów” (5 h) między E1 a E2; dawny §11.1 przeniesiony do E1b.
- **D. E2 rozszerzone o podwaliny v10:** kolumny własnego gracza, `battle_team_stats`, `battle_roster`, `replay_files.packet_stream_bytes` (18 → 27 h).
- **E. E5 zmienia zakres na „Redesign UI”** według zaakceptowanej makiety, z krokiem „Grafiki z instalacji gry” i decyzją D19; E6 włączony do E5 (20 + 4 → 49,5 h).
- **F. §11 przepisany:** tabele odniesienia przed modelem, lista „później / nie robimy”, zasady ścieżki B, nowy punkt o heatmapach wykryć.
- **G. Szacunek godzin przeliczony** (§10).
- **H. Rozstrzygnięcie 11 sprzeczności** zauważonych przy tej zmianie (§9 „Rozstrzygnięte”, „Sprzeczności 2026-11-11”).

### Zmiany 2026-11-14

- **Ścieżka B (decyzja autora):** treść §11.4–11.5 i opisy techniki (D18, §2, odpowiedzi 4–5 i punkt 7 w §9, wstęp §11) przeniesione bez zmian do notatki badawcze autora (poza repozytorium publicznym); tutaj zostają krótkie wzmianki. Status speca bez zmian: APPROVED.
- **§11.3:** odnośnik do [spec 28](28-ai-recap.md) (włączanie, dobór modelu do sprzętu, uruchamianie na żądanie; DRAFT, po 2.0). Status speca bez zmian: APPROVED.
- **Wersja publiczna (decyzja autora):** pierwsze publiczne wydanie to beta `1.0.0-beta.1` („Afterbattle beta 1.0”, kanał „Public beta” / `public_beta`); wewnętrznie bieg i etapy nazywają się dalej „v2” / E0–E8. Jedno źródło wersji bez zmian (`src-tauri/Cargo.toml`, D11). Tag `v1.0.0-beta.1`, wydanie na GitHubie oznaczone jako pre-release. Podpis Authenticode i updater — wymagane dopiero dla stabilnego 1.0.0.

### Zmiany 2026-11-15

- **E9 — dopracowanie do `1.0.0-beta.2` (decyzja autora):** zakres, kolejność etapów i kryteria akceptacji w [spec 29](29-e9-polish.md) (APPROVED): regresja ruchu w Czołgach i Historii, odświeżanie na żywo, paleta, logo i pasek przewijania, ekran ładowania, ruch nowej bitwy i liczb, wydanie `1.0.0-beta.2` („Afterbattle beta 2”, pre-release). Pliki marki w `docs/design/brand/`. Bez zmian w parserze i bazie. Status speca bez zmian: APPROVED.
- **§6.3 (decyzja autora):** mutacje nie blokują wydania. Próg ≥ 85% obowiązuje dla logiki nowej lub zmienionej w etapie; starsze moduły są w wewnętrznym backlogu mutacji autora, `baseline.rs` jako warunek startu moonshota A-1 (§11.1). Checkpoint E4 (mutacje średniej odniesienia, `session_insights`, generatora wyzwań) przestaje być bramką E7.

## 1. Cel v2.0

v2.0 to **konsolidacja i naprawa tego, co już jest na `main`, podwaliny danych pod „Wizję po v2.0” (tier i role w katalogu pojazdów, pełniejsza baza v10) oraz redesign interfejsu, bez nowych metryk analitycznych.**

Afterbattle 2.0 ma dawać poprawne statystyki **wyłącznie bitew losowych**, liczone z lokalnych powtórek: WN8, procent wygranych, przetrwanie, XP, obrażenia, asysta oraz istniejące analizy (średnie odniesienia, trendy, tempo bitwy, celność i przebicia, udział w drużynie, wyzwania). Poza tym:
- nie może się zawieszać, gubić bitew ani tracić danych przy uszkodzonej bazie;
- nie może łamać własnej deklaracji prywatności;
- mówi zrozumiałą polszczyzną.

Odbiorcy: autor i mała grupa znajomych. Architektura i proces wydania nie mogą blokować późniejszego publicznego wydania.

## 2. Stan obecny i zakres

### 2.1 Co ze starego zakresu już istnieje

| Stary zakres (8.11) | Stan na `main` | Co zostaje do zrobienia w v2.0 |
|---|---|---|
| **F1 WN8 dla bitwy, sesji i czołgu** | **Jest.** Liczone w locie z lokalnej migawki XVM, wzór pełny z obroną, agregaty z sum, 17 testów z wartościami dokładnymi (`ratings.rs`), utwardzone pobieranie (`expected_values.rs`) | Liczyć tylko bitwy losowe (M2). Pobieranie tylko po włączeniu przez gracza (M9, w E1). Znacznik próby po zakończeniu próby. Tolerancja złych wierszy (M8). Wersja danych XVM pokazana raz, w ustawieniach i stopce (D3). Test referencyjny z niezależnym kalkulatorem |
| **F2 widok czołgu z trendem** | **Jest.** `HistoryBrowser.tsx:129` → `PerformanceTrendsPanel` + `TrendChart` + `BaselinePanel`; backend `performance_trends`, `vehicle_baseline` | Poprawny zbiór bitew (M2, M6), asysta (M4), stany puste i testy. Bez nowych funkcji |
| **F3 coach tygodniowy** | **Nie ma wersji tygodniowej.** Jest inny „coach”: wnioski z sesji (`session_insights.rs`), trendy (`performance_trends.rs`) i wyzwania na 5 bitew na czołgu (`challenges.rs`, `GoalsPage.tsx`, `ChallengeUi.tsx`). WR nie jest celem nigdzie (`challenges.rs:207-231`, test w `performance_trends.rs`) | Naprawa kwalifikacji (M5), asysty (M4) i sygnałów (M10). Raport tygodniowy tylko jako warunkowy E7 |

Audyt nie wykazał luki, która wymagałaby nowej funkcji w v2.0. Jedyny kandydat to raport tygodniowy (E7). Jest warunkowy i zbudowany wyłącznie na istniejących modułach.

### 2.2 Poprawki z audytu

| Priorytet | Znaleziska | Efekt dla gracza |
|---|---|---|
| Krytyczne | M1, M2, M4, T1 | Statystyki liczą tylko bitwy losowe, asysta ma poprawną definicję |
| Krytyczne | D2, N1, N2, N5, T2 | Migracje z kopią, uszkodzona baza nie powoduje cichej utraty bitew ani fałszywego „usunięto dane” |
| Krytyczne, najpóźniej w E1 | M9, N1, N2 | Brak połączeń z XVM bez zgody gracza; bitwy nie przepadają w `%TEMP%`; „Usuń wszystkie dane” nie zgłasza fałszywego sukcesu |
| Krytyczne | K6 (S1), S2 | Brak znanych podatności w zależnościach |
| Krytyczne | A1, P1, P3 | Brak zamrożeń, import w tle, automatyczne doczytanie brakujących bitew |
| Krytyczne | Q1, U1 | Frontend da się utrzymać, błędy po polsku |
| Ważne | A2, S4, S5 (usunięcie kodu), M3, M5–M8, N3–N7, B1, B4, B5, A3–A7, D8–D10, Q2–Q7, Q10, P2, P4, U4–U8, S6, R1–R7, T3, T4, T8 | Zgodnie z audytem |
| Kosmetyczne | M10–M12, N8–N11, B3 | Gdy przy okazji dotykamy danego pliku |

## 3. Czego NIE robimy w v2.0

- Integracji z Tomato.gg i Wargaming API (usuwamy, D2).
- Odczytu `python.log` (usuwamy, D1).
- Updatera, Authenticode, publikacji w sklepie, telemetrii.
- Chmury i AI/LLM.
- Trybów innych niż losowe.
- Ścieżka B „Wizji po v2.0” (D18, notatki badawcze autora (poza repozytorium publicznym)).
- macOS i Linuksa.
- Nowego frameworka UI, biblioteki komponentów ani biblioteki wykresów. Redesign z E5 powstaje w obecnym stosie (React, ręczne SVG).
- Nowych typów wyzwań ani nowych metryk analitycznych. Dane z `battle_roster` są w v2.0 tylko zapisywane; jedynym odczytem nowych tabel jest porównanie sum obu drużyn z `battle_team_stats` w ekranie Bitwa (D17), bez list graczy.
- Przepisywania historii git (D15).

## 4. Decyzje

| ID | Decyzja | Uzasadnienie |
|---|---|---|
| **D1** | Usuwamy całą ścieżkę `python.log`: odczyt, kursor, delty, `pending_deltas`, `enrich_battle`, stan `ambiguous` po stronie zapisu, `json5`, `parse_session_stats`. `ambiguous` zostaje tylko jako wartość czytana ze starych danych. | Na realnym formacie gry ścieżka nie działa (A2). Jest źródłem M3 (wywrócenie raportu), B2 (przepełnienie stosu) i B3. PR Dependabota #1 (json5) zamykamy bez scalania. |
| **D2** | Usuwamy Tomato.gg (`tomato.rs`, `profile.rs`, komendy, UI), WG `application_id` (`advanced.rs`) i martwe komendy z A7 (lista w §7). Przy pierwszym starcie v2.0 kasujemy ich wpisy w Credential Managerze; brak wpisu nie jest błędem. | S4, S5, N6, Q6. Odpadają odczyty sejfu co 10 s i druga usługa sieciowa. |
| **D3** *(zmieniona jawnie 2026-11-09, P7)* | **WN8 liczone przy odczycie** z jednej aktywnej migawki wartości oczekiwanych, jak dziś. Nowa migawka jest aktywowana atomowo (już tak jest). Wersję danych XVM (data z `header.version`) **pokazujemy raz**: w ustawieniach i w stopce aplikacji, nie przy każdej wartości. Wyzwania nadal zamrażają SHA migawki. Usuwamy martwe kolumny `battles.wn8`, `wn8_estimated`, `wnx`. **Nie implementujemy** zapisu wersji wartości oczekiwanych przy bitwie ani wznawialnego przeliczenia. Pobranie: tylko gdy gracz włączył XVM (M9, E1); znacznik próby zapisywany po zakończeniu próby (sukces albo błąd z kodem); ponowienie automatyczne po błędzie najwcześniej po 1 h, po sukcesie po 24 h. | **Zmiana względem 8.11:** tamta decyzja zakładała zapis WN8 w bazie, a kod na `main` liczy WN8 przy odczycie, więc nie ma czego przeliczać ani wznawiać. Cel z 8.11 (każda wartość WN8 ma znane źródło i nigdy nie miesza dwóch zestawów) spełnia jedna atomowo aktywowana migawka. Świadomie akceptowany koszt: WN8 starej sesji może się zmienić po aktualizacji XVM; widoczna wersja danych w ustawieniach i stopce to wyjaśnia. Decyzja autora: P7. |
| **D4** | Bitwa liczy się do statystyk ⇔ `battle_type_raw == 1 AND bonus_type_raw == 1 AND data_quality = 'full' AND feature_schema_version IN (lista dozwolona)`. Jedna funkcja w Ruście (`stats_eligibility`) i jeden fragment SQL (stała) używane przez **wszystkie** moduły. Bramka importu sprawdza `battleType == 1` z liczby, nie z `gameplayID`. Bitwy częściowe zapisujemy, ale do statystyk nie wchodzą. | M1, M2, M6. Dziś każdy moduł filtruje inaczej. Tempo bitwy, konwersja i udział w drużynie już stosują ścisły warunek. |
| **D5** *(P2 oceniona od nowa)* | **Jednorazowe przeliczenie v9 → v10 z powtórek, z kopią `.bak`.** Przebieg: `VACUUM INTO afterbattle.v9.bak`; nowa baza v10; przeniesienie danych gracza (`settings`, `sessions`, `challenges`, `challenge_battles`); pełny import powtórek w tle silnikiem z E2 (paczki po 100, kursor, §4.1) z przypisaniem bitew do istniejących sesji po `arena_unique_id`; na koniec raport „przeliczono N bitew, pominięto M nielosowych, K bez pliku powtórki”. Bitwy bez pliku powtórki zostają tylko w `.bak` (P8). **Bramka P8:** przed przeliczeniem aplikacja liczy K i jego odsetek historii. Jeśli K > 5% bitew w bazie v9, przeliczenie się nie zaczyna; w testach i checkpoincie E2 wykonawca zatrzymuje się i czeka na decyzję autora. **Od v10:** każda migracja przyrostowa, sterowana `user_version`, z kopią `afterbattle.v{n}.bak` przed krokiem. | Audyt §6: bitew nielosowych i asysty w starszych wierszach nie da się poprawić SQL-em, bo brakuje surowych pól. **Zmiana względem 8.11:** przebudowa v9 → v10 zamiast v3 → v4, a dane gracza (wyzwania, sesje, ustawienia) są przenoszone, bo nie powstają z powtórek. Ustalenia N3 są w §4.1. |
| **D6** | Jedno długo żyjące połączenie SQLite za `Mutex`. Komendy dotykające plików lub wielu wierszy są `async` z `spawn_blocking`. Eventy `import-progress`, `data-changed`, `session-ended`. Frontend przestaje odpytywać co 10 s. Auto-koniec sesji w jednym miejscu (wątek trackera). | A1, A3–A6, D4, P2. |
| **D7** | Średnia odniesienia: jak dziś (wcześniejsze bitwy, bez bieżącej sesji), ale z predykatem D4 i **kwalifikacją per metryka**. WN8 wymaga migawki, obrażenia, przetrwanie i asysta nie. | M5, M6. Bez tego wnioski i wyzwania nie działają bez XVM. |
| **D8** | Definicje: asysta = `assist_spotting + assist_tracking`, stun osobno; wynik victory/defeat/draw/unknown (jak dziś); WR = wygrane / bitwy ze znanym wynikiem, remis = nie-wygrana (jak dziś); przetrwanie `deathReason == -1`; XP do średnich = `original_xp`; obrona = `droppedCapturePoints`; WN8 agregatu z sum (jak dziś). | M4, M11. Reszta już jest poprawna i chroniona testami. |
| **D9** | Frontend: najpierw mechaniczne formatowanie (E0), potem rozbicie na `pages/`, `components/`, `hooks/`, `i18n/` (E5). ESLint (`react-hooks`, `no-floating-promises`) i `prettier --check` jako bramki CI. Typy TS generowane przez `ts-rs`. Wykresy zostają w ręcznym SVG (`TrendChart.tsx`). | Q1, Q2, Q3, Q10. |
| **D10** | Błędy: backend wysyła `code` + opcjonalne `params`, teksty PL/EN są we froncie, test kompletności dla każdego wariantu `AppError`. Komunikat mówi, co się stało i co zrobić dalej, bez kodów, angielskich wtrąceń i słów „parser”, „baseline”, „replay”, „tracker”. | U1, U4, Q5. Wymóg autora. |
| **D11** | Wersja `2.0.0` z jednym źródłem prawdy (`Cargo.toml`); `tauri.conf.json`, `package.json` i `src/release.ts` muszą się z nim zgadzać (test). `CHANGELOG.md` uzupełniony o alpha 0.2 i v2.0. `scripts/release.ps1` (build + `SHA256SUMS.txt`). Bez Authenticode. | R1–R4, N10. |
| **D12** | WR jest metryką raportu, nie celem coacha. Stan na `main` już to spełnia; dodajemy test, że żaden generator celów (wyzwania, E7) nie zwraca WR. | P3 z 8.11, utrzymane. |
| **D13** | CI: akcje przypięte do SHA, konfiguracja Dependabota z `github-actions` (tylko w repozytorium prywatnym), zadania `test`, `audit` (`cargo audit`, `pnpm audit --prod`), `build` (instalator + SHA-256), ochrona gałęzi `main`. | S6, R6. R5 jest już naprawione. |
| **D14** | Pełny import wszystkich powtórek bez limitów 90 dni i 500 plików; przy starcie przyrostowe doczytanie plików nieobecnych w `replay_files` albo zmienionych (`size`, `mtime`); hash tylko z bloków JSON. Bitwy z importu starsze niż limit bezczynności nie tworzą sesji. | P1, P3, D9 z audytu. |
| **D15** | Nie przepisujemy historii git. Publiczne repo, jeśli powstanie, startuje od jednego czystego commita (stan po v2.0). | P6 utrzymane. Audyt potwierdził prawdziwy nick i ID w historii (`33fc853`, `2812b5e`). |
| **D16** | Tryb awaryjny bazy, w dwóch krokach. **E1 (zabezpieczenie):** w trybie awaryjnym tracker nie startuje (nic nie trafia do bazy w `%TEMP%`); baza awaryjna ma jedną stałą ścieżkę, kasowaną przy następnym udanym starcie; `delete_all_data` w trybie awaryjnym odmawia z komunikatem po polsku zamiast zgłaszać sukces. **E2 (odtworzenie):** plik, którego nie da się otworzyć, jest przenoszony do `afterbattle.corrupt-<ts>.sqlite3`, powstaje nowa baza w docelowej lokalizacji i rusza pełny import z powtórek; `delete_all_data` usuwa też `.corrupt-*` i `.bak` po potwierdzeniu. | N1, N2, D3 z audytu. To utrata danych i fałszywy komunikat, więc zabezpieczenie nie może czekać na E2. Powtórki są źródłem prawdy, więc odtworzenie jest bezpieczniejsze niż praca na bazie w `%TEMP%`. |
| **D17** *(2026-11-11)* | **Anonimowy skład bitwy (P1 z badania §3.3)** w nowej tabeli `battle_roster`, 30 wierszy na bitwę, kolumny według badania §8.2 D **bez `marks_on_gun`**. Numer `slot` 0–29 nadawany deterministycznie (strona, obrażenia malejąco, `typeCompDescr`, potem pozostałe pola), nigdy z identyfikatorów gry; `killer_slot` i `platoon_group` to lokalne numery w obrębie bitwy. Nie zapisujemy nicków, `accountDBID`, klanów, surowego `prebattleID` ani identyfikatorów pojazdów w bitwie, także w postaci hasza. D17 obejmuje wprost także **`battle_team_stats`**, w tym liczniki tierów i klas obu drużyn. Obie tabele są **wykluczone z eksportu**. **UI w v2.0** pokazuje wyłącznie agregaty drużyn (porównanie drużyn w ekranie Bitwa), **nigdy listy graczy składu**. W E2 `PRIVACY.md` i `docs/PRIVACY_PL.md` dostają jedno zdanie o lokalnym, anonimowym składzie bitew gracza. | Decyzja autora (badanie §11 pkt 1–2; rozstrzygnięcia 2026-11-11 pkt 1–2). Tabele odniesienia per tier i klasa (§11.1) wymagają statystyk wszystkich uczestników. **Decyzja jawnie zastępuje zakazy z kontraktu team-relative §20** („roster rows”, „other-player vehicle identities”, „team composition”) **i §36** („no roster persistence”, brak „roster row” i „team composition” w fixture'ach) **wyłącznie dla zapisu w `battle_roster` i `battle_team_stats` oraz dla syntetycznych fixture'ów tych tabel.** Zakaz §36 dla DTO i UI w odniesieniu do list graczy obowiązuje dalej. `team_relative_facts` i jej kontrakt zostają bez zmian. `marks_on_gun` odpada jako jedyne pole mówiące coś o graczu, a nie o bitwie. |
| **D18** *(2026-11-11)* | **Ścieżka B tylko prywatnie**, zasady w notatki badawcze autora (poza repozytorium publicznym). | Decyzja autora 2026-11-11. |
| **D19** *(2026-11-11)* | **Grafiki z instalacji gry** (ikony pojazdów, grafiki map) czytane lokalnie z folderu wybranego przez gracza. Nigdy w repo, fixture'ach, instalatorze, eksporcie ani zrzutach do dokumentacji i portfolio (tam zamienniki). Testy wyłącznie na syntetycznych obrazach. **W v2.0 wyłącznie pamięć podręczna w RAM (LRU, z limitem rozmiaru), kluczowana odciskiem wersji klienta; bez kopii na dysku.** Kopia dyskowa tylko jako osobna decyzja autora, jeśli pomiar w E5 pokaże start dłuższy niż 2 s. Brak grafiki → neutralny zamiennik. Wpis w `PRIVACY.md` i `docs/PRIVACY_PL.md`: aplikacja czyta grafiki z wybranego folderu gry, lokalnie, bez sieci, i trzyma je tylko w pamięci, bez zapisu na dysk. | Redesign E5. Grafiki są treścią WG; nie redystrybuujemy ich i nie tworzymy ich trwałych kopii (EULA 5.3(a); rozstrzygnięcie 2026-11-11 pkt 6). |
| **D20** *(2026-11-11)* | Funkcja modelu ze ścieżki A nazywa się **„Co różni Twoje wygrane od przegranych”**. Zakaz słów z kontraktu team-relative §4 (m.in. impact, carry, contribution score, player value, performance score) obowiązuje także dla niej, po polsku łącznie z „wpływ”. | Model na statystykach końcowych opisuje różnice, nie przyczyny (badanie §4.2). |

### 4.1 Ustalenie N3: dlaczego wyłączono ponowne przetwarzanie

Źródła: diff `b6e0040` („add team-relative replay facts enrichment”, 2026-07-29), `docs/ARCHITECTURE.md` z tego commita oraz wewnętrzne kontrakty team-relative v1 (§23) i preflight wkładu drużyny alpha 0.2 (§25). `KNOWN_ISSUES.md` milczy.

- **Co się stało.** Odświeżanie przestarzałych wierszy podczas skanu (`stale_replay_source_keys`) zastąpiono **ograniczonym uzgadnianiem** (`reconcile_team_relative_batch`, `local_tracker.rs:782`, `:816`). Ono dopisuje tylko anonimowe fakty drużynowe i podnosi wersję wiersza.
- **Powody (realne):**
  1. **Stabilność zamrożonych analiz.** Kontrakt §23 wymaga, by ponowne przetworzenie nie zmieniało tożsamości bitwy, przynależności do sesji, wyniku, tempa bitwy, WN8, wyzwań, średnich ani trendów. Pełne odświeżenie mogło je zmienić po cichu.
  2. **Ograniczona praca.** Maks. 100 wierszy na przebieg, kursor, jedna transakcja na bitwę, wznawialność.
  3. **Bez nadpisań.** Konflikt (inny wynik niż zapisany) zostawia stary wiersz i jest raportowany.
- **Wada, którą wprowadzono:** uzgadnianie ustawia `parser_version=7, feature_schema_version=3` (`storage.rs:1491-1493`) **bez** wypełnienia `battle_type_raw`, `bonus_type_raw` i liczników trafień, więc wersja w wierszu nie mówi, czy pola są kompletne.
- **Jak v2.0 uwzględnia te powody.** Starego mechanizmu nie przywracamy. Przeliczenie v10 (D5) jest jedną jawną i świadomą zmianą danych, z kopią i raportem dla gracza:
  - **(1) analizy:** zamrożone wartości wyzwań (średnia odniesienia, SHA migawki) zostają nienaruszone jako historia; zmieniają się tylko metryki bitew, które i tak były błędne;
  - **(2) wydajność:** przeliczenie działa w tle, w paczkach po 100 bitew, z kursorem i wznawianiem po przerwaniu; każda paczka to jedna transakcja;
  - **(3) konflikty:** bitwa, której powtórka daje inną tożsamość niż wiersz v9, nie jest przenoszona i trafia do licznika „konflikty” w raporcie.
- **Zasada na przyszłość:** wersję parsera i schematu zapisujemy tylko razem z kompletem pól tej wersji. Przyszłe zmiany parsera po v10 używają tego samego wzorca: paczki, kursor, brak nadpisań przy konflikcie, jawny raport.
- **Sprzątanie w E1:** usunąć martwe `stale_replay_source_keys` i `needs_parser_refresh`. Po E2 uzgadnianie team-relative jest zbędne (v10 ma fakty dla wszystkich bitew z powtórką), więc usuwamy je w E2.

## 5. Etapy dla wykonawcy

Zasady dla każdego etapu:
- osobna gałąź `v2/eN-<nazwa>` od `main`, zakładana dopiero po scaleniu poprzedniego etapu do `main`;
- TDD (najpierw test czerwony);
- przechodzą `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`, `pnpm lint`, `pnpm test`;
- wpis w `CHANGELOG.md`;
- checkpoint dla autora i **stop do akceptacji**.

Wykonawca nie uruchamia aplikacji i nie dotyka folderu gry bez zgody autora. Test na prawdziwych danych (golden) uruchamia autor. Godziny w nawiasach są szacunkiem od zera na podstawie audytu z 2026-11-09.

### E0 — Siatka bezpieczeństwa, zależności, formatowanie (12 h)
1. **`.gitattributes`** dla `src-tauri/gen/schemas/*.json` (`text eol=lf`), osobny commit. Plik jest już przygotowany lokalnie i sprawdzony: po kompilacji drzewo jest czyste (0,25 h).
2. **Prettier** jako devDependency z `.prettierrc`, potem `prettier --write src/`. **Bez zmian logiki.** Osobny commit `style: format frontend with prettier`, diff wyłącznie z formatowaniem. `pnpm test` i `pnpm build` przed i po. Formatowanie obejmuje `App.tsx`, `api.ts`, `i18n.ts`, `AttemptsConversionUi.tsx`, `ChallengeUi.tsx`, `ReleaseUi.tsx`, `HomeOverview.tsx`, `HistoryBrowser.tsx` (1,5 h).
3. **Zależności Rust** (2,5 h):
   - podbicie `rusqlite` (`Cargo.toml:26`, dziś 0.37) do wersji, która ciągnie `libsqlite3-sys` ≥ 0.39 (sprawdzić `cargo tree -i libsqlite3-sys`), z poprawkami API;
   - `cargo update -p rustls -p quinn-proto -p rustls-webpki -p webpki-roots`;
   - `cargo audit` bez podatności.
4. **`zod`:** usunąć z `package.json` (nieimportowany) (0,25 h).
5. **Dependabot** (4 h). Każdy PR rebase na gałęzi etapu, osobny commit, testy po każdym. **Nic nie scalamy do `main` przed checkpointem E0.**
   - Łatki: #3 serde_json, #6 uuid, #2 @vitejs/plugin-react, #5 vite.
   - Wersje major: #8 zip 4.6 → 8.6 (dostosować `vehicle_catalog_probe.rs`; test syntetycznej paczki `Stored`, odrzucenie wpisu `Deflated` i za dużego wpisu), #4 sha2 0.10 → 0.11 (zgodność skrótów z testami `expected_values` i `vehicle_class_catalog`).
   - Frontend: #7 jsdom 30, #9 jest-dom 7, #10 react + @types/react. Wchodzą tylko, jeśli `pnpm test` przechodzi; inaczej odkładamy z notatką.
   - #1 json5: zamknąć bez scalania (D1).
6. **Testy charakteryzacyjne** obecnych metryk (WN8, średnia odniesienia, trendy, wyzwania) na syntetycznych bitwach oraz test `golden_local_replays` (`#[ignore]`, `AFTERBATTLE_GOLDEN_DIR`, tylko odczyt, wypisuje same agregaty: liczbę plików, rozkład `battleType` × `bonusType`, liczbę pełnych i częściowych powtórek, sumy kontrolne metryk) (3 h).
7. **Checkpoint** (0,5 h):
   - formatowanie zawiera tylko zmiany formatowania;
   - `cargo audit` bez podatności, `pnpm audit --prod` czysty;
   - kompilacja nie zmienia plików śledzonych;
   - autor uruchamia test golden i zapisuje wynik bazowy.

### E1 — Prywatność, utrata danych, mniej kodu, poprawne dane wejściowe (20,5 h) — M9, N1, N2, D1, D2, D4, D8, D16 (krok 1)
Kolejność: najpierw prywatność i utrata danych, potem usuwanie kodu, potem poprawki.
- **M9, N1, N2 (4 h).** Każda poprawka zaczyna się od testu, który na obecnym kodzie **pada**; wynik czerwony trafia do raportu etapu. Potem poprawka i testy mutacyjne (`cargo-mutants`) na zmienionych funkcjach, próg ≥ 85%.
  - **M9:** automatyczne i ręczne pobranie XVM sprawdza `xvm_enabled`. Test: XVM wyłączony → atrapa klienta HTTP nie dostaje żadnego żądania przy starcie ani przy odświeżeniu. Przełącznik `set_xvm_enabled` steruje faktycznym pobieraniem; `refresh_xvm` i `ratings::refresh` usunięte.
  - **N1:** w trybie awaryjnym tracker nie startuje, jest jedna stała ścieżka bazy awaryjnej kasowana przy następnym udanym starcie, a gracz widzi komunikat (D16, krok 1). Test: uszkodzona baza → po imporcie syntetycznej powtórki nic nie trafia do bazy awaryjnej, a w `%TEMP%` nie przybywa plików przy kolejnych startach.
  - **N2:** `delete_all_data` w trybie awaryjnym zwraca błąd z kodem i tekstem PL/EN zamiast sukcesu. Test: tryb awaryjny → komenda zwraca błąd, a prawdziwy plik bazy jest nienaruszony.
- Usunięcie `python.log` (D1): ścieżka jest dziś spleciona z deltami i `enrich_battle`, więc trzeba usunąć też gałęzie `ambiguous` po stronie zapisu, testy tej ścieżki i martwy kod z §4.1 (4 h).
- Usunięcie Tomato, profilu i WG oraz martwych komend (D2, A7); czyszczenie Credential Managera przy starcie; aktualizacja `PRIVACY.md`, `SECURITY.md`, `THREAT_MODEL.md` i README (4 h).
- `PRIVACY.md` po usunięciu `python.log`, Tomato i WG zgodny z kodem; `docs/PRIVACY_PL.md` przepisany jako wierne tłumaczenie `PRIVACY.md` (zastępuje tekst z prywatnej alphy) (0,5 h).
- Predykat `stats_eligibility` (D4) na wejściu i we wszystkich modułach: `ratings`, `baseline`, `performance_trends`, `session_insights`, `challenges`, zapytania w `storage.rs`. `REPLAY_PARSER_VERSION` = 8 (3 h).
- Asysta (M4), XP do średnich (M11), `.earliest()` dla DST (D10) (1 h).
- Testy (3,5 h):
  - tabela `battleType` × `bonusType` (1/1, 1/≠1, 2, 22, 29, 101–109, 27, 43, brak, napis) przy `gameplayID = "ctf"`;
  - bitwa nielosowa nie zmienia żadnej metryki;
  - skład asysty;
  - test regresji dla M3;
  - kasowanie wpisów sejfu (także gdy ich brak);
  - testy mutacyjne `stats_eligibility` i `parse_battle_results` (≥ 85%).
- **Checkpoint** (0,5 h):
  - raport zawiera wyniki testów M9, N1 i N2 na kodzie sprzed poprawki (czerwone) i po niej (zielone) oraz wynik mutacji;
  - w kodzie nie ma `tomato`, `application_id`, `sessionStats`, `json5`, `python.log`;
  - w repo nie ma prawdziwych identyfikatorów graczy (sprawdzone tymi samymi wzorcami co w audycie, bez zapisywania wartości w repo);
  - raport podaje liczbę usuniętych linii.

### E1b — Tier i role w katalogu pojazdów (5 h, zakres 4–6 h) — badanie §2, B5
Dlaczego przed E2: kolumny tierów w v10 (`battles.vehicle_tier`, `battle_team_stats.tier_*`, `battle_roster.vehicle_tier`) wypełnia import z E2 w chwili przetwarzania powtórki. Bez tieru w katalogu byłyby puste i potrzebny byłby drugi przebieg po wszystkich powtórkach.

- **Pole `level`** z `scripts/item_defs/vehicles/<nacja>/list.xml` (liczba, w 1 270/1 270 rekordów klienta 2.4.1.1) w `vehicle_class_catalog`; zakres 1–11 (istnieje poziom XI). Wartość spoza zakresu = brak tieru dla rekordu, nie błąd całości (1,5 h).
- **Tagi `role_*`** z pola `tags` jako rola pojazdu (15 wartości w kliencie 2.4.1.1); nieznany tag `role_*` zapisany surowo, bez błędu (0,5 h).
- **Schemat pliku katalogu 1 → 2** (`vehicle_class_catalog.rs:29`). Katalog w schemacie 1 jest traktowany jako nieaktualny i budowany od nowa; bezpieczny powrót do poprzedniego katalogu działa jak dziś (0,5 h).
- **`id` jako napis** (9 rekordów, dziś liczone jako `missing_id`, `vehicle_catalog_probe.rs:253-255`): napis z samą liczbą całkowitą przyjmowany, inny odrzucany z licznikiem (0,5 h).
- **Odcisk katalogu** udostępniony importowi (E2 zapisuje go przy bitwie jako `vehicle_catalog_fingerprint`) (0,25 h).
- **Odporność na nowe nacje (B5):** brak jednej z 11 znanych nacji ani nowa nacja nie odrzucają całego katalogu; nowa nacja trafia do logu i diagnostyki. Status „katalog nieaktualny dla wersji gry X” w diagnostyce (0,75 h).
- **Testy (1 h):** syntetyczny `list.xml` (tier, role, `id` jako napis, poziom XI, brak `level`, wartość poza zakresem); testy obcinania i mutacji jak dla katalogu klas; migracja pliku katalogu 1 → 2. Zasady bez zmian: limity przed alokacją, `checked_*`, tylko wpisy `Stored`.
- **Poza zakresem:** „premium” (brak jednoznacznego znacznika, badanie §2 uwaga 1; decyzja autora: nie teraz), użycie tieru w średnich odniesienia i wyzwaniach.
- **Checkpoint:** autor uruchamia test golden; **odsetek pojazdów z powtórek autora z tierem ≥ 99%**; raport podaje rozkład tierów i liczbę nierozpoznanych pojazdów (same liczby).

### E2 — Baza v10, migracje, odtworzenie, brak brakujących bitew, podwaliny danych (27 h) — D5, D16 (krok 2), D6 (połączenie), D14 (import), D17
- Framework `MIGRATIONS` sterowany `user_version`; odmowa otwarcia bazy z nowszym schematem i czytelny komunikat; kopia `VACUUM INTO afterbattle.v{n}.bak` przed każdym krokiem; jedna transakcja na krok; `integrity_check` raz przy starcie (N5) (3 h).
- **Silnik pełnego importu** w wątku w tle (bez async komend i eventów, to E3): bez limitów 90 dni i 500 plików, paczki po 100 bitew z kursorem i wznawianiem (§4.1), postęp czytany przez istniejący `get_snapshot`. Ten sam silnik **doczytuje przy starcie** pliki nieobecne w `replay_files` albo zmienione (`size`, `mtime`) (4 h).
- Jednorazowe przejście v9 → v10 według D5 (5 h):
  - kopia;
  - **bramka P8** (liczba i odsetek bitew bez pliku powtórki; od 2026-11-13 tylko pełne bitwy, §9 P8);
  - przeniesienie `settings`/`sessions`/`challenges`/`challenge_battles`;
  - usunięcie martwych kolumn WN8 i uzgadniania team-relative;
  - przeliczenie silnikiem importu;
  - raport liczb dla gracza.
- Odtworzenie według D16, krok 2 (1,5 h).
- Jedno połączenie za `Mutex` (D4 z audytu) (2 h).
- **Podwaliny danych v10** (badanie §8; 7 h). Zasady:
  - wszystkie nowe kolumny `NULL`-owalne, bez wartości domyślnej 0 (kontrakt team-relative §18);
  - `REPLAY_PARSER_VERSION` podnoszona do 9 razem z kompletem nowych pól, nigdy częściowo (§4.1);
  - zero identyfikatorów: nicki, `accountDBID`, klany, surowy `prebattleID`, identyfikatory pojazdów w bitwie (klucze słownika `vehicles`, `killerID`), także w postaci hasza; `arena_unique_id` tylko tam, gdzie jest dziś;
  - nowe tabele dostają wiersze tylko dla bitew spełniających warunki team-relative §6 (pełna, losowa, 15 na 15); bitwa niekwalifikująca się nie ma wierszy;
  - tabele `STRICT` z `CHECK` i `ON DELETE CASCADE` z `battles`.

  Zakres:
  - **`battles`, kolumny badania §8.1 A** (1,5 h): `vehicle_tier`, `vehicle_catalog_fingerprint` (z E1b), `life_time_seconds`, `death_reason_raw`, `max_health`, `health_remaining` (bez `CHECK ≥ 0`), `direct_hits_received`, `piercings_received`, `no_damage_hits_received`, `explosion_hits_received`, `potential_damage_received`, `damage_from_invisibles`, `assist_smoke`, `assist_inspire`, `stun_duration`, `stun_num`, `capture_points`, `mileage` (surowo), `arena_type_id`, `own_platoon_size` (1–3).
  - **`battle_team_stats`, badanie §8.1 B** (1,5 h): 2 wiersze na bitwę, klucz (`battle_id`, `side`), `side` ∈ {`own`, `enemy`}; sumy, `survivors`, `life_time_total`, `tier_min`/`tier_max`/`tier_sum`, liczniki klas, `unresolved_vehicles`, `platoons`, `platooned_players`, `fact_schema_version`.
  - **`battle_roster`, badanie §8.2 D bez `marks_on_gun`** (D17; 3 h): 30 wierszy na bitwę, klucz (`battle_id`, `slot`); `side`, `is_own_player`, `type_comp_descr`, `vehicle_tier`, `vehicle_class`, `platoon_group` (0 = solo, potem 1, 2, … w obrębie bitwy), statystyki z badania §8.2 D, `killer_slot` (`NULL`, gdy pojazd przeżył albo zabójcy nie da się rozwiązać).
  - **`replay_files.packet_stream_bytes`, badanie §8.2 F** (0,5 h): długość części po blokach JSON, liczona z rozmiaru pliku i długości bloków, bez czytania strumienia.
  - **Eksport i usuwanie** (0,5 h): eksport sesji nie zawiera nic z `battle_roster` ani `battle_team_stats`; `delete_all_data` czyści obie tabele.
  - **`PRIVACY.md` i `docs/PRIVACY_PL.md`** (D17): jedno zdanie o lokalnym, anonimowym składzie bitew gracza (pojazd i statystyki uczestników, bez nicków, ID kont i klanów, wykluczony z eksportu).
- Testy (4 h):
  - pusta baza;
  - v9 z danymi gracza (zachowane wyzwania i sesje, kopia istnieje);
  - bramka P8: 6% bitew bez pliku → przeliczenie się nie zaczyna;
  - (2026-11-13, §9 P8) bramka ignoruje wiersze częściowe; 0% pełnych bez powtórki → przebudowa startuje; > 5% pełnych → stop; pusta sesja nie przechodzi do v10;
  - przerwane przeliczenie wznawia się od kursora bez duplikatów;
  - v10 → v10 idempotentnie;
  - nowszy schemat;
  - uszkodzony plik (start się udaje, jest `.corrupt-*`, nowe bitwy trafiają do odtworzonej bazy);
  - „aplikacja wyłączona → 3 nowe pliki → start → 3 bitwy”;
  - baza tylko do odczytu;
  - **brak identyfikatorów w nowych tabelach:** test schematu (żadna kolumna `battle_roster`, `battle_team_stats` ani nowa kolumna `battles` nie odpowiada polu tożsamości z listy zakazanej) i test wartości (syntetyczna powtórka z rozpoznawalnymi nickami, `accountDBID`, `clanDBID`, `prebattleID` i identyfikatorami pojazdów w bitwie → żadna z tych wartości nie występuje w żadnej komórce bazy);
  - **deterministyczna numeracja slotów:** ta sama bitwa z permutowaną kolejnością kluczy `vehicles` i `players` daje identyczne wiersze `battle_roster`; remisy obrażeń rozstrzygane według kolejnych pól, bez zależności od identyfikatorów;
  - **`killer_slot`:** wskazuje slot zabójcy z tego samego składu; `NULL` dla żywych i dla `killerID` spoza składu;
  - **`platoon_group`:** dwa plutony i gracze solo → grupy 1, 2 i 0; surowy `prebattleID` nie występuje w bazie; `own_platoon_size` zgodny ze składem;
  - bitwa częściowa, nielosowa albo nie 15 na 15 → brak wierszy w nowych tabelach, kolumny `battles` z §8.1 A wypełnione tam, gdzie pole istnieje;
  - `battle_team_stats` zgodne z sumami policzonymi z `battle_roster` dla tej samej bitwy;
  - eksport sesji bez danych składu;
  - `packet_stream_bytes` = rozmiar pliku − nagłówek − bloki JSON dla syntetycznej powtórki z dopisanym ogonem.
- **Checkpoint** (0,5 h):
  - powyższe testy i opis ręcznej procedury odtworzenia z `.bak`;
  - test golden u autora podaje (same liczby): odsetek bitew z wierszami w `battle_roster`, odsetek pojazdów składu z tierem, sumę `packet_stream_bytes`;
  - **autor uruchamia przeliczenie na kopii swojej bazy; wykonawca podaje liczbę bitew bez pliku powtórki i ich odsetek historii;**
  - **jeśli > 5%: stop i decyzja autora przed przeliczeniem prawdziwej bazy.**

### Przystanek „wersja 2.0-min” (po E2, 1,5 h)
Punkt, w którym autor **może zakończyć v2.0**. Spełnione są:
- statystyki liczą tylko bitwy losowe z poprawną asystą;
- brak połączeń z XVM bez zgody;
- brak cichej utraty bitew (tryb awaryjny, doczytanie po starcie, import bez limitów);
- migracje z kopią;
- brak znanych podatności;
- mniej kodu (bez `python.log`, Tomato, WG);
- tier w katalogu pojazdów (E1b) i podwaliny danych pod „Wizję po v2.0” w bazie v10 (E2).

Czego jeszcze brakuje:
- okno może przycinać się podczas pełnego importu, bo komendy są jeszcze synchroniczne (E3);
- komunikaty błędów są po polsku tylko częściowo, interfejs ma stary wygląd (E5);
- brak CI i skryptu wydania (E8).

Przystanek obejmuje:
- wpis 2.0-min w `CHANGELOG.md`;
- numer wersji ustalony z autorem w checkpoincie (musi przejść build instalatora NSIS/MSI);
- build lokalny wykonuje autor;
- test ręczny §6.4, punkty 1–3, 5–8, 11, 14, 15.

Jeśli autor kończy tutaj, E3–E8 przechodzą do v2.1.

### E3 — Wątki, eventy, wydajność importu (12,5 h) — D6, D14
- Komendy `async` + `spawn_blocking` (2,5 h).
- Event `import-progress {done,total}`, anulowanie (2 h).
- Hash tylko z bloków JSON zamiast całego pliku (1,5 h).
- Jeden harmonogram auto-końca sesji; lekki `snapshot` (bez WN8 dla 30 sesji, bez odczytów sejfu) (1,5 h).
- Eventy `data-changed` i `session-ended` zamiast odpytywania (2 h).
- Wstrzykiwany zegar zamiast `sleep` w testach (T8) (1,5 h).
- **Checkpoint** (1,5 h): test importu 2000 syntetycznych plików (postęp przez event, anulowanie, brak duplikatów, brak sesji dla starych bitew); okno reaguje podczas importu (test komendy async); autor mierzy czas pełnego importu u siebie.

### E4 — Analiza i WN8 (9,5 h) — D3, D7
- **Przed kodem (P4):** sprawdzić, czy `https://static.modxvm.com/wn8-data-exp/json/wg/wn8exp.json` (`expected_values.rs:16-17`) nadal działa i ma format `header.source == "XVM"`, `header.version` jako data, `data[]` z `IDNum, expDef, expFrag, expSpot, expDamage, expWinRate`. Zapisać w raporcie liczbę wierszy i wersję. Jeśli adres lub format się zmieniły: stop i decyzja autora (0,5 h).
- Znacznik próby i opóźnienia ponowień według D3 (M9 jest już zrobione w E1) (0,5 h).
- Tolerancja złych wierszy XVM z limitem (M8) (1 h).
- Kwalifikacja per metryka w średnich odniesienia i wnioskach (M5, D7); czyszczenie sygnałów przy za małej bazie (M10); kontrakt konwersji (M12) (2,5 h).
- Wersja danych XVM w ustawieniach i stopce (D3); test WN8 zgodny z niezależnym kalkulatorem dla 3 przypadków (±0,5 pkt) (1,5 h).
- Średnie odniesienia jako agregaty SQL zamiast `SELECT` 61 kolumn na bitwę (P4) (2 h).
- **Checkpoint** (1,5 h): mutacje w `wn8`, `stats_eligibility`, średniej odniesienia, `session_insights` i generatorze wyzwań wykryte w ≥ 85%; raport sprawdzenia XVM. **Bramka E7.**

### E5 — Redesign UI: struktura, komunikaty, dostępność, widok czołgu (49,5 h) — D9, D10, D17, D19
Wzorzec: zaakceptowana makieta https://claude.ai/artifact/Wsv91yfaKq3Eh4u6bEUJAL. Dotychczasowy zakres E5 zostaje i jest realizowany od razu na nowym wyglądzie, a nie na starym. Od 2026-11-11 E5 obejmuje też dawny E6 (krok 3).

**Krok 1. Grafiki z instalacji gry (10,5 h, na początku etapu; D19).**
- **Rozpoznanie (1,5 h), tylko odczyt, za zgodą autora na odczyt folderu gry**, wynik wyłącznie w liczbach zbiorczych i nazwach katalogów:
  - gdzie w `res/packages` leżą ikony pojazdów (małe konturowe i duże) oraz grafiki map (miniatury, tła);
  - format (PNG/DDS), rozmiary, metoda zapisu w paczce (`Stored`/`Deflated`), rozmiar paczek i współczynniki kompresji;
  - jak nazwy plików mapują się na `typeCompDescr` (nacja + nazwa pojazdu z katalogu) i na `arenaTypeID`.
- **Wpisy `Deflated` w czytniku paczek (2 h), z góry dozwolone** pod warunkami:
  - limit rozmiaru po rozpakowaniu i limit współczynnika kompresji sprawdzane przed alokacją (z nagłówka wpisu), a w trakcie rozpakowania twarde przerwanie po przekroczeniu limitu;
  - odczyt strumieniowy, bez wczytywania całej paczki do pamięci;
  - testy bomby dekompresji (fałszywy rozmiar w nagłówku, skrajny współczynnik) i obcinania na każdej długości.
  - **Stop i raport dla autora tylko wtedy**, gdy tych warunków nie da się spełnić (np. format nie podaje rozmiarów przed rozpakowaniem).
- **Moduł (4 h):** odczyt w locie czytnikiem paczek (`vehicle_catalog_probe.rs`: limity przed alokacją, wyszukiwanie po stałej nazwie, nic nie jest wypakowywane na dysk); konwersja do PNG w pamięci, jeśli źródłem jest DDS; mapowanie pojazd → ikona i mapa → grafika.
- **Pamięć podręczna i zamiennik (1 h):** wyłącznie w RAM, LRU z limitem liczby i rozmiaru, kluczowana odciskiem wersji klienta (nowa wersja klienta = pusta pamięć); **bez kopii na dysku**; neutralny zamiennik (klasa pojazdu, nazwa mapy), gdy grafiki brak.
- **Pomiar startu (0,5 h):** czas od startu do pierwszego ekranu z grafikami, w raporcie etapu. Jeśli > 2 s: stop i osobna decyzja autora o kopii dyskowej (D19).
- **Testy (1,5 h):** wyłącznie syntetyczne obrazy i syntetyczna paczka; obcinanie i mutacje jak dla katalogu; brak grafiki → zamiennik; zmiana odcisku wersji → unieważnienie pamięci; wypieranie LRU przy limicie; żaden plik grafiki nie powstaje w katalogu danych aplikacji.
- `PRIVACY.md` i `docs/PRIVACY_PL.md`: aplikacja czyta grafiki z wybranego folderu gry, lokalnie, bez sieci, i trzyma je tylko w pamięci.

**Krok 2. System wizualny (3 h).**
- Jeden system tokenów (kolory, typografia, odstępy) zamiast `styles.css` + `alpha.css`; jeden akcent (pomarańcz), fiolet usunięty.
- Zasady: bez pigułek, wersalików, nadtytułów i kropek w tytułach; kolor nigdy nie jest jedynym nośnikiem informacji (zawsze też tekst, znak lub kształt); poświata tylko na jednym kluczowym elemencie ekranu.

**Krok 3. Nawigacja, ekrany i widok czołgu (19 h).**
- Nawigacja: Sesja, Czołgi, Cele, Historia, Mapy, Ustawienia (z prywatnością w środku). **Mapy:** w buildzie prywatnym wyszarzone do czasu ścieżki B, **w publicznym buildzie ukryte** (flaga buildu, test obu wariantów) (2,5 h).
- Cztery ekrany według makiety: **Sesja** (3 h), **Czołg** (3 h), **Bitwa** (4,5 h), **Cele** (2 h); Historia i Ustawienia przeniesione na nowy system (1 h).
- **Ekran Czołg w v2.0** porównuje ze średnią gracza na tym czołgu. Porównanie z innymi graczami (tabele odniesienia, §11.1) dochodzi po v2.0 bez zmiany układu.
- **Ekran Bitwa:** porównanie obu drużyn z sum `battle_team_stats` (D17), wyłącznie agregaty, **nigdy lista graczy składu**; odczyt przez nowe zapytanie i DTO bez pól `battle_roster` (w godzinach ekranu Bitwa). Podsumowanie z odnośnikami do statystyk jest poza v2.0 (§11.3); miejsce w układzie zostaje.
- **Widok czołgu i wyzwania: konsolidacja (dawny E6, 3 h; 1 h mniej, bo układ i stany puste powstają razem z ekranem Czołg):** widok czołgu, trendy i wyzwania na predykacie D4 i definicjach D8; stronicowanie bitew po 50; wykres maks. 200 punktów; stany puste („za mało bitew na tym czołgu”).

**Krok 4. Dotychczasowy zakres na nowym wyglądzie (17 h).**
- Rozbicie sformatowanych plików na moduły (4 h; 3 h mniej niż w planie z 2026-11-09, bo ekrany z kroku 3 powstają od razu w nowej strukturze); ESLint jako bramka (1 h).
- Słownik błędów PL/EN dla ok. 31 kodów i test kompletności (2,5 h).
- Teksty: „Procent wygranych”, „powtórka”, limit bezczynności z ustawień, „USUŃ”/„DELETE” zależnie od języka (U4, U5, U8) (1,5 h).
- Okna dialogowe z Esc i fokusem (U7) (1,5 h).
- Obsługa błędów zamiast `.catch(()=>{})` (Q2, Q3); jedno źródło języka (N8) (1 h).
- `session-ended` z backendu zamiast `useRef` (A6) (1 h).
- Typy `ts-rs` i test kontraktu `generate_handler` ↔ `api.ts` (Q10, T3) (2,5 h).
- Testy dla `GoalsPage`, `HomeOverview`, `BattlePaceUi` (T4) (2 h).
- **Checkpoint:**
  - żaden widoczny tekst nie pasuje do `[a-z]+_[a-z_]+`; sprawdzenie klawiaturą opisane w raporcie;
  - w repo, fixture'ach i konfiguracji instalatora nie ma grafik WG (sprawdzenie typów i rozmiarów plików graficznych w drzewie);
  - w kodzie frontendu nie ma `alpha.css` ani kolorów spoza tokenów;
  - zrzuty ekranów do raportu wyłącznie z zamiennikami grafik;
  - w katalogu danych aplikacji nie ma plików grafik; raport podaje pomiar startu;
  - DTO i UI ekranu Bitwa nie zawierają wierszy składu (test);
  - (dawny E6) agregaty widoku czołgu zgodne z `metrics()` na tych samych danych; test wykresu (0, 1 i 200 punktów).

### E6 — włączony do E5 (2026-11-11)
Zakres dawnego E6 (widok czołgu i wyzwania) jest w E5, krok 3. Numeracji pozostałych etapów nie zmieniamy, żeby nie przesuwać odwołań (bramka E7, D12, gałęzie `v2/eN-*`).

### E7 — Raport tygodniowy (10 h, warunkowo)
- Zakładka „Tydzień” zbudowana **wyłącznie** na `performance_trends` i `challenges`: wskaźnik tygodnia dla obrażeń + asysty, przetrwania, XP i WN8 względem własnej średniej na czołgu, 4 poprzednie tygodnie, oznaczenie pewności, podsumowanie wyzwań tygodnia. WR tylko jako kontekst (D12). Bez nowych typów celów.
- **Bramka (po E4):** E0–E4 zamknięte bez regresji w teście golden; godziny E0–E4 nie przekroczyły szacunku o więcej niż 30%; autor potwierdza zakres.
- **Checkpoint:** testy na syntetycznych tygodniach (za mało bitew, n < 15 → „za mało bitew na trend”, granice tygodnia ISO w czasie lokalnym, WR nigdy nie jest celem).

### E8 — Release i CI (8,5 h) — D11, D13, P5
- README: instrukcja dla instalujących samodzielnie (ostrzeżenie SmartScreen, weryfikacja `Get-FileHash -Algorithm SHA256` z `SHA256SUMS.txt`) (0,5 h). Do tego czasu instaluje autor.
- Wersja 2.0.0 w jednym źródle i test zgodności 4 miejsc (1,5 h).
- `CHANGELOG.md` (1 h), `scripts/release.ps1` z SHA-256 (1 h).
- CI według D13 (2,5 h).
- Dokumenty zgodne z kodem: `PRIVACY.md` (XVM opt-in), `KNOWN_ISSUES.md`, `SECURITY.md`, `THREAT_MODEL.md` z nową datą przeglądu (N7, R7) (1,5 h).
- Przeniesienie ręcznego narzędzia czyszczenia bazy z alfy poza zestaw testów (N9) (0,5 h).
- **Checkpoint:** CI zielone na `main`; artefakt zgodny z `SHA256SUMS.txt` (`Get-FileHash`); autor wykonuje test ręczny z §6.4.

## 6. Testy

### 6.1 Jednostkowe
- **Wejście:** tabela `battleType` × `bonusType`, każde pole wyników (brak / poza zakresem / napis zamiast liczby), remis, wiele pojazdów, DST, limity rozmiarów.
- **Kwalifikacja:** `stats_eligibility` (D4) w każdym module; bitwa nielosowa nie zmienia żadnej metryki.
- **Metryki:** asysta (radio + gąsienice), WR z remisami, przetrwanie, `original_xp`, `NULL` w agregatach.
- **WN8:** 3 przypadki referencyjne z niezależnego kalkulatora, wersja migawki w odpowiedzi, brak żądań sieciowych przy wyłączonym XVM.
- **Średnia odniesienia i wnioski:** kwalifikacja per metryka, tylko wcześniejsze bitwy, brak sygnałów przy za małej bazie.
- **i18n:** kompletność PL/EN dla każdego `AppError` i statusu.
- **Katalog (E1b):** tier 1–11, role, `id` jako napis, brak lub zły `level`, nowa nacja nie psuje całości.
- **Podwaliny v10 (E2):** brak identyfikatorów w nowych tabelach (schemat i wartości), deterministyczne sloty, `killer_slot`, `platoon_group`, brak wierszy dla bitew niekwalifikujących się.
- **Grafiki (E5):** syntetyczne obrazy i paczka, zamiennik, unieważnienie pamięci RAM po zmianie wersji klienta, wypieranie LRU, brak plików grafik na dysku; wpisy `Deflated`: bomba dekompresji (fałszywy rozmiar, skrajny współczynnik) i obcinanie.
- **Ekran Bitwa (E5):** DTO porównania drużyn nie ma pól ani wierszy z `battle_roster`.

### 6.2 Integracyjne
- Migracje (E2): pusta baza, v9 → v10 z danymi gracza, idempotencja, nowszy schemat, uszkodzony plik, tylko do odczytu.
- Import 2000 syntetycznych plików z postępem i anulowaniem; doczytanie po starcie; plik dopisany z 1 do 2 bloków podnosi bitwę do pełnej bez nowej sesji.
- Kontrakt IPC zgodny z typami z `ts-rs`.
- `delete_all_data` w trybie zwykłym i awaryjnym: brak wierszy (także w `battle_roster` i `battle_team_stats`), brak wpisów w sejfie, brak plików `.bak`/`.corrupt-*` po potwierdzeniu; pamięć grafik w RAM wyczyszczona.
- Eksport sesji nie zawiera danych z `battle_roster` ani `battle_team_stats`.
- Test golden u autora: liczba bitew w statystykach = liczba powtórek z `battleType == 1 && bonusType == 1` i pełnymi wynikami; odsetek pojazdów z tierem ≥ 99% (E1b).

### 6.3 Mutacyjne
`cargo-mutants` dla wskazanych funkcji, próg ≥ 85%, każdy przeżyły mutant ma test albo uzasadnienie. **Od 2026-11-14 (decyzja autora):** mutacje ≥ 85% obowiązują dla logiki nowej lub zmienionej w etapie; starsze moduły są w wewnętrznym backlogu mutacji autora i nie blokują wydania. Funkcje:
- (E1) bramka zgody XVM przed pobraniem (M9), start trackera w trybie awaryjnym (N1), `delete_all_data` w trybie awaryjnym (N2);
- `stats_eligibility`;
- `parse_battle_results`;
- `wn8`;
- średnia odniesienia;
- `session_insights`;
- generator wyzwań;
- kroki migracji;
- auto-koniec sesji;
- (E5) limity rozpakowania wpisów `Deflated` w czytniku paczek;
- (E7) wskaźnik tygodnia.

### 6.4 Test ręczny autora
1. Instalacja 2.0.0 na bazie alpha. Sprawdź, że powstał `afterbattle.v9.bak`. Import w tle ma pasek postępu, a okno reaguje.
2. Po imporcie raport „przeliczono / pominięto nielosowe / bez pliku” zgadza się z testem golden. Aktywne wyzwanie i historia wyzwań są zachowane.
3. Porównaj 3 losowe bitwy z ekranem wyników w grze (obrażenia, asysta radio + gąsienice, XP, wynik).
4. Przy wyłączonym XVM aplikacja nie łączy się z siecią (zapora lub monitor sieci). Po włączeniu pojawia się WN8 z datą wartości oczekiwanych; porównaj WN8 jednej sesji z zewnętrznym kalkulatorem (±2%).
5. Zagraj 2–3 bitwy losowe przy aplikacji w zasobniku: sesja startuje sama, liczby aktualizują się bez klikania.
6. Zamknij aplikację, zagraj 2 bitwy, uruchom ponownie: obie bitwy pojawiają się same.
7. Zagraj 1 bitwę szkoleniową lub w innym trybie: nie pojawia się w statystykach.
8. Zostaw sesję bez bitew na czas limitu: pojawia się raport i komunikat po polsku.
9. Przełącz PL ↔ EN i przejrzyj ekrany: bez kodów, bez angielskich wtrąceń w PL.
10. Przejdź aplikację samą klawiaturą (Tab, Esc).
11. Skopiuj `afterbattle.sqlite3`, nadpisz oryginał śmieciami i uruchom: komunikat o odtworzeniu, plik `.corrupt-*`, nowe bitwy trafiają do odtworzonej bazy (sprawdź po restarcie).
12. Widok czołgu dla najczęściej granego czołgu wygląda sensownie, a czołg z 1 bitwą ma czytelny stan pusty.
13. (Jeśli E7) zakładka „Tydzień”: przy małej liczbie bitew pojawia się „za mało bitew”.
14. Eksport sesji nie zawiera nicka, ID, ścieżek, dokładnych godzin ani składu bitew.
15. „Usuń wszystkie dane”: Credential Manager bez wpisów `app.afterbattle.*`, brak `.bak` i `.corrupt-*`, aplikacja wraca do konfiguracji.
16. `Get-FileHash -Algorithm SHA256` instalatora zgodny z `SHA256SUMS.txt`.
17. Diagnostyka pokazuje wersję katalogu pojazdów i pokrycie tierów; po aktualizacji gry status „katalog nieaktualny” pojawia się albo katalog odświeża się sam.
18. Ekrany Sesja, Czołg, Bitwa i Cele pokazują ikony pojazdów i grafiki map z gry; po zmianie folderu gry na pusty pojawiają się zamienniki, a aplikacja działa dalej. Start do pierwszego ekranu z grafikami trwa ≤ 2 s.
19. W katalogu danych aplikacji nie ma plików grafik.
20. Ekran Bitwa pokazuje porównanie obu drużyn, bez listy graczy. W buildzie publicznym nie ma pozycji „Mapy”.

## 7. Komendy IPC (v2.0)

**Zostają (z ewentualną zmianą na async):**
- `get_snapshot`, `get_expected_values_status`, `get_vehicle_class_catalog_status`, `refresh_vehicle_class_catalog`, `refresh_expected_values`, `set_xvm_enabled` (podłączona do D3);
- `detect_wot_installations`, `configure_local_source`, `pause_tracker`, `resume_tracker`;
- `start_session`, `end_session`, `get_session_summary`;
- `get_attempts_conversion_session_report`, `get_attempts_conversion_recent_report`, `get_battle_history_page`, `get_vehicle_history_overview`, `get_vehicle_history_report`;
- `get_player_baseline`, `get_vehicle_baseline`, `get_session_baseline_comparison`, `get_performance_trends`;
- `get_challenge_suggestions`, `start_vehicle_challenge`, `get_active_challenge`, `get_challenge_history`, `abandon_active_challenge`;
- `update_preferences`, `complete_onboarding`, `reset_onboarding`, `get_diagnostics`, `export_anonymous_session`, `delete_all_data`.

**Nowe:** `start_import`, `cancel_import` (zastępują `import_local_history`); `get_recent_sessions_wn8` (zmiana 2026-11-12, zaakceptowana przez autora: WN8 ostatnich 30 sesji przeniesione z `get_snapshot` do osobnej komendy, żeby snapshot był lekki zgodnie z E3; bez zmiany danych); (E5) odczyt grafiki pojazdu i mapy z pamięci w RAM — **`get_game_image(kind, key)`**, komenda zwracająca bajty PNG albo pustą odpowiedź (ustalone w E5, spec 25 §6; CSP `img-src` dostaje `blob:`, bez uprawnień sieciowych); (E5) porównanie drużyn dla bitwy z `battle_team_stats` — **`get_battle_view(battleId)`** (ustalone w E5, spec 25 §5.3: `BattleView` bitwy i pole `teams` z sumami obu drużyn; DTO bez danych `battle_roster`); `get_battle_history_page` dostaje opcjonalny filtr `mapKey` (spec 25 §5.5); (E7) `get_weekly_report`. E2 nie dodaje komend; `battle_roster` nie ma odczytu w v2.0.

**Usuwane:** `import_local_history`, `import_history`, `sync_battles`, `local_source_status`, `refresh_xvm`, `store_wg_application_id`, `remove_wg_application_id`, `configure_profile`, `store_api_key`, `remove_api_key`, `test_api_connection`.

**Eventy:** `import-progress`, `data-changed`, `session-ended`.

## 8. Ryzyka

| Ryzyko | Prawdopodobieństwo / wpływ | Mitygacja |
|---|---|---|
| Aktualizacja WoT zmieni blok wyników | średnie / wysoki | `parser_version`, nieznane pola `NULL`, test golden po patchu |
| Patch zmieni paczki klienta (nacje, format), katalog klas się zestarzeje (B5) | średnie / średni | Bezpieczny powrót do poprzedniego katalogu jest już w kodzie; status w diagnostyce |
| Przeliczenie v9 → v10 zgubi dane gracza | niskie / wysoki | `.bak` przed krokiem, test migracji z wyzwaniami i sesjami, punkt 2 testu ręcznego |
| Powtórki usunięte przez gracza → bitwy znikną ze statystyk po v10 | średnie / średni | Raport „K bez pliku powtórki” przed zatwierdzeniem; dane zostają w `.bak` |
| XVM zmieni adres lub format | średnie / średni | Sprawdzenie przed E4, aplikacja działa bez WN8 |
| Major `zip`/`sha2`/`jsdom` łamie API | średnie / niski | Osobne commity w E0, możliwość odłożenia frontendowych PR |
| Rozbijanie frontendu zmieni zachowanie | średnie / średni | Formatowanie oddzielone od refaktoru, testy komponentów przed i po |
| Przekroczenie budżetu przesunie E7 | średnie / niski | Bramka po E4 |
| Brak podpisu → SmartScreen u znajomych | wysokie / niski | Instrukcja weryfikacji SHA-256 (P5) |
| Patch przenosi pojazd między tierami albo zmienia `list.xml` | średnie / niski | Tier zapisany przy bitwie z odciskiem katalogu (E2); status „katalog nieaktualny” (E1b) |
| Skład bitwy (D17) da się połączyć z nickiem przez zewnętrzny serwis z tą samą bitwą | niskie / średni | Dane tylko lokalnie, bez `marks_on_gun`, wykluczone z eksportu, w UI tylko agregaty drużyn; zdanie w `PRIVACY.md` |
| Grafiki w paczkach są `Deflated` (ryzyko bomby dekompresji) albo w DDS | średnie / średni | Obsługa `Deflated` z limitem rozmiaru po rozpakowaniu i współczynnika przed alokacją, odczyt strumieniowy, testy bomby i obcinania, mutacje; stop tylko, gdy warunków nie da się spełnić; zamienniki zawsze działają |
| Dekodowanie grafik bez kopii dyskowej wydłuża start | średnie / niski | Pamięć RAM (LRU), pomiar w E5; > 2 s → osobna decyzja autora o kopii dyskowej (D19) |
| Pamięć RAM grafik rośnie bez kontroli | niskie / niski | LRU z limitem liczby i rozmiaru, test wypierania |
| Redesign przekracza budżet E5 | średnie / średni | Kroki 1–4 rozliczane osobno w raporcie; bramka E7 po E4 bez zmian |

## 9. Pytania do autora

### Rozstrzygnięte
- **P1 — baza kodu:** `main` @ `8ba16b0` (2026-11-09). Gałąź `feature/replay-analytics-alpha-0.2` zawiera wyłącznie README; odłożone.
- **P2 — oceniona od nowa (2026-11-09):** jednorazowe przeliczenie v9 → v10 z `.bak` i przeniesieniem danych gracza; od v10 migracje przyrostowe z `.bak` (D5). Uzasadnienie: audyt §6.
- **P3 — tak:** WR tylko w raporcie (D12).
- **P4 — XVM zostaje:** sprawdzenie adresu i formatu przed kodem w E4.
- **P6 — nie przepisujemy historii:** publiczne repo od czystego commita (D15).

- **P5 — na razie instaluje autor (2026-11-09).** Instrukcja SmartScreen i weryfikacji SHA-256 w README w ramach E8.
- **P7 — WN8 liczone przy odczycie (2026-11-09).** Wersja danych XVM pokazana raz (ustawienia i stopka). Jawna zmiana D3.
- **P8 — bitwy bez pliku powtórki zostają tylko w `.bak` (2026-11-09).** Warunek: checkpoint E2 podaje ich liczbę i odsetek historii; przy > 5% stop przed przeliczeniem i decyzja autora (D5).
- **P8 — bramka liczy wyłącznie pełne bitwy (rozstrzygnięte przez autora 2026-11-13).** Na kopii bazy autora bramka zatrzymała przebudowę: 628 wierszy, 503 bez pliku powtórki (80,10%). Diagnostyka (`legacy_gate_breakdown_on_a_copy`, same liczby) pokazała, że te 503 wiersze to częściowe wiersze z wczesnej alpha: `data_quality = partial`, bez `arenaUniqueID`, bez surowych typów bitwy, bez wersji parsera, z czerwca i lipca 2026; nigdy nie liczyły się do statystyk. Wszystkie 123 pełne bitwy mają powtórki. Ustalenia: (1) bramka P8 liczy tylko wiersze `data_quality = full`; wiersze częściowe nie wchodzą ani do licznika, ani do mianownika progu i zostają wyłącznie w `.bak`; próg 5% bez zmian; (2) sesje, które po przebudowie nie mają żadnej bitwy, nie przechodzą do v10 (zostają w `.bak`); pozostałe sesje i wyzwania jak dotąd; (3) raport przeliczenia pokazuje osobno: pełne bitwy bez powtórki, pominięte wiersze częściowe, pominięte puste sesje. Zmienia D5 w części „Bramka P8” i „przeniesienie danych gracza”.
- **Kolejność (2026-11-09):** M9, N1, N2 najpóźniej w E1, każde z testem czerwonym przed poprawką i testem mutacyjnym. Przystanek „2.0-min” po E2. Ustalenie N3 w §4.1.

- **Decyzje 2026-11-11 (odpowiedzi na §11 badania):**
  1. Skład bitwy: P1, anonimowy (D17).
  2. `marks_on_gun` innych graczy: nie zapisujemy.
  3. Tier przed E2: tak, jako etap E1b.
  4.–5. Ścieżka B i archiwizacja `entity_defs`: notatki badawcze autora (poza repozytorium publicznym) (D18).
  6. Pamięć podręczna `battle_results` gry: nie teraz.
  7. Nazwa funkcji modelu: „Co różni Twoje wygrane od przegranych”; zakaz wpływ/impact/carry utrzymany (D20).
  8. Sprzęt do LLM: decyzja przy specu debriefu.
  9. Ekonomia: trwale poza zakresem.
  10. Premium: nie teraz.
- **Redesign UI (2026-11-11):** zakres E5 według zaakceptowanej makiety (D19).

- **Sprzeczności 2026-11-11 (rozstrzygnięte przez autora):**
  1. **Kontrakt team-relative §36:** D17 uchyla go także, ale tylko dla zapisu w `battle_roster`/`battle_team_stats` i dla syntetycznych fixture'ów. UI w v2.0 pokazuje wyłącznie agregaty drużyn (porównanie drużyn w ekranie Bitwa), nigdy listy graczy składu (D17, §3, E5 krok 3).
  2. **`battle_team_stats`:** liczniki tierów i klas są objęte D17 wprost.
  3. **Liczba „wpływu” na bitwę** (różnica kontrfaktyczna z badania §4.4 pkt 3) nie powstaje ani w v2.0, ani w §11.2: żadnej jednej liczby oceny gracza, tylko porównania. Punkt usunięty z planu (§11.2).
  4. **„Mapy” w nawigacji:** wyszarzone tylko w buildzie prywatnym; w publicznym buildzie ukryte (D18, E5 krok 3).
  5. **Czytnik paczek:** obsługa wpisów `Deflated` z góry dozwolona w E5 krok 1, pod warunkami: limit rozmiaru po rozpakowaniu i współczynnika kompresji przed alokacją, odczyt strumieniowy, testy bomby dekompresji i obcinania. Stop tylko wtedy, gdy tych warunków nie da się spełnić.
  6. **Grafiki z gry:** w v2.0 wyłącznie pamięć w RAM (LRU), bez kopii na dysku. Kopia dyskowa tylko jako osobna decyzja autora, jeśli pomiar pokaże start dłuższy niż 2 s (D19, §8).
  7. **Archiwum `entity_defs`:** notatki badawcze autora (poza repozytorium publicznym). Zamknięte (D18).
  8. **`PRIVACY.md` i `docs/PRIVACY_PL.md`:** E1 aktualizuje oba po usunięciu `python.log`, Tomato i WG (`PRIVACY_PL.md` jako wierne tłumaczenie `PRIVACY.md`); E2 dopisuje w obu zdanie o składzie (D17).
  9. **E6 wchodzi do E5** (krok 3); numeracja pozostałych etapów bez zmian.
  10. **Gałęzie:** każdy etap zakłada gałąź od `main` po scaleniu poprzedniego etapu (§5).
  11. **Cel v2.0 potwierdzony:** konsolidacja, podwaliny danych i redesign, bez nowych metryk analitycznych (§1, §3).

### Otwarte
Brak.

## 10. Szacunek godzin (liczony od zera 2026-11-09, przeliczony 2026-11-11)

| Etap | Godziny | Zmiana 2026-11-11 |
|---|---|---|
| E0 Siatka bezpieczeństwa, zależności, formatowanie | 12 | — |
| E1 Prywatność i utrata danych, mniej kodu, poprawne dane wejściowe | 20,5 | +0,5 (`PRIVACY_PL.md` jako tłumaczenie) |
| E1b Tier i role w katalogu pojazdów | 5 (4–6) | nowy |
| E2 Baza v10, migracje, odtworzenie, doczytanie, podwaliny danych | 27 | +9 (podwaliny 7, testy 2) |
| Przystanek 2.0-min | 1,5 | — |
| E3 Wątki, eventy, wydajność importu | 12,5 | — |
| E4 Analiza i WN8 | 9,5 | — |
| E5 Redesign UI z widokiem czołgu | 49,5 | +29,5 (grafiki 10,5, tokeny 3, nawigacja i ekrany 16, dawny E6 3, rozbicie −3) |
| E6 (włączony do E5) | 0 | −4 |
| E7 Raport tygodniowy (warunkowo) | 10 | — |
| E8 Release i CI | 8,5 | — |

| Ścieżka | Etapy | Godziny | Z buforem 20% | Przed zmianą |
|---|---|---|---|---|
| **Minimalna, wersja 2.0-min** (poprawne dane, bez utraty bitew, prywatność, tier, podwaliny) | E0, E1, E1b, E2 + przystanek | 66 | 79 | 51,5 / 62 |
| Z redesignem | E0–E5 (z E1b i dawnym E6) | 136 | 163 | 92 / 110 |
| Pełna bez raportu tygodniowego | E0–E5 + E8 | 144,5 | 173 | 104,5 / 125 |
| Pełna z raportem tygodniowym | E0–E8 | 154,5 | 185 | 114,5 / 137 |

Ścieżka pośrednia E0–E4 (bez redesignu): 86,5 h, z buforem 104 h.

Wersja 2.0-min i ścieżka „z redesignem” nie obejmują E8. Taka wersja trafia tylko do autora (P5); do znajomych dopiero po E8.

Wzrost 2026-11-11 (114,5 h → 154,5 h dla pełnej, +40 h): redesign E5 z dawnym E6 netto +25,5 h, podwaliny v10 +9 h, E1b +5 h, E1 +0,5 h. Wzrost względem 8.11 (91 h → 114,5 h dla pełnej) wynika ze skali kodu (+39,8 tys. linii). Najbardziej rosną:
- E1: prywatność i utrata danych z testami mutacyjnymi, usuwanie ścieżki splecionej z nowymi modułami, predykat w 6 modułach;
- E2: przeniesienie danych gracza, silnik importu z kursorem, bramka P8, tryb awaryjny;
- E5: 8 gęstych plików zamiast jednego.

Koszt dawnego E6 i E7 maleje, bo funkcje już istnieją.

## 11. Wizja po v2.0

Zasada nadrzędna: **wszystko wyłącznie po bitwie, nic w trakcie gry.** Aplikacja czyta tylko zakończone pliki powtórek. Nie robi nakładek, nie czyta pamięci ani ruchu sieciowego klienta i nie podaje żadnych informacji w trakcie bitwy, zgodnie z Fair Play Policy Wargaming. Każdą funkcję z tej listy sprawdzamy pod tym kątem przed specem.

Zmiana 2026-11-09 (autor): wizja ma dwie ścieżki. **Ścieżka A** (analiza wyników z bloku wyników powtórki) stoi na danych, które aplikacja już ma, i idzie pierwsza. **Ścieżka B** jest osobnym eksperymentem, od 2026-11-11 wyłącznie prywatnie (D18, notatki badawcze autora (poza repozytorium publicznym)).

Zmiana 2026-11-11 (autor, na podstawie badania podwalin): tier pojazdu przeszedł do v2.0 jako etap E1b, a dane potrzebne ścieżce A zapisuje baza v10 (E2, D17). Kolejność: v2.0 → 11.1 → 11.2 → 11.3; ścieżka B niezależnie, gdy autor zdecyduje. Każdy punkt dostaje własny spec (21, 22, …) przed kodem.

### 11.1 Ścieżka A, krok 1: tabele odniesienia per tier i klasa (spec 21)
- **Najpierw tabele, potem jakikolwiek model.** Dla każdej komórki (tier, klasa): mediana i kwartyle obrażeń, asysty (radio + gąsienice), wykryć i czasu życia z wierszy `battle_roster` wszystkich uczestników bitew gracza (badanie §4.4 pkt 1).
- **Dla gracza:** „na tle graczy w takim samym czołgu w moich bitwach”. Ekran Czołg z E5 dostaje to porównanie bez zmiany układu.
- **Skala:** ok. 63 tys. rekordów gracz-bitwa u autora. Te rekordy zasilają tabele odniesienia, **nie model** (11.2). Rozkład jest nierówny (w próbie badania 79% wierszy to tier XI); komórka z małą liczbą rekordów pokazuje „za mało danych”, a nie wartość.
- **Zasady:** tylko bitwy kwalifikujące się (D4, warunki team-relative §6); żadnych identyfikatorów (D17); tabela odniesienia liczona przy odczycie z liczników, bez zapisu wartości pochodnych.
- Dawny punkt „tier pojazdu w katalogu” jest zrealizowany w E1b.
- **Zmiana 2026-11-15 (autor, po badaniu A-1): A-1 przechodzi do kandydatów na funkcję** — porównanie percentylowe z graczami z bitew autora, liczone lokalnie, bez wysyłania danych. Badanie na powtórkach autora (notatki badawcze autora, poza repozytorium publicznym): 2012 bitew, 57 424 wiersze populacji, każda komórka tier × klasa VI–XI ma ≥ 200 wierszy poza artylerią VI (140). Ustalenia: próg 200 wierszy (mniej = „za mało danych”); wartość gracza (mediana) od 10 jego bitew w komórce, poniżej tylko tło; bez podziału na pozycję w bitwie (do A-2); wszystkie bitwy, rewizja przy ok. dwukrotnie większej liczbie danych; porównanie wyłącznie w percentylach, celność tylko jako tło. **Warunek wejścia do aplikacji:** mutacje `baseline.rs` ≥ 85% (backlog mutacji).

### 11.2 Ścieżka A, krok 2: „Co różni Twoje wygrane od przegranych” (spec 22)
- **Nazwa i słownictwo (D20):** funkcja opisuje różnice między wygranymi i przegranymi, nie przyczyny. Słowa z zakazu kontraktu team-relative §4, po polsku łącznie z „wpływ”, nie występują w nazwie, opisie ani UI. Brak jednej liczby „oceny gracza”.
- **Próba:** etykieta (wygrana/przegrana) jest jedna na bitwę, a 30 wierszy jednej bitwy jest od siebie zależnych. **Efektywna próba modelu to ok. 2,1 tys. bitew, nie 63 tys. rekordów** (badanie §4.3). Przy tej próbie rozsądne są 10–20 cech; bez gradient boostingu i bez modeli per czołg albo per mapa (badanie §4.4, §9 pkt 7).
- **Dane wejściowe (po v2.0):** kolumny `battles` z E2, `battle_team_stats` (różnice między drużynami, skład tierów i klas, plutony), `team_relative`, `attempts_conversion`, mapa (`arena_type_id`). Walkowery z `battle_pace` wykluczone z uczenia i oceny.
- **Cechy-przecieki wykluczone bezwzględnie** (badanie §4.2): `xp`, `original_xp`, miejsce według XP, kredyty, osiągnięcia, `markOfMastery` oraz wszystko pochodne od wyniku: `destroyed_team*_score`, tempo bitwy, `finishReason`, `duration`, `teamHealth`, `commonNum*`. Pozostałe statystyki końcowe (obrażenia, fragi, przetrwanie) są współwyznaczane przez wynik; opis funkcji to mówi wprost.
- **Metoda (do ustalenia w specu 22):** regresja logistyczna z regularyzacją na poziomie bitwy, walidacja chronologiczna (uczenie na starszych bitwach, test na nowszych), kalibracja prawdopodobieństw. Model uczony i uruchamiany lokalnie.
- **Bez liczby „wpływu” na bitwę:** różnica kontrfaktyczna z badania §4.4 pkt 3 jest usunięta z planu (rozstrzygnięcie 2026-11-11 pkt 3). Funkcja pokazuje wyłącznie porównania (wygrane vs przegrane, gracz vs tabela odniesienia), nigdy jednej liczby oceny gracza ani bitwy.

### 11.3 Ścieżka A, krok 3: debrief z lokalnego LLM z cytatami do statystyk (spec 23)
- Krótki opis bitwy lub sesji prostym językiem, w którym każde zdanie wskazuje konkretną wartość (pole, bitwę, wynik z 11.1 lub 11.2). Zdanie bez odnośnika nie trafia do raportu. Ta sama metoda co w analizatorze działek: ugruntowanie cytatów i zero zmyśleń jako miara jakości (badanie §7.3).
- Model nie liczy i nie ocenia: liczby i porównania powstają w Ruście, model tylko formułuje zdania.
- Model lokalny, bez chmury, nie w instalatorze i nie pobierany bez osobnej zgody. **Wybór modelu i sprzętu docelowego: decyzja przy specu 23** (kandydaci w badaniu §7.1).
- Ocena na ręcznie sprawdzonym zestawie bitew.
- Miejsce na podsumowanie z odnośnikami jest już w układzie ekranu Bitwa (E5).

Projekt włączania, doboru modelu do sprzętu i uruchamiania na żądanie: spec 28 (DRAFT, po 2.0).

### 11.4–11.5 Ścieżka B

Ścieżka B (eksperyment badawczy, wyłącznie prywatnie, D18) jest opisana w notatki badawcze autora (poza repozytorium publicznym).

### 11.6 Później albo nie robimy

**Później** (dane są w powtórkach, więc da się je doczytać jawnym przebiegiem według §4.1):

| Co | Dlaczego nie teraz |
|---|---|
| `battles`: `mark_of_mastery`, `is_first_blood`, `damaged_while_moving`, `sniper_damage` (badanie §8.2 E) | tanie, ale semantyka częściowo niepewna (matryca: AMBIGUOUS); bez odbiorcy w v2.0 |
| `personal.details` (per cel: trafienia, krytyki, pożary) | bitmaski `crits` bez kontraktu wersji; brak odbiorcy w §11 |
| `R[1].position`, `deathInfo` | znaczenie niezweryfikowane; `killer_slot` (D17) daje graf zniszczeń |
| Cokolwiek ze strumienia pakietów w bazie | przed spike'em nie wiadomo, co i w jakiej postaci zapisywać; źródłem zostaje plik powtórki |
| Pamięć podręczna wyników bitew gry (`battle_results\*.dat` w `%APPDATA%`) | decyzja autora: nie teraz; poza zatwierdzonym folderem gry, publiczne parsery nieaktualne, format pickle |
| Pojazd premium jako cecha | decyzja autora: nie teraz; brak jednoznacznego znacznika w `list.xml` (badanie §2) |

**Nie robimy:**

| Co | Dlaczego |
|---|---|
| Ekonomia (kredyty, złoto, czynniki premium, 170+ pól) | decyzja autora: trwale poza zakresem; `PRIVACY.md` deklaruje, że aplikacja nie czyta kredytów i złota |
| `R[0].avatars`, Battle Pass, misje, odznaki | bez wartości analitycznej |
| Surowy JSON wyników „na zapas” | łamie `PRIVACY.md` („does not store raw replay bytes”) i zawiera nicki |
| Wartości pochodne w bazie (udziały, rankingi, WN8, tabele odniesienia) | liczone przy odczycie z liczników (D3) |
| `marks_on_gun` innych graczy | decyzja autora; jedyne pole mówiące coś o graczu, nie o bitwie (D17) |
| Nicki, `accountDBID`, klany, surowy `prebattleID`, także zahaszowane | hasz stałego identyfikatora nadal jest identyfikatorem (badanie §9 pkt 1) |
| Publiczne API WG (np. encyklopedia pojazdów) | wymaga `application_id` i sieci, licencja zabrania trwałych kopii danych, odwraca D2; tier i klasa są lokalnie w `list.xml` |
| Kopiowanie kodu z repozytoriów bez licencji (`replays_unpack`) i ze zdekompilowanego klienta (`izeberg/wot-src`, `WorldOfTanks-Decompiled`) | brak licencji; dwa ostatnie to kod WG (EULA 5.3(b)); nie używamy ich nawet jako ściągi |
| Cokolwiek w trakcie bitwy (nakładki, pamięć, ruch sieciowy, czytanie `temp.wotreplay` na żywo) | Fair Play Policy; zasada nadrzędna §11 |
