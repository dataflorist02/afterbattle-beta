# Spec 29 — E9: dopracowanie do 1.0.0-beta.2 (ruch, marka, ekran ładowania)

- **Status: APPROVED (decyzje autora 2026-11-15).** Kontrakt dla etapów 0–5 biegu do `1.0.0-beta.2`.
- Rozwija: spec 25 (UI), spec 26 (E5-flare, reguły ruchu §7a). Nie zmienia backendu poza tym, co wymaga §2.
- Źródła decyzji: rozmowa architektoniczna 2026-11-15 i plansze na płótnie Claude Design „Afterbattle 2.0 — redesign” (rząd „Logo”: wariacje konturu, ekran ładowania, paleta).
- Pliki marki: `docs/design/brand/` (opis w `docs/design/brand/README.md`).

## 0. Zakres i kolejność

| Etap | Gałąź | Sekcja |
|---|---|---|
| 0 | `v2/e8-ui-fix-regress` | §1 Regresja ruchu w Czołgach i Historii |
| 1 | `v2/beta2-live-refresh` | §2 Odświeżanie na żywo |
| 2 | `v2/e9-brand` | §3 Paleta, §4 Logo, §7 Pasek przewijania |
| 3 | `v2/e9-loading` | §5 Ekran ładowania |
| 4 | `v2/e9-motion` | §6 Ruch: nowa bitwa i liczby |
| 5 | (wydanie) | §8 Wersja i nazwa, §9 Akceptacja |

Poza zakresem: podsumowania AI (spec 28), moonshot, jasny motyw aplikacji, dźwięki, zmiany w parserze i bazie.

## 1. Regresja ruchu w Czołgach i Historii (etap 0)

Poprawka E8-ui-fix (`6807a75`) jest w wydaniu `v1.0.0-beta.1` (sprawdzone przez autora 2026-11-15: `merge-base --is-ancestor` = tak, build 19:50 po scaleniu 18:02), a w zainstalowanej aplikacji nie działa. Ograniczony ruch w Windows wyłączony.

**Objawy (autor):** Czołgi: przy wyborze czołgu miga prawa strona (panel szczegółów: bitwy, kafelki, wykres). Historia: fala kończy się mniej więcej tam gdzie przed poprawką.

**Ustalenia z kodu (architekt, 2026-11-15), do potwierdzenia przez wykonawcę:**
1. `TanksPage.tsx`: `<TankDetail key={current.vehicleId} …>` montuje panel od nowa przy każdym wyborze czołgu. Nowy montaż resetuje oba `useAsync` w panelu do `{status:"loading"}`, więc panel na chwilę pokazuje stan ładowania, potem treść. To jest miganie, nie animacja; testy w jsdom go nie widzą, bo dane z atrapy przychodzą w tej samej klatce.
2. `useAsync.ts`: efekt przy zmianie `deps` robi `setState({status:"loading"})` przed `reload()`. `reload` zachowuje poprzednie dane, ale efekt nie. Każda zmiana zależności daje klatkę ładowania.
3. Kafelki map w panelu doczytują grafiki przez `useGameImage` po wyświetleniu, więc obraz „wskakuje” po animacji.
4. Historia: wiersze są pogrupowane w dni (`h3.day-header` + `ul.hrow-list` na dzień). `measuredWave` liczy krok z odległości dwóch pierwszych `li` i wysokość od pierwszego wiersza do dołu okna; nagłówki dni, fonty wczytane po pomiarze (`font-display: swap`) i obrazy wierszy zmieniają układ po pomiarze. Wykonawca ustala w prawdziwym układzie, który z tych czynników ucina falę.

**Wymagania:**
- Panel szczegółów czołgu nie montuje się od nowa przy zmianie czołgu (bez `key` na czołgu albo z zachowaniem stanu). Przy zmianie czołgu stare dane zostają na ekranie, dopóki nie przyjdą nowe (stan „odświeżanie”, nie „ładowanie”); wymiana treści bez animacji wejścia.
- `useAsync`: tryb zachowujący poprzednie dane przy zmianie zależności (stan `ready` zostaje do nadejścia nowych danych). Domyślne zachowanie zmieniaj tylko, jeśli żaden ekran nie zależy od klatki ładowania; opisz decyzję w STATUS.
- Kafelki map: grafika nie zmienia rozmiaru kafelka po wczytaniu (stałe proporcje), pojawia się przez `opacity`.
- Historia: fala obejmuje wiersze widoczne przy pierwszym wyświetleniu w prawdziwym układzie. Pomiar po `document.fonts.ready` i pierwszym layoucie (`requestAnimationFrame`); nagłówki dni nie zaniżają liczby; przy pomiarze zerowym wartość domyślna 14. Górny limit `WAVE_MAX_ROWS` (20) bez zmian.
- Testy: zmiana wybranego czołgu nie pokazuje stanu ładowania ani animacji wejścia; dane przychodzące w kolejnej klatce (atrapa z opóźnieniem); pomiar przy zerowych wymiarach; nagłówki dni w środku fali.

