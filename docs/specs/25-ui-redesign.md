# Spec 25 — Redesign UI (E5 speca 20)

- **Status:** ROBOCZY wykonawcy (2026-11-12), realizowany w biegu E5 bez osobnej akceptacji (polecenie autora). Zmiany zakresu względem speca 20 są niedozwolone; tu jest tylko „jak”.
- **Nadrzędne:** `docs/specs/20-afterbattle-v2.md` (E5 kroki 1–4, D9, D10, D17, D18, D19, §6, §7), `docs/design/README.md` (wiążący).
- **Makieta:** `docs/design/*.dc.html` to specyfikacja struktury, stylów i treści, nie kod do kopiowania. Kod powstaje od nowa w React 19, bez bibliotek UI i wykresów (§3 speca 20).

## 1. Zakres i granice

Robimy: nowy system wizualny, nawigację, ekrany Sesja, Czołgi/Czołg, Bitwa (w Historii), Cele, Historia, Ustawienia (z prywatnością), grafiki z gry w RAM, dotychczasowy zakres E5 krok 4 (moduły, ESLint, słownik błędów, teksty, dialogi, obsługa błędów, `session-ended`, `ts-rs`, testy) i dawny E6 (widok czołgu i wyzwania).

Nie robimy (z makiety lub poza biegiem):
- **Porównania z innymi graczami** na ekranie Czołg (tabele odniesienia §11.1 speca 20). W v2.0 to samo miejsce porównuje z **Twoją średnią na tym czołgu** (spec 20, E5 krok 3).
- **Podsumowania z odnośnikami** w ekranie Bitwa (§11.3). Miejsce w siatce zostaje puste (pusta kolumna siatki, bez tekstu zastępczego).
- **Celów tygodniowych i „Ten tydzień wobec poprzednich czterech”** z makiety `Cele.dc.html`: to nowe typy celów i raport tygodniowy (E7, poza biegiem; §3 speca 20 zakazuje nowych typów wyzwań). Ekran Cele w v2.0 pokazuje **istniejące wyzwania na 5 bitew na czołgu** w języku wizualnym makiety (karta celu, pasek postępu z `role="img"` i `aria-label`, zdanie „skąd ten cel”). Sekcja tygodniowa nie jest renderowana.
- Listy graczy składu (D17): nigdzie.

## 2. Inwentarz: obecne komponenty → nowe ekrany

| Obecnie (`src/`) | Linie | Docelowo |
|---|---|---|
| `App.tsx` (zakładki home/session/goals/history/privacy/settings, karty statusu, modal bitwy, ustawienia, prywatność) | 1134 | `app/App.tsx` (powłoka: pasek nawigacji, routing stanu, stopka, eventy), reszta rozbita na strony niżej |
| `HomeOverview.tsx` | 530 | `pages/SessionPage.tsx` (Sesja jako ekran startowy; stany pierwszego uruchomienia z `FirstRunGuide`) |
| `SessionInsightsUi.tsx`, `BaselineUi.tsx`, `BattlePaceUi.tsx`, `AttemptsConversionUi.tsx` | 215+771+147+368 | `components/session/*` (sygnały „Co wyróżnia tę sesję”, porównanie ze średnią, tempo, celność) — logika prezentacji przeniesiona, wygląd nowy |
| `HistoryBrowser.tsx` (lista bitew, przegląd czołgów, raport czołgu z trendem) | 787 | `pages/HistoryPage.tsx` (bitwy i sesje z filtrem), `pages/TanksPage.tsx` (lista czołgów + `TankDetail`) |
| `PerformanceTrends.tsx`, `TrendChart.tsx` | 120+218 | `components/charts/TrendChart.tsx` (ręczne SVG, maks. 200 punktów) |
| `GoalsPage.tsx`, `ChallengeUi.tsx` | 81+425 | `pages/GoalsPage.tsx`, `components/goals/*` |
| `BattleModal` (w `App.tsx`) | — | `pages/BattlePage.tsx` (pełny ekran w sekcji Historia, „Wróć”) |
| `ReleaseUi.tsx` (`AboutPanel`, `FirstRunGuide`, `FriendlyFailure`) | 274 | `pages/SettingsPage.tsx` (O aplikacji, prywatność, dane XVM, katalog, diagnostyka, raport przebudowy), `components/FirstRunGuide.tsx`, `components/FriendlyFailure.tsx` |
| `Ui.tsx` (`MetricIcon` itd.) | 343 | `components/ui/*` (Card, Stat, Button, Dialog, Icon, VehicleImage, MapImage, EmptyState, ErrorState) |
| `presentation.ts` | 228 | `lib/format.ts` (liczby, czas, procenty; `Intl` dla PL/EN) |
| `i18n.ts` | 1119 | `i18n/pl.ts`, `i18n/en.ts`, `i18n/errors.ts` (słownik kodów błędów), `i18n/index.ts` (jedno źródło języka, N8) |
| `api.ts`, `types.ts` | 393+752 | `api/client.ts`, `api/events.ts`; typy z `ts-rs` w `api/generated/` |
| `styles.css`, `alpha.css` | 2411+1525 | `styles/tokens.css` + `styles/base.css` + moduły CSS per komponent (zwykłe pliki `.css` importowane w komponentach); `alpha.css` usunięty |

