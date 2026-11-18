# Spec 31 — 1.0 RC, część 1: ostatnie szlify UI

- **Status: APPROVED (decyzje autora 2026-11-17).** Zamiast osobnej bety 5: razem ze spec 32 tworzy jedno wydanie `1.0.0-rc.1` (wydanie i akceptacja: spec 32 §6–§7).
- **Makiety (zaakceptowane 2026-11-17):** płótno „Afterbattle 2.0 — redesign”, rząd „1.0 RC: ostatni szlif i nowe ekrany”: `RC-Sesja`, `RC-Podsumowanie`, `RC-Ustawienia`, `RC-Start`, `RC-Aktualizacja` (plus `RC-Przeglad`, `RC-Czolg` dla spec 32). Grafiki map i ikony czołgów na makietach to zamienniki (D19); w aplikacji zostają z plików gry jak dziś.
- Data: 2026-11-17. Autor decyzji: Szymon.
- Rozwija: spec 25 (UI), spec 29 (ruch, marka, tokeny `--motion-*`), spec 30 (podsumowania).
- Miejsce w planie: `1.0.0-rc.1` = ten spec + spec 32 (A-1, Przegląd, aktualizacje). Zakres 1.0.0 jest zamknięty. Nowe pomysły trafiają do listy „po 1.0.0” w STATUS, nie do tego speca.

## 0. Zakres i kolejność

| Etap | Gałąź | Sekcja |
|---|---|---|
| 0 | `v2/e11-feedback` | §1 Zgłoszenia testerów bety 4 |
| 1 | `v2/e11-motion` | §2 Snappy: liczby i tokeny ruchu |
| 2 | `v2/e11-recap-toggle` | §3 Podsumowanie: pierwsze zdanie + „Pokaż więcej” |
| 3 | `v2/e11-settings` | §4 Nowy układ Ustawień |
| 4 | `v2/e11-review` | §5 Przegląd UI |
| 5 | `v2/e11-backlog` | §6 Drobne zaległości (wydanie: spec 32 §7) |

**Poza zakresem:** nowe ekrany, nowe opcje w Ustawieniach, zmiany parsera, bazy i metryk (golden musi dać 2013 / `1703f8dbeb68775e`), A-1, Dashboard, aktualizacje (spec 32), LLM (spec 28), angielskie podsumowania.

## 1. Zgłoszenia testerów bety 4 (etap 0)

- Na 2026-11-17 brak zgłoszeń. Zgłoszenia, które przyjdą w trakcie biegu, autor dopisuje do STATUS (tabela: numer, opis, ekran, decyzja: poprawka / po 1.0.0 / odrzucone); wykonawca bierze tylko te z decyzją „poprawka”.
- Każda poprawka z testem, który był czerwony przed zmianą.
- Zgłoszenia, które wymagają nowej funkcji, idą do listy „po 1.0.0”.

## 2. Snappy: liczby i tokeny ruchu (etap 1)

„Snappy” znaczy mierzalnie:

| Miara | Próg |
|---|---|
| Reakcja na kliknięcie (stan `:active`, zmiana zaznaczenia) | < 100 ms, bez czekania na dane |
| Przejście między ekranami | 150–200 ms (`--motion-base`) |
| Hover, fokus, przyciski | 120–150 ms (`--motion-hover`) |
| Wejście nowych elementów | ≤ 250 ms (`--motion-enter`) |
| Liczby (count-up) | bez zmian, 600 ms, tylko przy nowej bitwie (spec 29 §6) |
| Przesunięcia układu po wczytaniu (CLS) | 0 na ekranach Sesja, Historia, Czołgi, Bitwa, Cele |

Wymagania:
- Jedno źródło: tokeny `--motion-*` i `--ease-*` w `src/styles/tokens.css`. Żadnych literałów czasu ani krzywej w komponentach; test (grep w CSS i TSX) to pilnuje.
- Przejście ekranów: krótkie przenikanie z przesunięciem ≤ 8 px. Stare dane zostają do nadejścia nowych (spec 29 §1), bez klatki ładowania.
- Stany `:active` na przyciskach, wierszach i kafelkach: natychmiastowa zmiana tła lub skali (≤ 2%).
- Szkielety ładowania tylko tam, gdzie dane przychodzą dłużej niż 300 ms; o stałych wymiarach.
- `prefers-reduced-motion`: przejścia zastąpione zmianą bez ruchu, count-up wyłączony (jak dziś).
- Bez animacji w trakcie bitwy (Fair Play): aplikacja nie zmienia zachowania, gdy gra jest aktywna.
- Pomiar: zrzut z `tools/screens` przed i po + krótka tabela czasów w STATUS (Performance API w trybie testowym albo pomiar ręczny autora).

**Lista ruchu (zaakceptowana 2026-11-17):**
- Zostaje bez zmian (spec 29): wjazd nowego kafelka bitwy na Sesji; liczby 600 ms tylko przy nowej bitwie; fala w Historii (14) i wsunięcie nowego wiersza; grafiki przez `opacity` bez zmiany rozmiaru; ekran ładowania z żarem według postępu (> 400 ms).
- Nowe: przejścia ekranów (przenikanie, przesunięcie ≤ 8 px, 150–200 ms); `:active` < 100 ms (tło lub skala ≤ 2%), hover 120–150 ms; rozwinięcie podsumowania (wysokość 250 ms, obrót strzałki, bez skoku treści nad kartą); przełączniki w Ustawieniach 150 ms i „Zapisano” z wygaszeniem; wskaźnik zaznaczenia w przełącznikach segmentowych (pierwsze uruchomienie, Ustawienia); okienko aktualizacji wsuwa się z prawego górnego rogu; szkielety tylko > 300 ms.
- Celowo bez ruchu: nic w trakcie bitwy; Przegląd bez animacji wejścia przy powrocie; porównanie A-1 (paski i znacznik) statyczne; zmiana czołgu bez animacji wejścia; `prefers-reduced-motion` — wszystko natychmiast.