## 2. Odświeżanie na żywo (etap 1)

**Błąd (autor, 14.11):** po nowej bitwie ekran Sesja pokazuje stare dane; trzeba wyjść i wejść.

**Stan kodu:** `App.tsx` subskrybuje `import-progress`, `data-changed`, `session-ended` i przeładowuje snapshot. `SessionPage` ma już `newBattleIds`, `shouldCountUp`, `useCountUp`, a `SessionStory` klasę `tile-new` (E5-flare). Te animacje odpalą się dopiero, gdy dane Sesji naprawdę się odświeżą.

**Wymagania:**
- Wykonawca ustala przyczynę (czy backend emituje zdarzenie po imporcie pojedynczej nowej bitwy w trakcie sesji; czy strony odświeżają swoje dane po nowym snapshocie, czy tylko przy montażu) i opisuje ją w STATUS.
- Jeden mechanizm dla Sesji, Historii, Czołgów, Celów i Bitwy: dane strony zależą od wersji danych z backendu (np. licznik zmian w snapshocie), a nie od ponownego wejścia.
- Odświeżenie zachowuje poprzednie dane do nadejścia nowych (§1), nie restartuje animacji wejścia, nie przewija listy, nie zmienia wybranego czołgu ani filtrów.
- Testy: zdarzenie z backendu → nowa bitwa widoczna na Sesji i w Historii bez ponownego wejścia; brak klatki ładowania.

## 3. Paleta (etap 2)

Jedno źródło: `src/styles/tokens.css` (spec 25 §3 bez zmian). **Istniejących nazw nie zmieniamy** (`--accent`, `--accent-soft`, `--text-2` itd. zostają); dodajemy tylko brakujące tokeny. Wartości zgodne z planszą „Paleta kolorów”.

| Token (nowy) | Wartość | Użycie |
|---|---|---|
| `--bg-deep` | `#070b14` | tło ekranu ładowania |
| `--surface` | `#0d1422` | pasek tytułu, panele (jeśli dziś inny literał) |
| `--navy` | `#16223a` | płytka ikony |
| `--navy-hi` | `#1e3156` | środek winiety ikony |
| `--steel-line` | `#8b97ab` | kontur znaku, zimny kontur ekranu ładowania |
| `--ember-lo` | `#e0661f` | gradient żaru |
| `--ember-deep` | `#9a3a1a` | początek gradientu żaru |
| `--ember-white` | `#ffebd0` | najgorętszy punkt żaru |
| `--up` | `var(--accent-soft)` | liczba rośnie |
| `--down` | `#6e8fc6` | liczba spada (kontrast 5,8:1 do `--bg`, ciemniejszy od `--up`) |
| `--scroll-thumb` | `rgba(139, 151, 171, 0.35)` | pasek przewijania (§7) |
| `--scroll-thumb-hover` | `rgba(139, 151, 171, 0.6)` | jw., najechanie |
| `--scroll-thumb-active` | `var(--accent)` | jw., przeciąganie |

Zasady: pomarańcz to jedyny akcent, najwyżej jedna rzecz na ekranie przyciąga wzrok; tekst w stali, pomarańczowy tekst tylko dla linków i jednej wyróżnionej liczby; gradient żaru tylko w znaku, na ekranie ładowania i w postępie; porażka bez czerwieni (bez zmian). `--danger` zostaje dla błędów.

Test `tokens.test.ts`: nowe tokeny istnieją; żaden komponent nie używa literału koloru (jak dziś).

## 4. Logo (etap 2)

Zatwierdzona rodzina (wariacja 1 konturu): litera A jako kontur z poprzeczką w kształcie znaku wzrostu; kontur stalowy, rozgrzana tylko poprzeczka. Oryginalne, bez motywów z gry.