## 3. System wizualny

**Jeden plik tokenów** `src/styles/tokens.css` (custom properties na `:root`), jedyne miejsce z wartościami kolorów. Test: w `src/**/*.{css,tsx}` poza `tokens.css` nie ma literałów `#rgb`/`#rrggbb`/`rgb(`/`rgba(`/`hsl(` (skrypt w teście Vitest).

| Token | Wartość (z README makiety) |
|---|---|
| `--bg` | `#0A101C`, tło z dwiema poświatami (`--glow-blue rgba(70,104,210,0.20)`, `--glow-orange rgba(240,138,60,0.09)`) |
| `--card-bg`, `--card-border`, `--radius-card` | `rgba(255,255,255,0.035)`, `rgba(255,255,255,0.07)`, 20 px |
| `--text`, `--text-2`, `--text-3` | `#EEF1F6`, `#9AA6B8`, `#8693A6` |
| `--accent`, `--accent-link`, `--accent-link-hover` | `#F08A3C`, `#F3A36B`, `#FFC08F` |
| `--team-enemy` | `#6F87B8` |
| `--focus` | obrys 2 px `--accent-link`, widoczny tylko przy `:focus-visible` |
| odstępy | `--space-1..8` = 4, 8, 12, 16, 20, 24, 28, 44 px |
| typografia | `--font-num: 'Barlow Semi Condensed'` (liczby 300, nagłówki 500), `--font-text: 'Inter'`; skala 13/14/15/18/36/104 px |

Zasady (spec 20, E5 krok 2): jeden akcent (pomarańcz), fiolet usunięty; bez pigułek, wersalików, nadtytułów, numeracji i kropek w tytułach; kolor nigdy jedynym nośnikiem („Wygrana”/„Porażka” słowami, kierunek zmiany słowem „więcej/mniej” i znakiem); poświata i pomarańczowa ramka tylko na jednym elemencie ekranu (karta główna). Cele kliknięć ≥ 44 px, kontrast ≥ 4,5:1 (test kontrastu par tokenów tekst/tło w Vitest, obliczony wzorem WCAG).

**Fonty lokalnie:** Barlow Semi Condensed 300/400/500/600 i Inter 400/500/600 jako `woff2` w `src/assets/fonts/` z `@font-face` w `tokens.css`, bez Google Fonts (CSP bez sieci). Pliki pobrane przez menedżer pakietów (`@fontsource/barlow-semi-condensed`, `@fontsource/inter`, podzbiory `latin` i `latin-ext` dla polskich znaków), skopiowane do repo; pakiety nie zostają w zależnościach. Licencja: SIL OFL 1.1 obu rodzin sprawdzona w plikach `LICENSE` pakietów, kopia w `src/assets/fonts/OFL.txt` z listą rodzin i wersji w `src/assets/fonts/README.md`. Obecne `BarlowCondensed-*.woff2` są usuwane.

## 4. Struktura frontendu (D9)

