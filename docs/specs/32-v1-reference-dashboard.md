# Spec 32 — 1.0 RC, część 2: porównanie z graczami (A-1), Przegląd, aktualizacje

- **Status: APPROVED (decyzje autora 2026-11-17, §9).** Razem ze spec 31 tworzy jedno wydanie `1.0.0-rc.1` (zamiast bet 5 i 6).
- **Makiety (zaakceptowane 2026-11-17):** płótno „Afterbattle 2.0 — redesign”, rząd „1.0 RC”: `RC-Przeglad`, `RC-Czolg`, `RC-Start`, `RC-Aktualizacja`, `RC-Ustawienia`. Dawna plansza `Moon-A1-Czolg` jest nieaktualna (nagłówek oceniający łamie D20).
- Data: 2026-11-17. Autor decyzji: Szymon.
- Rozwija: spec 20 §11.1 (A-1 jako funkcja, decyzje autora z 2026-11-15), wynik badania A-1 (notatki badawcze autora, poza repozytorium publicznym), spec 25 (UI), spec 29 (ruch), spec 31 (beta 5).
- **Zakres 1.0.0 jest zamknięty (decyzja autora 2026-11-17):** spec 31 + ten spec. Każdy nowy pomysł trafia do listy „po 1.0.0” w STATUS.
- Nazwa ekranu: **Przegląd** (w kodzie i dokumentach wcześniej „Dashboard”); pierwsza pozycja nawigacji.
- Podpis Authenticode **poza 1.0.0** (decyzja autora 2026-11-17): instalator bez podpisu jak w becie; wraca tylko, jeśli testerzy odpadają na ostrzeżeniu SmartScreen.

## 0. Zakres i kolejność

| Etap | Gałąź | Sekcja |
|---|---|---|
| 0 | `v2/e12-baseline-mutants` | §1 Warunek wejścia: mutacje `baseline.rs` |
| 1 | `v2/e12-reference` | §2 Tabele odniesienia (backend) |
| 2 | `v2/e12-reference-ui` | §3 Porównanie na ekranie Czołg |
| 3 | `v2/e12-overview` | §4 Przegląd i ekran startowy |
| 4 | `v2/e12-updater` | §5 Automatyczne aktualizacje |
| 5 | `v2/e12-release` | §6 Akceptacja, §7 Wydanie `1.0.0-rc.1` |

Kolejność całego biegu RC: spec 31 (etapy E11) → ten spec (etapy E12) → wydanie `1.0.0-rc.1`. Etap 0 (mutacje) może ruszyć w tle wcześniej. Po RC: dwa tygodnie, potem `1.0.0` (§7).

**Poza zakresem:** A-2 (wygrane a przegrane, podział na pozycję w bitwie), LLM (spec 28), ścieżka B, Authenticode, nowe metryki w parserze. Golden musi dać 2013 / `1703f8dbeb68775e`.

## 1. Warunek wejścia: mutacje `baseline.rs` (etap 0)

- Warunek z spec 20 §11.1 i z backlogu mutacji autora: ≥ 85% dla funkcji decyzyjnych `baseline.rs`.
- Pełny moduł to 98 mutantów (przebieg z 2026-11-15), więc zgodnie z regułą z 2026-11-16: wykonawca zawęża filtrem (`-F`) do funkcji wybierających okno i liczących odniesienie (`load_candidates`, `wn8_window` i te, których używa §2), podaje `cargo mutants --list` z liczbą. ≤ 30 — start w tle, `-j 2`; > 30 — zakres i liczba do STATUS, stop do decyzji autora.
- Trzy przeżyłe z 2026-11-15: test albo uzasadnienie równoważności w backlogu mutacji autora.

## 2. Tabele odniesienia (etap 1)

### 2.1 Źródło populacji — **decyzja autora przed startem**

| Opcja | Opis | Za | Przeciw |
|---|---|---|---|
| **A. Tylko lokalnie (rekomendacja)** | Populacja = wiersze `battle_roster` z bitew gracza, liczone przy odczycie (jak w badaniu). | Zgodne z decyzją z 2026-11-15 („liczone lokalnie, bez wysyłania danych”); zero nowych danych w instalatorze; każdy gracz ma tło ze swoich serwerów i swojego poziomu. | Nowy gracz długo widzi „za mało danych” (ok. 28,5 wiersza na bitwę → próg 200 w komórce wymaga kilkudziesięciu bitew w danym tierze i klasie). |
| B. Tabela dołączona do aplikacji | Zagregowane percentyle z bitew autora w instalatorze, zastępowane lokalnymi, gdy komórka osiągnie próg. | Od pierwszego uruchomienia coś widać. | Tło z jednego konta i jednego serwera; dane innych graczy (choć zagregowane) w publicznym repo; trzeba je odświeżać przy zmianach balansu. |