| Plik w `docs/design/brand/` | Użycie |
|---|---|
| `afterbattle-icon.svg`, `afterbattle-icon-1024.png` | ikona aplikacji: kontur na granatowej płytce, linia gruba (przetrwa 16 px). Źródło dla `pnpm tauri icon docs/design/brand/afterbattle-icon-1024.png` |
| `afterbattle-mark.svg` | znak na ciemne tło od ok. 48 px (README, ekran „O programie”) |
| `afterbattle-mark-small.svg` | znak poniżej 48 px i druk jednobarwny |
| `afterbattle-mark-light.svg` | znak na jasne tło |
| `afterbattle-mark-dark-192.png`, `afterbattle-mark-light-192.png` | README (`<picture>` z `prefers-color-scheme`) |
| `afterbattle-loading-reference.svg` | wzorzec ekranu ładowania (§5), zamrożony na 62% |

**Wymagania:**
- `pnpm tauri icon` z pliku 1024 nadpisuje `src-tauri/icons/` (dziś zielona tarcza z czasów alfy); sprawdzić listę plików względem `tauri.conf.json`, usunąć nieużywane ikony mobilne tylko jeśli konfiguracja ich nie wymaga. Stary `src-tauri/icons/icon.svg` zastąpiony nowym.
- Pasek nawigacji (`.rail-logo`, dziś litera „A” na pomarańczowym kwadracie): ikona aplikacji jako inline SVG (40 px), kolory z tokenów, `aria-hidden`.
- Pasek tytułu: ikona 16 px przed tytułem (z tego samego komponentu SVG).
- Znak jako jeden komponent React (`src/components/BrandMark.tsx`) z wariantami `icon`, `mark`, `mark-small`; kształty jedno źródło (współrzędne z plików marki), żeby ekran ładowania (§5) używał tych samych ścieżek.
- README: logo przez `<picture>` (ciemne i jasne), bez obrazów z gry.
- Grafiki marki trafiają do migawki publicznej (nie są na liście wyłączeń).

## 5. Ekran ładowania (etap 3)

**Wygląd (plansza „Ekran ładowania: kontur, który się rozżarza”):** na tle `--bg-deep` tylko kontur znaku na środku, rozmiar `clamp(160px, 22vmin, 280px)`. Bez napisu, bez osobnego paska, bez płytki. Zimny kontur: `--steel-line` z przezroczystością 0,38, linia 0,9 (w jednostkach viewBox 64). Rozgrzana część: ta sama ścieżka, gradient żaru (`--ember-white` u podstawy → `--accent-soft`/`#ffa556` → `--ember-lo` → `--ember-deep` u szczytu), linia 1,1, lekka poświata (rozmycie ok. 1,1 jednostki). Postęp = wysokość rozgrzanej części od podstawy (y 57) do szczytu (y 7), przez maskę z `transform: scaleY(p)` (tylko transform).

**Kiedy pokazywać:**
1. Start aplikacji, gdy pierwszy snapshot nie jest gotowy po 400 ms (zimny start). Przy szybszym starcie ekran się nie pojawia (D19: ≤ 2 s do pierwszego ekranu).
2. Pierwszy import przy pustej bazie (0 bitew), dopóki pierwsza paczka nie da czegoś do pokazania.
Nie pokazywać przy imporcie w tle, gdy są dane: wtedy zostaje `ImportBar` (bez zmian). Przebudowa z migracji v9→v10 działa w tle (E2) i też nie blokuje okna.

**Postęp:** z prawdziwych danych: `import-progress` (`done/total`) albo etapy startu, jeśli backend je podaje. Bez danych o postępie (zimny start): stan nieokreślony = żar tylko u podstawy (ok. 8%) i spokojne pulsowanie jasności (`opacity` 0,7–1, 1,6 s). Zakaz sztucznego postępu z zegara.

**Ruch:** zmiana postępu przejściem 300 ms `--ease-out`; po 100% przytrzymanie 150 ms i zanik ekranu 200 ms (`opacity`). Przy `prefers-reduced-motion`: bez przejść i pulsowania, wysokość od razu równa postępowi.

**Dostępność:** `role="progressbar"`, `aria-label` („Wczytywanie” / „Loading”), `aria-valuemin/max/now` przy znanym postępie; przy nieokreślonym bez `aria-valuenow`.

**Testy:** reguły pokazywania (próg 400 ms, pusta baza, import w tle bez ekranu); mapowanie postępu na skalę (0 → minimalna, 1 → pełna, wartości poza zakresem przycięte); stan nieokreślony; ograniczony ruch; zanik po 100%.

## 6. Ruch: nowa bitwa i liczby (etap 4)