```
src/
  app/         App.tsx, Navigation.tsx, Footer.tsx, useAppEvents.ts, buildFlags.ts
  pages/       SessionPage, TanksPage (+TankDetail), GoalsPage, HistoryPage, BattlePage, MapsPage (zaślepka), SettingsPage
  components/  ui/, session/, tanks/, goals/, history/, battle/, charts/
  hooks/       useSnapshot, useCommand (ładowanie/błąd/dane), useGameImage, useDialog
  i18n/        pl.ts, en.ts, errors.ts, index.ts
  api/         client.ts, events.ts, generated/ (ts-rs)
  lib/         format.ts
  styles/      tokens.css, base.css
```

Nawigacja (spec 20, E5 krok 3): pionowy pasek, okrągłe ikony z podpisami: Sesja, Czołgi, Cele, Historia, Mapy, a na dole Ustawienia. Prawdziwe `<button>` z `aria-current="page"`. **Mapy:** flaga buildu `VITE_PUBLIC_BUILD` (`buildFlags.ts`); prywatny build: pozycja wyszarzona (`aria-disabled="true"`, podpowiedź „Pojawi się po rozbudowie analizy powtórek”), publiczny: brak pozycji. Test obu wariantów. Stopka: wersja aplikacji i „Dane WN8: XVM · <wersja>” (D3, raz; drugie miejsce: Ustawienia).

## 5. Ekrany: źródło każdej liczby i stany

Wspólne stany każdego ekranu: **ładowanie** (szkielet kart bez liczb, `aria-busy`), **pusto** (zdanie, co zrobić dalej), **błąd** (tekst ze słownika błędów + „Spróbuj ponownie”), **brak XVM** (WN8 nie jest pokazywane; jedno zdanie z odnośnikiem do Ustawień; pozostałe liczby działają, D7), **za mało bitew** (zdanie z liczbą bitew i progiem). Stan błędu nigdy nie jest cichy (koniec `.catch(()=>{})`, Q2/Q3).

### 5.1 Sesja (ekran startowy)

| Element | Źródło (IPC → pole) | Etap danych |
|---|---|---|
| Nagłówek „Dzisiejsza sesja”, „Od HH:MM, N bitew losowych, aktywna/zakończona” | `get_snapshot` → `activeSession.startedAt`, `.analyzedBattleCount`, `.status` (gdy brak aktywnej: ostatnia sesja z `recentSessions`) | E2 |
| Przycisk „Zakończ sesję” / „Rozpocznij sesję” | `end_session` / `start_session`; potwierdzenie końca z `session-ended` | E3 |
| Karta główna „Średnie obrażenia” + zdanie „O X% więcej/mniej niż Twoja średnia na tych samych czołgach (Y)” | `get_session_summary(id)` → `metrics.averageDamage`; `get_session_baseline_comparison(id)` → `vehicles[].damage` ważone liczbą bitew na czołgu (średnia odniesienia per czołg, D7) | E4 |
| Wygrane „a z n” | `metrics.victories`, `metrics.battlesWithKnownOutcome` (D8) | E1 |
| Przetrwanie „a z n” | `metrics.survived`, `metrics.survivalSampleSize` | E1 |
| Asysta „śr.” | `metrics.averageAssist` (radio + gąsienice, D8) | E1 |
| WN8 sesji (mała karta, tylko z XVM) | `get_session_summary` → `wn8.value`; brak XVM → karta nie istnieje | E4 |
| „Co wyróżnia tę sesję” | `get_session_baseline_comparison` → `insights.summaries[].strongestPositive/Negative/supportingSignals`, teksty z `evidenceTextKey` | E4 (M5, M10) |
| Tabela „Bitwy w tej sesji” (Wynik, Czołg z ikoną, Mapa z miniaturą, Obrażenia, Asysta, Godz.) | `get_session_summary` → `battles[]` (`outcome`, `tankName`, `wgTankId`, `mapName`, `mapKey`, `damage`, `assist`, `battleTime`); ikony `get_game_image` | E1, E5 |
| Tempo bitwy, celność i przebicia (poziom drugi, rozwijane) | `summary.battlePace`, `get_attempts_conversion_session_report` | E1 |
| Pasek importu „Odczyt powtórek: x / y” + „Zatrzymaj odczyt” | event `import-progress`, `cancel_import` | E3 |