**Decyzja autora (2026-11-17): A**, z czytelnym stanem postępu w komórce („tło od 200 wyników, masz 120”). Opcja B na liście „po 1.0.0”, jeśli testerzy uznają pusty ekran za problem.

### 2.2 Wymagania (przy opcji A)

- Komórka = (tier, klasa). Tiery VI–XI; poniżej VI tylko liczność.
- Metryki: obrażenia, asysta radio + gąsienice, wykrycia, zniszczenia, zablokowane, czas życia; celność **tylko jako tło** (decyzja 2026-11-15 pkt 4).
- Progi (decyzja 2026-11-15): wartości od **200 wierszy**; 50–199 oznaczone „orientacyjnie”; < 50 tylko liczność. Wartość gracza (mediana) od **10 jego bitew** w komórce; poniżej tylko tło.
- Populacja: wiersze składu bez gracza, jego plutonu i wierszy bez tieru lub klasy; tylko bitwy liczone (D4); bez duplikatów kopii.
- Porównanie **wyłącznie w percentylach** (p25, mediana, p75, p90 i pozycja mediany gracza w rozkładzie). Bez jednej liczby „oceny gracza” (D20), bez słów oceniających.
- Wszystkie bitwy (bez okna czasowego); rewizja przy ok. dwukrotnie większej liczbie danych.
- Liczone przy odczycie z `battle_roster`, bez zapisu wartości pochodnych; komenda Tauri zwraca tylko agregaty (typy z `ts-rs`).
- **Prywatność:** zero identyfikatorów (D17); test, że odpowiedź nie zawiera `typeCompDescr` ani niczego z wierszy pojedynczych bitew; komórki < 50 bez wartości.
- Wydajność: zapytanie dla wszystkich komórek < 200 ms przy 60 tys. wierszy (pomiar na danych syntetycznych tej skali).
- Testy: zgodność z wydrukiem badania na syntetycznym zestawie o znanych percentylach; progi 49/50/199/200 i 9/10 bitew; pluton wykluczony.

## 3. Porównanie na ekranie Czołg (etap 2)

- Zgodnie z spec 20 §11.1: „na tle graczy w takim samym czołgu w moich bitwach”, w panelu czołgu, bez zmiany układu ekranu (nowa sekcja w panelu szczegółów).
- Dla każdej metryki: pasek rozkładu populacji (p25–p75, mediana, p90) i znacznik mediany gracza; podpis liczbowy; stan „za mało danych” z postępem.
- Tier i klasa z wybranego czołgu. **Decyzja autora (2026-11-17): tylko komórka tier × klasa, bez przełącznika.** Znacznik gracza = mediana ze wszystkich jego bitew w tej komórce (podpis mówi to wprost, makieta `RC-Czolg`).
- Odnośniki jak w podsumowaniach (spec 30): najechanie podświetla liczbę.
- Makieta: `RC-Czolg`. Paski i znacznik statyczne (bez animacji).

## 4. Dashboard i ekran startowy (etap 3)

Makieta: `RC-Przeglad`.

### 4.1 Ekran startowy zależny od sesji

- Trwająca sesja (w tym zakończona w ciągu ostatnich N minut bezczynności) → start na **Sesji**.
- Brak trwającej sesji → start na **Dashboardzie**.
- Ręczny wybór ekranu w nawigacji zawsze wygrywa do końca uruchomienia. Bez ustawienia „ekran startowy” w 1.0.0.

### 4.2 Zawartość Dashboardu

Tylko dane, które aplikacja już liczy (bez nowych metryk w parserze):