Opiera się na istniejących mechanizmach z `src/lib/motion.ts` (E5-flare): `newBattleIds`, `shouldCountUp`, `useCountUp`, `COUNT_MS = 600`, klasy `tile-new` i `tile-flash`. Po §2 zaczną działać; ten etap je rozszerza i dopracowuje.

- **Nowa bitwa w Sesji:** kafelek w pasku „bitwa po bitwie” wjeżdża (`tile-new`, bez zmian), a pozostałe kafelki przesuwają się płynnie (technika FLIP: pomiar przed i po, `transform`, `--motion-enter`). Tylko dla bitwy, która właśnie doszła do trwającej sesji; nigdy przy wejściu na ekran.
- **Nowa bitwa w Historii** (gdy ekran jest otwarty): nowy wiersz na górze wchodzi animacją `row-in`, reszta listy przesuwa się bez skoku; pozycja przewinięcia zachowana, jeśli użytkownik nie jest na górze listy.
- **Liczby w Sesji:** wszystkie liczby sekcji podsumowania (dziś tylko średnie obrażenia) liczą od starej do nowej wartości (`countValue`, `COUNT_MS`), cyfry o stałej szerokości (`font-variant-numeric: tabular-nums`). Liczba, która wzrosła: nakładka w kolorze `--up`, która spadła: `--down`; nakładka widoczna 1 s, potem zanika przez `opacity` w 400 ms (zgodnie z §7a: rusza się tylko `transform` i `opacity`, kolor tekstu się nie animuje). Brak zmiany wartości = brak ruchu.
- **Ograniczony ruch:** od razu nowa wartość i nowy układ, bez nakładki koloru.
- **Testy:** kierunek zmiany (wzrost, spadek, bez zmiany, `null`); formatowanie w trakcie liczenia (bez skoków szerokości); „tylko nowa bitwa” (wejście na ekran, inna sesja, zakończona sesja); FLIP bez animacji przy ograniczonym ruchu.

## 7. Pasek przewijania (etap 2)

„Afterbattle'owy” pasek dla wszystkich przewijanych obszarów (okno, `.content`, listy boczne, tabele):
- standardowe `scrollbar-width: thin` i `scrollbar-color: var(--scroll-thumb) transparent` (WebView2/Chromium);
- najechanie na przewijany obszar: `--scroll-thumb-hover`; wciśnięty przycisk myszy w obszarze (`:active`, przybliżenie przeciągania): `--scroll-thumb-active`;
- bez `::-webkit-scrollbar` (Chromium ignoruje go przy ustawionym `scrollbar-color`), bez bibliotek JS i bez własnego przewijania; klawiatura, kółko i dotyk bez zmian.

## 8. Wersja i nazwa (etap 5)

- Wersja `1.0.0-beta.2` w `src-tauri/Cargo.toml` (jedyne źródło, test zgodności z `package.json` bez zmian).
- Tytuł okna i pasek tytułu: „Afterbattle beta N”, gdzie N to numer wydania próbnego z wersji (`1.0.0-beta.2` → „Afterbattle beta 2”); wersja stabilna → „Afterbattle 1.0.0”. Reguła jako czysta funkcja z testem (zastępuje dzisiejsze „Afterbattle beta 1.0”).
- Kanał „Public beta” bez zmian. Tag `v1.0.0-beta.2`, wydanie jako pre-release.

## 9. Akceptacja

- Kontrole z STATUS §0 zielone; golden bez zmian (2013 bitew, `1703f8dbeb68775e` przy `AFTERBATTLE_GOLDEN_UNTIL=2026-11-11T21:00`).
- Mutacje ≥ 85% dla nowej i zmienionej logiki (`motion.ts`, logika ekranu ładowania, reguła nazwy wersji, mechanizm odświeżania).
- Testy z §1–§8 istnieją i przechodzą; `pnpm test` przed i po etapie 2 (tylko tokeny i style) daje identyczny wynik poza nowymi testami.
- Migawka publiczna zaktualizowana skryptem (tryb aktualizacji), pliki marki w środku, skany czyste.
- **Test ręczny autora (dopisany do listy):** odświeżanie Sesji po bitwie bez wychodzenia; nowa bitwa wjeżdża, liczby liczą z krótkim kolorem kierunku; Czołgi bez migania prawej strony przy wyborze czołgu; fala w Historii do dołu okna (2560×1440 i okno zmniejszone); ekran ładowania tylko przy zimnym starcie lub pustej bazie, żar rośnie z postępem; pasek przewijania; nowa ikona w pasku zadań, zasobniku, Alt+Tab i liście aplikacji; to samo z włączonym ograniczonym ruchem w Windows.