Stany: brak skonfigurowanego folderu → `FirstRunGuide`; brak bitew w sesji → „Zagraj bitwę losową, a wyniki pojawią się tutaj same”; < 3 bitwy → karta główna bez porównania, zdanie „Porównanie pojawi się po 3 bitwach”; średnia odniesienia < 30 bitew → „Za mało bitew na tym czołgu do porównania (n z 30)”.

### 5.2 Czołgi i Czołg (dawny E6)

| Element | Źródło | Etap |
|---|---|---|
| Lista czołgów z ikoną i liczbą bitew, „Wszystkie N” | `get_vehicle_history_overview` → `vehicles[]` (`vehicleId`, `tankName`, `battles`) | E1 |
| Nagłówek: duża ikona, nazwa, „Klasa, poziom, N bitew, ostatnia …” | overview + katalog (`vehicleClass`, `tier`, E1b) | E1b |
| Cztery karty „Na tle Twojej średniej na tym czołgu”: Obrażenia, Asysta, Przetrwanie, Trafienia; zdanie „ostatnie 10 bitew wobec średniej ze 100” | `get_vehicle_baseline(id)` → okna `last10` i `last100` (`averageDamage`, `averageAssist`, `survivalRate`); trafienia: `get_vehicle_history_report(id)` (celność z konwersji) | E4 |
| Wykres „Obrażenia w czasie” (średnia z 10 bitew) | `get_performance_trends(id)` → punkty, maks. 200 (próbkowanie po stronie backendu) | E1 |
| Bitwy na tym czołgu, stronicowanie po 50 | `get_battle_history_page({vehicleId, pageSize: 50, cursor})` | E1 |
| „Gdzie idzie najlepiej / najsłabiej” | ta sama strona historii zgrupowana po `mapKey` w `get_vehicle_history_report` (średnie obrażenia i przetrwanie tylko istniejących metryk; mapa wchodzi przy ≥ 5 bitwach) | E5 (rozszerzenie raportu, bez nowej metryki) |
| Aktywne wyzwanie na tym czołgu | `get_active_challenge` | E1 |

Stany: czołg z 1 bitwą → „Za mało bitew na tym czołgu” (próg 10 dla kart, 2 punkty dla wykresu); brak wykresu przy 0 i 1 punkcie (komunikat), 200 punktów rysowane w całości. Agregaty widoku czołgu zgodne z `metrics()` na tych samych bitwach (test, dawny E6).

### 5.3 Bitwa (z Historii i z tabeli sesji)

| Element | Źródło | Etap |
|---|---|---|
| Tytuł „Wygrana/Porażka/Remis na <mapie>”, czołg, czas, długość bitwy | `get_battle_view(battleId)` → `battle` (istniejące DTO `BattleView` z kolumn `battles`: `outcome`, `mapName`, `tankName`, `battleTime`, `durationSeconds`) | E5 |
| Karta „Obrażenia” + „Najwięcej w drużynie, X% jej obrażeń” | `damage`; z `team_relative_facts`: `damage_greater_count == 0` → „najwięcej”, udział = `damage / team_damage_total` | E2 |
| Trafienia „a z b”, Przebicia, Zniszczone, Asysta, Wykrycia, Zablokowane | `direct_enemy_hits`, `shots`, `piercing_enemy_hits`, `frags`, `assist`, `spots`, `blocked` | E1 |
| Przetrwanie „Przeżyłeś, h z H HP” / „Zniszczony” | `survived`, `health_remaining`, `max_health` (v10) | E2 |
| „Drużyny”: obrażenia, wykrycia, asysta (radio + gąsienice), przetrwali — obie strony; „Obie drużyny X poziom, plutony: a / b” | ta sama komenda `get_battle_view(battleId)` → pole `teams` (DTO `{own, enemy}`) z `battle_team_stats`: `damage_total`, `spotted_total`, `assist_radio_total + assist_track_total`, `survivors`, `tier_min`, `tier_max`, `platoons`; **bez żadnych pól z `battle_roster`** (D17, test) | E5 |
| Tło z grafiką mapy | `get_game_image(map, mapKey)` | E5 |
| Podsumowanie | poza v2.0, miejsce w siatce | — |