| Blok | Źródło |
|---|---|
| Ostatnia sesja: liczby i pierwsze zdanie podsumowania | `get_session_summary`, `get_recap` |
| Trendy: obrażenia, wygrane (kontekst, nie cel — D12), WN8 z XVM, w oknie ostatnich 100 bitew wobec 100 wcześniejszych, podpis „ostatnie 100 bitew” (decyzja 2026-11-18) | `get_performance_trends` |
| Najlepsze czołgi (percentyl mediany obrażeń czołgu w jego komórce tier × klasa, decyzja 2026-11-18) i najczęściej grane jedną linią | `get_vehicle_history_overview`, `get_reference_tables` (pozycje czołgów) |
| Tło A-1: w których komórkach tier × klasa mediana gracza jest powyżej / poniżej mediany populacji (bez oceny, tylko pozycja) | §2 |
| Cele tygodnia: stan | `get_weekly_report`, `get_active_challenge` |

- „Najlepsze czołgi”: kryterium jawne w podpisie; bez słów oceniających.
- Każdy blok klikalny → właściwy ekran (Sesja, Czołgi, Historia, Cele).
- Stany puste dla nowego gracza (0 bitew, < 10 bitew, brak XVM).
- Ruch według spec 31 §2; bez animacji wejścia przy powrocie na ekran.
- „Najlepsze czołgi” — **decyzja autora (2026-11-18, zastępuje decyzję z 2026-11-17):** percentyl mediany obrażeń czołgu na tle populacji jego komórki tier × klasa (§2), od 10 bitew na czołgu w tej komórce; komórki z mniej niż 200 wierszami populacji wykluczone. Top 3, podpis przy czołgu: „88. percentyl w ciężkich X” (na ekranie „88.” i „w ciężkich X”). Liczone przy odczycie razem z tabelami §2 (`get_reference_tables`, pole `tanks`: identyfikator własnego czołgu, tier, klasa, liczba bitew, mediana, pozycja); żadnych danych innych graczy poza agregatami.
- Ikony czołgów i tło mapy (karta ostatniej sesji: mapa najlepszej bitwy) z plików gry jak na innych ekranach (D19); brak grafiki → neutralny zamiennik.

## 5. Automatyczne aktualizacje (etap 4)

### 5.1 Zgoda na sieć — **decyzja autora przed startem**

Aktualizacje to nowe połączenie sieciowe (dziś sieć tylko dla XVM i katalogu, za zgodą). **Decyzja autora (2026-11-17): pytanie przy pierwszym uruchomieniu** („Sprawdzać aktualizacje przy starcie?”, makieta `RC-Start`) + przełącznik w Ustawieniach → Prywatność i dane. Istniejący użytkownicy (po aktualizacji z bety): to samo pytanie raz, przy pierwszym starcie RC. Bez zgody: brak zapytań; w „O aplikacji” link do strony wydań.

### 5.2 Wymagania

- `tauri-plugin-updater` (Tauri v2), NSIS. Podpis aktualizacji kluczem autora (Ed25519/minisign): klucz prywatny **poza repo i poza CI publicznym**; publiczny w `tauri.conf.json`.
- `bundle.createUpdaterArtifacts: true`; `scripts/release.ps1` dokłada podpisane artefakty i generuje `latest.json`. Klucz generuje autor (`pnpm tauri signer generate`), plik klucza poza repo; `release.ps1` czyta go ze zmiennej środowiskowej i bez niej odmawia budowania z artefaktami aktualizacji. Wykonawca nigdy nie generuje ani nie czyta klucza prywatnego.
- Endpoint: `https://github.com/dataflorist02/afterbattle-beta/releases/latest/download/latest.json`. Uwaga: `latest` wskazuje wydanie oznaczone jako najnowsze, nie pre-release — bety nie aktualizują się same; pierwszą automatyczną aktualizację da wydanie po 1.0.0.
- Uprawnienia Tauri tylko `updater:default` (+ `process` do ponownego uruchomienia), dopisane w capabilities z testem listy uprawnień.
- UX: sprawdzenie raz przy starcie, w tle; informacja „Dostępna wersja X” z notatkami i przyciskami „Zainstaluj i uruchom ponownie” / „Później”. Nigdy w trakcie bitwy i nigdy bez kliknięcia gracza.
- Aktualizacja nie rusza bazy poza migracjami (test migracji z 1.0.0-beta.6 → 1.0.0).
- PRIVACY.md, docs/PRIVACY_PL.md, THREAT_MODEL: nowe połączenie (co jest wysyłane: tylko zapytanie HTTP o plik; brak identyfikatorów), ryzyko podmiany aktualizacji i obrona podpisem.