## 3. Podsumowanie: pierwsze zdanie + „Pokaż więcej” (etap 2)

- Karta podsumowania (Sesja, Bitwa, Cele) pokazuje domyślnie **pierwsze zdanie** z odnośnikiem i przycisk „Pokaż więcej” / „Pokaż mniej”.
- Rozwinięcie: wysokość animowana tokenem `--motion-enter`, bez przesuwania przewiniętej treści nad kartą.
- Stan zapamiętany w preferencjach (jedno ustawienie dla wszystkich ekranów), domyślnie zwinięte.
- Odnośniki F1… działają w obu stanach; po zwinięciu podświetlenie liczb spoza pierwszego zdania znika.
- Dostępność: przycisk z `aria-expanded` i `aria-controls`, obsługa klawiaturą.
- **Decyzja autora (2026-11-17):** domyślnie zwinięte; przycisk „Pokaż więcej (N)” z liczbą ukrytych zdań (makieta `RC-Sesja`, `RC-Podsumowanie`).

## 4. Nowy układ Ustawień (etap 3)

Bez nowych opcji i bez zmian w logice (poza przełącznikiem aktualizacji ze spec 32 §5); tylko układ, kolejność i opisy. Makieta: `RC-Ustawienia`.

- Grupy w kolejności użycia:
  1. **Gra** — folder gry, wstrzymaj / wznów śledzenie;
  2. **Ty** — język, forma zwracania się, czas bezczynności kończący sesję;
  3. **Dane** — import, przebudowa, katalog pojazdów, WN8 i XVM (ze zgodą);
  4. **Prywatność i dane** — sprawdzanie aktualizacji (spec 32 §5), co zostaje na komputerze, usunięcie wszystkich danych (na końcu; ostrzega tylko przycisk);
  5. **O aplikacji** — wersja, diagnostyka.
  - Zmiana autora 2026-11-17 (po STOP 1): eksport anonimowej sesji i licencje usunięte z listy — dziś ich nie ma, a RC nie dodaje opcji; są na liście „po 1.0.0” w STATUS.
- Nawigacja w obrębie ekranu: lista grup po lewej (szerokie okno) albo nagłówki z kotwicami (wąskie).
- Każda opcja: etykieta, jedno zdanie opisu, stan. Zapis natychmiast tam, gdzie dziś jest przycisk „Zapisz” bez potrzeby potwierdzenia (język, forma, bezczynność) — z komunikatem „Zapisano”.
- Testy: każda dotychczasowa opcja jest dostępna i działa jak przed zmianą (test charakteryzacyjny przed przebudową).

## 5. Przegląd UI (etap 4)

- Spójność odstępów (siatka 4/8 px z tokenów), promieni, cieni i kolorów stanów.
- Stany puste na każdym ekranie: jedno zdanie, co zrobić, i akcja, jeśli jest.
- Fokus widoczny na każdym elemencie interaktywnym; kolejność tabulacji zgodna z układem.
- Kontrast tekstu ≥ 4,5:1 (tekst drugorzędny ≥ 3:1 tylko dla dużych etykiet).
- Wynik: lista znalezisk w STATUS (ekran, problem, poprawka) i zrzuty przed / po.

## 6. Drobne zaległości (etap 5)

- `src/publicSnapshot.test.ts`: jawny limit czasu 30 s dla testów uruchamiających PowerShell (CI bety 4 padło na 5 875 ms). Jeśli zrobione wcześniej osobną gałęzią — tylko sprawdzić.
- Notatki do wydania zapisywane bez BOM (`[IO.File]::WriteAllLines`); polecenie w sekcji wydania w STATUS.
- Mutacje przed wydaniem: tylko funkcje decyzyjne `recap.rs` (reguły wyboru faktów i progi), ≤ 30 mutantów (`cargo mutants --list` najpierw), w tle, `-j 2`. Powyżej 30 — zakres i liczba do STATUS, bez startu.
- Wersja, CHANGELOG i zrzuty: wspólnie ze spec 32 §7 (`1.0.0-rc.1`).

## 7. Akceptacja (część wspólnej akceptacji RC, spec 32 §6)

- Kontrole z §0 STATUS zielone; golden autora 2013 / `1703f8dbeb68775e`.
- Test ręczny autora: nowa sekcja w liście „Test ręczny 2.0.0” (snappy na 5 ekranach, karta podsumowania zwinięta/rozwinięta, Ustawienia — każda opcja).
- Migawka publiczna: zero nowych trafień skanu; CI publiczne zielone.

## 8. Wycena (z historii etapów)

Podobna do E9 (ruch, ładowanie, marka): ok. 1 500–2 500 linii, ok. 2–3 h pracy agenta, ok. 1 h przeglądu autora. Zgłoszenia testerów nieznane — doliczane po ich zebraniu.