Stany: bitwa częściowa lub nielosowa → karta „Ta bitwa nie liczy się do statystyk” (kod `battle_not_counted` / `battle_partial`), bez sekcji Drużyny; brak wiersza `battle_team_stats` → „Porównanie drużyn niedostępne dla tej bitwy”.

### 5.4 Cele

| Element | Źródło | Etap |
|---|---|---|
| Aktywne wyzwanie: tytuł celu słowami, „a z 5 bitew, b udanych”, pasek postępu z `aria-label`, zdanie „Cel to Twoja średnia … (wartość) …” | `get_active_challenge` (zamrożona średnia, `thresholdValue`, `requiredSuccesses`) | E1, E4 |
| Propozycje po sesji (karta z przyciskiem „Zacznij”) | `get_challenge_suggestions(sessionId)`, `start_vehicle_challenge` | E4 (D7: działają bez XVM) |
| Historia wyzwań | `get_challenge_history` | E1 |
| „Porzuć” w dialogu z Esc i fokusem | `abandon_active_challenge` | E5 |

Stany: brak aktywnego i propozycji → „Po sesji z co najmniej 3 bitwami na jednym czołgu pojawią się propozycje”; WR nigdy nie jest celem (D12).

### 5.5 Historia

| Element | Źródło | Etap |
|---|---|---|
| Przełącznik „Bitwy / Sesje” | — | — |
| Lista bitew z filtrem **czołg** i **mapa**, stronicowanie po 50 | `get_battle_history_page(filter)` — filtr rozszerzony o `mapKey` (zmiana parametru istniejącej komendy, bez nowej komendy) | E5 |
| Lista sesji (data, liczba bitew, śr. obrażenia, WN8 gdy XVM) | `get_snapshot.recentSessions`, `get_recent_sessions_wn8` | E3 |
| Klik bitwy → Bitwa; klik sesji → raport sesji (ekran Sesja w trybie „zakończona”) | `get_session_summary(id)` | E1 |
| Eksport sesji | `export_anonymous_session` | E1 |

Stany: pusta historia z importem w toku → pasek postępu i „Historia wypełnia się z powtórek”; filtr bez wyników → „Brak bitew dla tego filtra” + „Wyczyść filtr”.

### 5.6 Ustawienia (z prywatnością)

Sekcje: Folder gry (`detect_wot_installations`, `configure_local_source`, pauza/wznowienie), Import (start/stop, raport przebudowy z diagnostyki: przeliczono / nielosowe / bez pliku / konflikty), Dane WN8 (włącz XVM — zgoda, `set_xvm_enabled`; wersja danych, liczba pojazdów, pominięte wiersze M8, ostatnia próba i błąd; „Odśwież”), Katalog pojazdów (status, pokrycie tierów), Język, Limit bezczynności (U5), Prywatność (co zostaje na dysku, grafiki tylko w pamięci, eksport, „Usuń wszystkie dane” z wpisaniem „USUŃ”/„DELETE” zależnie od języka, U8), Diagnostyka (kopiuj raport), O aplikacji. Ten sam język wizualny co reszta (karty, tokeny).

## 6. Grafiki z gry (krok 1, D19, rozstrzygnięcia 5–6)