## 6. Akceptacja (`1.0.0-rc.1`, razem ze spec 31 §7)

- Kontrole z §0 STATUS zielone; golden 2013 / `1703f8dbeb68775e`.
- Mutacje `baseline.rs` (funkcje z §1) ≥ 85% lub uzasadnione.
- Test ręczny autora: nowa sekcja w liście „Test ręczny 2.0.0” (Dashboard przy sesji i bez, porównanie A-1 na 3 czołgach różnych klas, stan „za mało danych”, aktualizacje: zgoda, odmowa, próba na lokalnym `latest.json`).
- Migawka publiczna: zero nowych trafień; CI publiczne zielone.

## 7. Wydanie

- `1.0.0-rc.1` (pre-release, w aplikacji „Afterbattle 1.0 RC”). Po wydaniu tylko poprawki błędów, każda jako `rc.N`.
- **Zegar dwóch tygodni:** od wydania RC; startuje od nowa tylko przy poważnym błędzie (utrata danych, crash, złe liczby). Drobiazgi go nie zerują.
- Po dwóch tygodniach spokoju: **ten sam commit** jako `1.0.0`, bez zmian w kodzie (zmienia się tylko wersja w plikach i CHANGELOG).
- Użytkownicy RC instalują `1.0.0` ręcznie (aktualizator czyta tylko wydanie oznaczone jako najnowsze, nie pre-release).
- `1.0.0`: zwykłe wydanie (nie pre-release), z `latest.json` i podpisanymi artefaktami aktualizacji. Instalator bez Authenticode — w notatkach jedno zdanie o ostrzeżeniu SmartScreen i sumie SHA-256.
- SECURITY.md i RELEASE_CHECKLIST: dziś Authenticode jest tam wymogiem dla 1.0.0 — zmienić zgodnie z decyzją autora z 2026-11-17 (podpis aktualizacji Tauri + SHA-256; Authenticode po 1.0.0, warunkowo).
- Po 1.0.0: case study na dataflorist.com (styczeń 2027), potem lista „po 1.0.0”.

## 8. Wycena (z historii etapów)

Większa niż spec 30: nowy backend (§2), dwa ekrany (§3, §4) i nowa zależność z siecią (§5). Szacunek: ok. 3 500–5 000 linii, ok. 4–6 h pracy agenta w 5 etapach, ok. 2–3 h przeglądu autora (makiety, decyzje, test ręczny). Mutacje `baseline.rs` zależne od liczby po zawężeniu.

## 9. Rozstrzygnięcia autora (2026-11-17)

1. §2.1 źródło populacji: **A, tylko lokalnie**.
2. §3: **tylko komórka tier × klasa**, bez przełącznika.
3. §5.1: **pytanie przy pierwszym uruchomieniu** + przełącznik w Ustawieniach.
4. §4.2: ~~mediana obrażeń wobec Twojej średniej, od 10 bitew~~ — zmienione 2026-11-18, pkt 7.
5. Spec 31 §3: podsumowanie **domyślnie zwinięte**.
6. Jedno wydanie `1.0.0-rc.1` zamiast bet 5 i 6; 1.0.0 = ten sam commit po dwóch tygodniach bez poważnego błędu.

### Rozstrzygnięcia autora (2026-11-18, wznowienie po pauzie)

7. §4.2 „Najlepsze czołgi”: **percentyl mediany obrażeń czołgu w jego komórce tier × klasa** (§2), od 10 bitew na czołgu, komórki < 200 wierszy wykluczone; top 3, podpis „88. percentyl w ciężkich X”.
8. §4.2 Trendy: **okno 100 bitew** (bez 30 dni), podpis „ostatnie 100 bitew”.
9. §5.2: sprawdzanie i instalacja z Rusta za bramką zgody, **bez uprawnień JS `updater:default` i `process`** (mniej, niż dopuszczał ten punkt). `bundle.createUpdaterArtifacts` wyłączone w konfiguracji; `scripts/release.ps1` włącza je przez `--config` tylko z kluczem w `TAURI_SIGNING_PRIVATE_KEY` (hasło: `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), bez klucza odmawia. Klucz publiczny autora w `tauri.conf.json`.
10. §5.2 test migracji: dla RC to **1.0.0-beta.4 → 1.0.0-rc.1** (schemat 10 bez kroków migracji).