- **Rozpoznanie** (tylko odczyt `res/packages`, wynik tylko liczbami i nazwami katalogów w raporcie etapu): położenie ikon pojazdów i grafik map, format, rozmiary, metoda zapisu, mapowanie nazw na `typeCompDescr` i `mapKey`.
- **Wynik rozpoznania (2026-11-12):** ikony pojazdów w `gui-part1..4.pkg`, katalogi `gui/maps/icons/vehicle/` (duże, 1259 PNG, maks. 36 kB) i `gui/maps/icons/vehicle/contour/` (1261 PNG, maks. 6 kB), nazwy `<nacja>-<nazwa z katalogu>.png`; grafiki map w `gui/maps/icons/map/stats/` (58 PNG, maks. 325 kB, nazwy = `mapKey`). **Wszystkie te wpisy są `Stored`, 0 `Deflated`; DDS występuje tylko w `map/screen/`, którego nie używamy.** Wniosek: czytnik nie potrzebuje rozpakowania ani konwersji DDS (decyzja 6 niżej).
- **Czytnik (gdyby kiedyś był potrzebny `Deflated`):** rozszerzenie `vehicle_catalog_probe.rs` o wpisy `Deflated`: limit rozmiaru po rozpakowaniu i limit współczynnika (rozmiar po / przed) sprawdzane z nagłówka przed alokacją; strumieniowe rozpakowanie (`flate2` już w drzewie przez `zip`, bez nowej zależności runtime) z twardym przerwaniem po przekroczeniu limitu w trakcie; wyszukiwanie po stałej nazwie; nic na dysk. Niespełnienie warunków → twardy stop.
- **Konwersja:** PNG przekazywane bez zmian; DDS → PNG w pamięci tylko jeśli rozpoznanie pokaże DDS (dekoder w Ruście bez nowej zależności albo stop z decyzją autora).
- **Pamięć:** `GameImageCache` w RAM, LRU z limitem liczby (domyślnie 512) i rozmiaru (domyślnie 32 MiB), klucz `(odcisk wersji klienta, rodzaj, klucz)`; zmiana odcisku czyści pamięć; „Usuń wszystkie dane” czyści pamięć.
- **IPC:** komenda `get_game_image(kind: "vehicle" | "map", key)` zwracająca bajty PNG (`tauri::ipc::Response`) albo `null`; frontend tworzy `blob:` URL (CSP `img-src 'self' blob: data:`), bez nowych uprawnień sieciowych. Dopisana do §7 speca 20.
- **Zamiennik:** brak grafiki → neutralna ikona klasy pojazdu / nazwa mapy na gradiencie (komponenty `VehicleImage`, `MapImage`).
- **Pomiar startu:** czas od startu do pierwszego ekranu z grafikami (log w diagnostyce) — w raporcie etapu. > 2 s → stop (D19).
- **PRIVACY:** zdanie w `PRIVACY.md` i `docs/PRIVACY_PL.md`.

## 7. Krok 4: dotychczasowy zakres na nowym wyglądzie

- ESLint 9 (flat config) z `typescript-eslint` (`no-floating-promises`), `eslint-plugin-react-hooks`; skrypt `pnpm lint` (ESLint + `prettier --check`) jako bramka.
- Słownik błędów PL/EN dla wszystkich kodów `AppError` i statusów (≈31) w `i18n/errors.ts`, test kompletności: lista kodów generowana z Rusta (test w Ruście wypisuje warianty do pliku `src/api/generated/error-codes.json`), test TS sprawdza, że każdy ma tekst PL i EN bez `[a-z]+_[a-z_]+`.
- Teksty U4, U5, U8 („Procent wygranych”, „powtórka”, limit bezczynności z ustawień, „USUŃ”/„DELETE”).
- Dialogi: jeden komponent `Dialog` (`role="dialog"`, `aria-modal`, pułapka fokusu, Esc zamyka, powrót fokusu) — U7.
- `session-ended` z backendu zamiast `useRef` (A6).
- `ts-rs` (dev-dependency Rusta, D9) dla DTO z IPC; test kontraktu: nazwy z `generate_handler!` (parsowane z `lib.rs`) == nazwy w `api/client.ts`.
- Testy `GoalsPage`, `SessionPage` (dawny `HomeOverview`), `BattlePace` (T4).

## 8. Testy

- Rust: czytnik grafik (wpis `Deflated` odrzucony przed alokacją, limit rozmiaru wpisu, obcinanie paczki na każdej długości, nazwa spoza wzorca; mutacje ≥ 85%), pamięć grafik (LRU, unieważnienie po odcisku, brak plików na dysku w katalogu danych), `get_battle_view` (DTO bez pól `battle_roster` — test serializacji kluczy), filtr mapy w historii, grupowanie po mapach w raporcie czołgu, agregaty widoku czołgu == `metrics()`, `get_game_image` dla braku grafiki.
- Frontend (Vitest + Testing Library, syntetyczne dane): każdy ekran w stanach ładowanie / pusto / błąd / brak XVM / za mało bitew; nawigacja klawiaturą (Tab, Esc w dialogach); flaga Mapy (dwa warianty); brak tekstów `[a-z]+_[a-z_]+` w DOM wszystkich ekranów w PL i EN; brak kolorów spoza tokenów; brak `alpha.css`; kontrast tokenów; wykres 0 / 1 / 200 punktów; DTO Bitwy bez składu.
- Repo: test, że w drzewie nie ma grafik poza `src/assets/afterbattle-mark.svg`, fontami i `docs/status/screens/*.png` o rozmiarach zrzutów (lista dozwolona + typy plików).
- Zrzuty: `pnpm screens` uruchamia Vite z `VITE_MOCK_IPC=1` (moduł `api/mock.ts` z danymi syntetycznymi i zamiennikami grafik), zrzuty każdego ekranu w każdym stanie do `docs/status/screens/` przez `playwright-core` (dev-dependency, bez pobierania przeglądarek) sterujący systemowym Microsoft Edge w trybie headless. Bez Tauri.

## 9. Kolejność kroków (gałąź `v2/e5-ui-redesign`)

1. Rozpoznanie paczek (raport liczbowy, zrobione) → czytnik grafik `Stored` z limitami (TDD, mutacje) → `GameImageCache` + `get_game_image` + PRIVACY.
2. Tokeny, fonty lokalne, `base.css`, komponenty `ui/` (test tokenów i kontrastu).
3. Struktura katalogów, `api/` z `ts-rs`, test kontraktu IPC, `i18n/` z jednym źródłem języka i słownikiem błędów.
4. Powłoka: nawigacja (flaga Mapy), stopka, eventy (`session-ended`), routing.
5. Ekrany: Sesja → Czołgi/Czołg (z backendem grupowania po mapach) → Historia (filtr mapy) → Bitwa (`get_battle_view`) → Cele → Ustawienia.
6. Usunięcie starych plików (`alpha.css`, `styles.css`, stare komponenty), ESLint jako bramka.
7. Zrzuty ekranów z mockiem IPC, pomiar startu (autor), checkpoint E5 (spec 20) w raporcie etapu.

## 10. Decyzje wykonawcy w tym specu

1. Cele w v2.0 = istniejące wyzwania w nowym wyglądzie; karty tygodniowe z makiety czekają na E7 (§1).
2. „Gdzie idzie najlepiej/najsłabiej” liczone z istniejących metryk (śr. obrażenia, przetrwanie) zgrupowanych po mapie, próg 5 bitew na mapę; to układ danych, nie nowa metryka.
3. Bitwa jako pełny ekran zamiast modala (makieta), z „Wróć”.
4. Grafiki przez komendę zwracającą bajty, nie protokół zasobów (prostsze testy, brak zmian uprawnień).
6. **Bez obsługi `Deflated` i DDS w v2.0.** Spec 20 dopuszcza `Deflated` (rozstrzygnięcie 5), ale nie wymaga go; rozpoznanie pokazało, że żadna potrzebna grafika nie jest kompresowana. Czytnik nadal odrzuca wpis `Deflated` przed alokacją (test), a brak grafiki daje zamiennik. Mniej kodu, brak ryzyka bomby dekompresji; mutacje „limitów rozpakowania” (§6.3) nie mają czego dotyczyć, w zamian mutacje obejmują limity czytnika grafik (rozmiar wpisu, nazwa, `Stored`).
5. Nowe komendy E5 to dokładnie dwie przewidziane w §7 speca 20: `get_game_image` (grafika z RAM) i `get_battle_view` (porównanie drużyn z `battle_team_stats` razem z istniejącym `BattleView` tej bitwy, żeby ekran Bitwa działał także dla bitew spoza sesji). Nazwy dopisane do §7.
