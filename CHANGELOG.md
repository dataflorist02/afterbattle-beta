# Changelog

## Unreleased

## 1.0.0-rc.2

Bezpieczeństwo: aktualizacja zip. Druga wersja kandydata na 1.0 („Afterbattle 1.0 RC 2”), bez zmian w tym, co widzi gracz; zegar do 1.0.0 biegnie dalej od rc.1. Instalator bez podpisu Authenticode (pre-release). / Security: zip update. Second build of the 1.0 release candidate ("Afterbattle 1.0 RC 2"), nothing changes for the player; the clock to 1.0.0 keeps running from rc.1. Installer without an Authenticode signature (pre-release).

### Internal

- Security: the updater no longer pulls `zip` 4.6.1 (RUSTSEC-2026-0367, -0368, -0369, -0429, -0545, -0548). `tauri-plugin-updater` 2.12.0, the latest 2.x, keeps TLS and the system proxy without its `zip` feature, which only unpacks zipped Windows updates to disk; our updates are the signed NSIS installer itself. Our own package reader uses `zip` 9.7.0 (was 9.5.0) and reads entries into memory. A test keeps every locked `zip` at 9.0.5 or newer; `cargo audit`: 0 vulnerabilities.
- Clippy clean on CI's Rust 1.89.0 (a constant `is_empty()` in a test) and on stable 1.100 (`as_chunks`, `?`); MSRV stays 1.89.
- The update public key was rotated before the first release (key id `112017D205DB8B09`); the old key never signed a release.
- The public snapshot check for names of excluded files also runs in `pnpm test`, over the tracked files the snapshot keeps.

## 1.0.0-rc.1

Kandydat na wersję 1.0 („Afterbattle 1.0 RC”): Przegląd jako pierwszy ekran, porównanie z graczami z Twoich bitew na ekranie Czołg, aktualizacje za zgodą i ostatnie szlify interfejsu. Instalator bez podpisu Authenticode (pre-release). / Release candidate for 1.0 ("Afterbattle 1.0 RC"): Overview as the first screen, a comparison with the players of your battles on the Tank screen, updates with consent, and the last interface polish. Installer without an Authenticode signature (pre-release).

### Po polsku — co się zmienia dla gracza

- **Przegląd.** Nowy pierwszy ekran: ostatnia sesja na całą szerokość z mapą najlepszej bitwy w tle, średnimi obrażeniami, wygranymi, WN8 sesji i pierwszym zdaniem podsumowania; trendy z ostatnich 100 bitew z małymi wykresami (obrażenia, WN8, wygrane — tylko kontekst); cele tygodnia; najlepsze czołgi i Twoja pozycja na tle graczy. Bez trwającej sesji aplikacja startuje na Przeglądzie, w trakcie sesji na Sesji.
- **Na tle graczy z Twoich bitew (ekran Czołg).** Dla tieru i klasy wybranego czołgu: rozkład wyników innych graczy z Twoich bitew (środkowa połowa, mediana, 90%) i Twoja mediana jako percentyl, dla obrażeń, asysty, wykryć, zniszczeń, zablokowanych i czasu życia; celność tylko jako tło. Liczone na tym komputerze z zapisanych składów, bez Ciebie i Twojego plutonu; wartości od 200 wyników, Twoja mediana od 10 bitew.
- **Najlepsze czołgi.** Na Przeglądzie trzy czołgi o najwyższym percentylu mediany obrażeń w swojej grupie tier × klasa (od 10 bitew), np. „88. w ciężkich X”, i najczęściej grane w jednej linii.
- **Aktualizacje za zgodą.** Przy pierwszym uruchomieniu (i raz po aktualizacji z bety) pytanie „Sprawdzać aktualizacje przy starcie?”. Po „tak” aplikacja raz przy starcie pyta GitHuba o numer najnowszej wersji i pokazuje „Dostępna wersja X” z notatkami; instaluje dopiero po kliknięciu „Zainstaluj i uruchom ponownie”, tylko z podpisem autora. Przełącznik w Ustawieniach → Prywatność i dane; adres strony wydań w „O aplikacji”. Wydania oznaczone jako pre-release (jak to RC) nie są proponowane automatycznie.
- **Szybsza reakcja na kliknięcie:** przyciski, kafelki i wiersze reagują na wciśnięcie od razu (wcześniej po ok. 60–100 ms); ekrany wchodzą krótszym przenikaniem z przesunięciem 8 px; szkielet ładowania pojawia się tylko, gdy dane idą dłużej niż 0,3 s; stopka nie przeskakuje przy wczytaniu ekranu.
- **Krótsze podsumowania:** karta podsumowania sesji, bitwy i tygodnia pokazuje najpierw pierwsze zdanie; reszta po kliknięciu „Pokaż więcej (N)”. Wybór jest zapamiętany dla wszystkich ekranów.
- **Nowy układ Ustawień:** grupy Gra, Ty, Dane, Prywatność i dane, O aplikacji z listą odnośników po lewej; przy każdej opcji jedno zdanie, co robi. Język, forma zwracania się i limit bezczynności zapisują się od razu (komunikat „Zapisano.”), bez przycisku „Zapisz”. Pobieranie WN8 z XVM to przełącznik.
- **Puste ekrany mówią, co zrobić:** Sesja, Czołgi i Historia mają jedno zdanie z następnym krokiem; pusta Historia ma przycisk „Odczytaj wszystkie powtórki”. Odstępy na wszystkich ekranach wyrównane do siatki 4 px.

### In English — what changes for the player

- **Overview.** A new first screen: the last session on the whole width with the best battle's map, average damage, wins, session WN8 and the first summary sentence; trends of the last 100 battles with small charts; the week's goals; the best tanks and your position among the players. Without a running session the app starts on Overview, during a session on Session.
- **Against the players in your battles (Tank screen).** For the selected tank's tier and class: the distribution of the other players in your battles (middle half, median, 90%) and your median as a percentile, for damage, assist, spotting, kills, blocked damage and lifetime; accuracy as background only. Counted on this computer from the stored rosters, without you and your platoon; values from 200 results, your median from 10 battles.
- **Best tanks.** On Overview, the three tanks with the highest percentile of median damage in their tier × class group (from 10 battles), and the most played in one line.
- **Updates with consent.** The first start (and once after updating from a beta) asks whether to check for updates at start. With a yes the app asks GitHub once per start for the newest version number and shows "Version X is available" with notes; it installs only after a click, and only with the author's signature. A switch in Settings → Privacy and data; the releases page address in About. Releases marked as pre-release (like this RC) are not offered automatically.
- **Snappier response to clicks:** buttons, tiles and rows react to a press at once (before: after about 60–100 ms); screens enter with a shorter fade and an 8 px rise; a loading skeleton shows only when data takes longer than 0.3 s; the footer no longer jumps while a screen loads.
- **Shorter summaries:** the session, battle and week summary cards show the first sentence; the rest opens with "Pokaż więcej (N)" (Show more). The choice is remembered for every screen.
- **New Settings layout:** the groups Game, You, Data, Privacy and data, About, with a list of links on the left; one sentence on what each option does. Language, address form and the inactivity limit save at once ("Saved."), without a Save button. WN8 from XVM is a switch.
- **Empty screens say what to do:** Session, Tanks and History have one sentence with the next step; the empty History has "Odczytaj wszystkie powtórki" (read all battle files). Spacing on every screen follows the 4 px grid.

### Internal

- A-1 reference tables (`get_reference_tables`): percentiles per tier × class from the stored rosters at read time, no derived values stored, no row data or identifiers in the answer; the player's own tanks with their damage percentile for the Overview. Mutations of `baseline.rs` window and reference functions: 96.2%.
- Updates: `tauri-plugin-updater` 2, all calls from Rust behind the consent gate (no updater or process permission in the window); the author's minisign public key in `tauri.conf.json`; `scripts/release.ps1` signs the update artifacts and writes `latest.json` only with `TAURI_SIGNING_PRIVATE_KEY` set, otherwise it refuses. PRIVACY (EN/PL) and THREAT_MODEL describe the new connection.
- A 1.0.0-beta.4 database opens in the RC unchanged (schema 10, no migration).
- Motion times and curves only from tokens (`tokens.css`), guarded by a test over CSS and TSX; `tools/screens/timing.mjs` measures the spec 31 §2 table (press, screen change, CLS) in a real browser.
- `tools/screens/recap.mjs` captures the summary cards closed and open and checks that opening moves nothing above the card.
- `tools/screens/compare.mjs` shoots every screen of any checkout at 1280 and 1920 px (before and after a change); Settings options are covered by characterization tests written before the new layout.
- `tools/screens/review.mjs` audits the rendered screens (text contrast, focus rings, Tab reach and order, spacing grid); a test keeps spacing on the 4 px grid and radii on tokens.
- The public snapshot tests that start PowerShell have an explicit 30 s timeout, because a cold start of pwsh on the CI runner exceeded the default 5 s.

## 1.0.0-beta.4

Czwarta publiczna beta: podsumowania sesji, bitwy i tygodnia prostą polszczyzną, składane z Twoich liczb według stałych reguł (bez modelu AI), i ustawienie formy zwracania się. Instalator bez podpisu (pre-release). / Fourth public beta: plain-Polish summaries of the session, battle and week, put together from your numbers by fixed rules (no AI model), and an address setting. Unsigned installer (pre-release).

### Po polsku — co się zmienia dla gracza

- **Podsumowanie sesji.** Pod liczbami sesji karta z 3–5 zdaniami: liczba bitew i wygrane wobec Twoich ostatnich 100 bitew, obrażenia wobec Twojej średniej na tych czołgach, najlepsza bitwa z mapą, WN8 wobec 30 dni przed sesją (z danymi XVM), przetrwanie (gdy różni się o 5 punktów lub więcej) i najczęściej grany czołg.
- **Podsumowanie bitwy.** Na ekranie policzonej bitwy: wynik z mapą, czołgiem i przetrwaniem, obrażenia wobec Twojej średniej na tym czołgu, a potem do trzech kolejnych faktów (zniszczone pojazdy z miejscem w drużynie, obrażenia sojuszników dzięki Twojemu wykrywaniu i zrywaniu gąsienic, pancerz, wykrycia, udział w obrażeniach drużyny, strzały) według stałych progów.
- **Podsumowanie tygodnia.** Na ekranie Celów, pod nagłówkiem tygodnia: liczba bitew i wygrane (tylko kontekst, nigdy cel), wskaźniki, które odbiegają od zwykłego poziomu o 5 punktów lub więcej, stan celów tygodnia, poprzedni tydzień i wyzwania na 5 bitew. Przy mniej niż 15 bitwach jedno zdanie: za mało bitew w tym tygodniu.
- **Odnośniki do liczb.** Każde zdanie kończy się odnośnikiem (F1, F2, …). Najechanie albo fokus podświetla tę liczbę na ekranie; gdy liczby nie ma na ekranie, podpowiedź mówi, czego dotyczy zdanie.
- **Bez oceniania i bez AI.** Teksty nie oceniają gry („dobrze”, „słabo”), nie podają przyczyn i nie mówią o „wpływie”. Powstają od razu z gotowych fraz, w kilku wariantach, żeby kolejne sesje nie brzmiały tak samo; ta sama sesja zawsze ma ten sam tekst. Nic nie jest zapisywane ani wysyłane.
- **Jak się do Ciebie zwracać.** Przy pierwszym uruchomieniu pytanie: on, ona albo bez formy; zmienisz to w Ustawieniach. Forma męska („zadałeś”), żeńska („zadałaś”) albo neutralna, bez czasownika („Wygrana na mapie …, czołg przetrwał do końca.”, „Coraz lepiej z każdą bitwą”) — domyślna, dopóki nie wybierzesz. Obejmuje podsumowania i dotychczasowe teksty w 2. osobie.
- **Liczby w zdaniach.** Jeden fakt w zdaniu. Zdanie z porównaniem ma zawsze ten sam układ: wartość, różnica, odniesienie w nawiasie („Średnio 3 414 obrażeń, o 13% więcej niż zwykle na tych czołgach (3 021).”, „Wygrałeś 7 z 12 bitew, czyli 58%, o 7 pkt proc. więcej niż w ostatnich 100 bitwach (51%).”). Zero: „0 obrażeń; zwykle na tym czołgu 2 900.”, „Czołg nie przetrwał do końca żadnej bitwy sesji; zwykle 38%.”. Podsumowanie bitwy zaczyna się od wyniku („Przegrana na Test Valley, czołg Synth 44 M zniszczony.”). Najczęściej grany czołg pojawia się tylko, gdy ma ponad połowę bitew sesji („Cała sesja na czołgu X.” przy wszystkich).
- **Tylko po polsku.** W wersji angielskiej karty podsumowań są ukryte.

### In English — what changes for the player

- **Summaries in Polish.** The Session, Battle and Goals screens show a short summary card in plain Polish: 3–5 sentences (one sentence for a week under 15 battles), each with a reference (F1, F2, …) that highlights its number on the screen. The text is built from your numbers by fixed rules and ready phrases — no AI model, no network, nothing stored. The cards are hidden in the English version.
- **Address setting.** The first run asks how the Polish texts should address you (he, she or no form); Settings keep the same choice. The neutral form, the default until you answer, uses no verb ("Wygrana na …, czołg przetrwał do końca."). It also covers the existing second-person texts.
- **Numbers in sentences.** One fact per sentence; every comparison reads value, difference, reference in brackets, with one name per reference; a zero reads "<zero>; zwykle <reference>"; the battle summary opens with the result; the most played tank appears only above half of the session's battles.

### Summaries (technical)

- `src-tauri/src/recap.rs`: the facts of each summary (spec 30 §4) with the selection rules and thresholds as constants; `get_recap(kind, id)` returns facts only (typed with ts-rs), computed on request like the weekly report. Kept facts are numbered in text order; the seed is FNV-1a of the session id, the battle's arena id or the week's start.
- `src/lib/recap/`: Polish phrases with 2–4 variants, numeral agreement from `Intl.PluralRules`, numbers from `format.ts`, the verb form from a stem (`-łeś`, `-łaś`; the neutral form skips variants with a verb, no impersonal `-no`/`-to`), deterministic variant choice without two sentences starting with the same word.
- Invariant tests (spec 30 §7) on 100 sessions, 200 battles and 50 weeks of synthetic data, every variant in all three forms: every number from a cited fact, directions with their sign, no contradiction with text facts, every word in the SJP.PL list or a game term (`WN8`, `XP`), no repeated or dropped fact, no judging words, 3–5 sentences of at most 25 words.
- Word list `src/lib/recap/words.pl.txt` (234 forms) from SJP.PL "odmiany słów" 2026-11-01 under CC BY 4.0 (NOTICE); `scripts/check-recap-words.py` checks it against a local `odm.txt` (SHA-256 of the edition), CI never downloads it.
- No new dependency, permission or network access.

### Internal (since 1.0.0-beta.3)

- **Internal:** spec 30 approved with the author's decisions of 2026-11-16; spec 20 §11.6 lists the tank lifetime field for the next schema change.
- **Internal:** mutation rule of 2026-11-16 (spec 20 §6.3): mutations only for decision logic with a function filter, threshold 85%, once before a release, in the background; phrases and variants are covered by the invariant tests.
- **Internal:** research notes of the local AI summaries study, iterations 3 and 4 (facts as ready phrases, fact selection rules, a dictionary check, an impersonal style, a template baseline); they stay in the private repository.
- **Internal:** spec 28 (local AI summaries) records the author's decision to postpone the language model, with the numbers behind it, and two checker rules for a possible return: a Polish dictionary (SJP.PL) and rejecting sentences that contradict a text fact.

## 1.0.0-beta.3

Trzecia publiczna beta: poprawka zgłoszenia testerów z 2026-11-16 (przy świeżej instalacji aplikacja nie znajdowała folderu gry), instalator bez podpisu (pre-release). / Third public beta: a fix for the testers' report of 2026-11-16 (on a fresh install the app did not find the game folder), unsigned installer (pre-release).

### Po polsku — co się zmienia dla gracza

- **Gra na dowolnym dysku.** „Znajdź grę” szuka World of Tanks we wpisach instalacji w rejestrze Windows, w typowych folderach na wszystkich dyskach stałych (`Games\World_of_Tanks_EU`, `_NA`, `_ASIA`, `World_of_Tanks`) i w bibliotekach Steam. Wcześniej sprawdzało tylko cztery ścieżki na dysku C:. Wykrywanie trwa najwyżej kilka sekund i nie przeszukuje całych dysków.
- **Ręczny wybór folderu.** Nowy przycisk „Wskaż folder gry” w Ustawieniach, obok „Znajdź grę”, otwiera systemowe okno wyboru folderu. Można wskazać folder gry albo folder z nagraniami bitew (`replays`) w nim — aplikacja sama przejdzie do folderu gry.
- **Pierwsze uruchomienie.** Bez wybranej gry Sesja od razu pokazuje „Wskaż folder gry” z przyciskami „Znajdź grę” i „Wskaż ręcznie”, zamiast odsyłać do Ustawień.
- **Konkretne komunikaty.** Przy złym folderze aplikacja mówi, czego brakuje: pliku WorldOfTanks.exe, folderu z nagraniami bitew (replays), samych nagrań (z podpowiedzią: w grze Ustawienia → Ogólne → Nagrywaj bitwy) albo uprawnień do odczytu. Gdy wykrywanie nic nie znajdzie, mówi to i proponuje wybór ręczny.

### In English — what changes for the player

- **The game on any drive.** "Find the game" looks for World of Tanks in the install entries of the Windows registry, in the usual folders on every fixed drive (`Games\World_of_Tanks_EU`, `_NA`, `_ASIA`, `World_of_Tanks`) and in Steam libraries. Before, it checked only four paths on drive C:. Detection takes a few seconds at most and never searches whole drives.
- **Choosing the folder by hand.** A new "Choose the game folder" button in Settings, next to "Find the game", opens the system folder picker. You can choose the game folder or the folder with battle recordings (`replays`) inside it — the app moves to the game folder by itself.
- **First start.** Without a chosen game, Session shows "Choose the game folder" with "Find the game" and "Choose manually" right away instead of pointing to Settings.
- **Specific messages.** For a wrong folder the app says what is missing: the WorldOfTanks.exe file, the battle recordings folder (replays), the recordings themselves (with a hint: in the game, Settings → General → Record battles) or the permission to read it. When detection finds nothing, it says so and offers the manual choice.

### Hotfix — game folder (technical)

- `src-tauri/src/game_folder.rs`: detection is a function of its sources (registry install locations, fixed drives, Steam roots from the registry) tested on temporary folders; it probes the uninstall entries (HKLM, HKLM WOW6432Node, HKCU) whose `InstallLocation` has `WorldOfTanks.exe`, the usual folders on every fixed drive, `steamapps\common\World of Tanks` in every Steam library (`libraryfolders.vdf`), returns canonical folders without duplicates and stops at a 4 s limit; no recursive scan, no network or removable drives.
- Validation of a folder (`validate_root`) gives four new error codes with Polish and English texts: `game_executable_missing`, `replays_folder_missing`, `replays_folder_empty` (only when choosing a folder), `game_folder_unreadable`. A chosen `replays` folder becomes its parent; a chosen `WorldOfTanks.exe` (also in `win64`/`win32`) its game folder.
- `tauri-plugin-dialog` 2.7.3 (official, the newest for Rust 1.89) and `@tauri-apps/plugin-dialog` ~2.7.3; the window capability gains `dialog:allow-open` only. Windows-only runtime crates for detection: `winreg` 0.55 and `windows-sys` 0.61 (both already in `Cargo.lock`).

### Internal (since 1.0.0-beta.2)

- **Internal:** the public snapshot script excludes one more folder (research trial code that never reaches the integration branch) and searches a one-segment excluded folder in public files as a path prefix instead of a bare word; tests cover both cases.
- **Internal:** research notes of the first discovery-sprint run (local AI summaries, reference tables, replay stream review); they stay in the private repository.
- **Internal:** spec 28 (local AI summaries, not in the app yet) records the author's decisions: model levels and VRAM thresholds, explicit choice of the discrete GPU, self-published Q4_K_M files, citation repair only when a number fits exactly one fact.
- **Internal:** spec 20 §11.1 lists reference tables per tier and class (a percentile comparison with the players of the author's own battles, computed locally, nothing sent) as a feature candidate; a mutation run of the reference-average module is the condition before it enters the app.

## 1.0.0-beta.2

Druga publiczna beta, instalator bez podpisu (pre-release). Wewnętrznie etap E9, spec 29. / Second public beta with an unsigned installer (pre-release). Internally stage E9, spec 29.

### Po polsku — co się zmienia dla gracza

- **Odświeżanie na żywo.** Nowa bitwa pojawia się sama na Sesji, w Historii, Czołgach i Celach, bez wychodzenia z ekranu; pasek „Odczyt powtórek” rośnie sam. Odświeżenie nie miga, nie gubi wybranego czołgu, filtrów ani doczytanych wierszy.
- **Poprawione animacje Czołgów i Historii.** Wybór czołgu nie miga już prawą stroną; łagodne wejście wierszy w Historii sięga do dołu okna także w dniu z jedną bitwą.
- **Nowe logo.** Litera A z rozgrzaną poprzeczką w ikonie aplikacji (pasek zadań, zasobnik, Alt+Tab, menu Start), w pasku nawigacji i w pasku tytułu; paleta granat–stal–żar.
- **Ekran ładowania.** Tylko gdy start naprawdę trwa: kontur znaku rozżarza się od podstawy razem z prawdziwym postępem. Zwykły start — od razu aplikacja.
- **Animacje nowych bitew i liczb.** Nowa bitwa wjeżdża na pasek Sesji i na górę Historii, liczby sesji liczą od starej do nowej wartości z krótkim kolorem kierunku (ciepły w górę, chłodny w dół). Wejście na ekran niczego nie porusza.
- **Pasek przewijania** w kolorach Afterbattle: cienki, jaśniejszy po najechaniu, pomarańczowy przy przeciąganiu.
- Wszystko to bez ruchu przy włączonym w Windows ograniczeniu animacji.

### In English — what changes for the player

- **Live refresh.** A new battle shows up by itself on Session, History, Tanks and Goals without leaving the screen; the "Reading battle files" bar moves by itself. A refresh never flashes and keeps the chosen tank, filters and loaded rows.
- **Fixed motion on Tanks and History.** Choosing a tank no longer flashes the right-hand side; the soft entrance of History rows reaches the bottom of the window also on a day with a single battle.
- **New logo.** An A with a glowing bar in the app icon (taskbar, tray, Alt+Tab, Start menu), the navigation rail and the title bar; a navy, steel and ember palette.
- **Loading screen.** Only when the start really takes time: the outline of the mark glows up from the base with the real progress. A normal start shows the app at once.
- **Motion of new battles and numbers.** A new battle slides onto the Session strip and onto the top of History; session numbers count from the old to the new value with a short tint of the direction (warm up, cool down). Entering a screen moves nothing.
- **Scrollbar** in the Afterbattle colors: thin, brighter on hover, orange while dragging.
- All of it without motion when Windows' reduce-animations setting is on.

### E9 stage 0 — Tanks and History motion in the installed app (spec 29 §1)

- Tanks: choosing another tank keeps the right-hand panel on screen; the old data stays until the new tank's data arrives, with no loading flash and no second entrance.
- History: the entrance wave reaches the bottom of the window also when the newest day has a single battle (a day header between the first rows no longer shortens it); measured after the fonts and the first frame.
- Map pictures that arrive after a tile is drawn fade in.

### E9 stage 1 — live refresh (spec 29 §2)

- The window now receives the backend's events (it was not allowed to listen, so they never arrived): a new battle shows up on Session, History, Tanks and Goals without leaving the screen, and the import progress bar moves by itself.
- A refresh keeps what is on screen until the new data is in: no loading flash, the chosen tank and filters stay, rows loaded with "Show more" stay, nothing scrolls or enters again.

### E9 stage 2 — brand (spec 29 §3, §4, §7)

- New Afterbattle mark: an A outline with a rising bar, in the app icon (taskbar, tray, Alt+Tab, Start), the navigation rail, the title bar and the README.
- The brand palette as tokens (navy, steel, ember; a cool blue for falling numbers); the title bar on the brand surface.
- A thin scrollbar in the brand colors everywhere, brighter on hover and orange while dragging.

### E9 stage 3 — loading screen (spec 29 §5)

- When the start really takes time (a slow cold start, or the first reading of your replays into empty data), the outline of the mark glows up from the base with the real progress, then fades. A normal start shows the app at once.

### E9 stage 4 — motion of a new battle (spec 29 §6)

- A battle that arrives while Session is open slides onto the strip; the session's numbers count from the old to the new value with a short warm (up) or cool (down) tint. Entering the screen moves nothing.
- A battle that arrives while History is open enters at the top and the list glides down; a scrolled list keeps its place.

## 1.0.0-beta.1

Pierwsze publiczne wydanie — beta, instalator bez podpisu. Wewnętrznie ta wersja to „v2” (etapy E0–E8 niżej). / First public release — a beta with an unsigned installer. Internally this version is "v2" (stages E0–E8 below).

### Po polsku — co się zmienia dla gracza

- **Nowy wygląd.** Ekrany Sesja, Czołgi, Cele, Historia, Bitwa i Ustawienia na grafikach map z Twojej gry, z własnym paskiem tytułu okna. Na dużych monitorach mapy są ostre: aplikacja bierze ekrany ładowania z gry i dopasowuje je do miejsca, w którym je pokazuje. Nazwy czołgów są takie jak w grze.
- **Sesja opowiada, jak poszło.** Jedno zdanie na górze („Mocna sesja: obrażenia o 18% wyżej niż zwykle”), bitwa po bitwie z najlepszą wyróżnioną i najciekawsze momenty sesji.
- **Cele na tydzień.** Dwa–trzy cele wyliczone z Twoich czterech poprzednich tygodni (przetrwanie, obrażenia z asystą na ulubionym czołgu, XP), postęp na bieżąco i ocena poprzedniego tygodnia. Procent wygranych nigdy nie jest celem.
- **Liczą się tylko bitwy losowe** z pełnym wynikiem; trening, turnieje i wydarzenia są zapisane, ale nie psują statystyk. Asysta to wykrycie + gąsienice, XP bez premium.
- **Nic nie ginie.** Cała historia powtórek wczytuje się w tle, bez limitu 500 plików i 90 dni. Przed każdą zmianą bazy powstaje kopia; uszkodzona baza jest odsuwana na bok i odtwarzana z powtórek.
- **Prywatność.** Aplikacja czyta tylko powtórki i pliki gry, nic nie wysyła; jedynym połączeniem są dane WN8 z XVM, i to dopiero po włączeniu. Usunięto Tomato.gg, klucz Wargaming i czytanie `python.log`.
- **Spokojniejsza praca.** Długie operacje nie blokują okna, odczyt powtórek można zatrzymać, a ruch na ekranie wyłącza ustawienie „ogranicz ruch” w Windows.

### In English — what changes for the player

- **New look.** Session, Tanks, Goals, History, Battle and Settings screens on map pictures from your own game, with the window's own title bar. On large monitors maps are sharp: the app uses the game's loading screens and fits them to where they are shown. Tank names are the game's own.
- **The session tells you how it went.** One sentence at the top ("Strong session: damage 18% above usual"), battle by battle with the best one framed, and the session's notable moments.
- **Goals for the week.** Two or three goals worked out from your four previous weeks (survival, damage with assist on your favourite tank, XP), live progress and a verdict on last week. Win rate is never a goal.
- **Only random battles count**, with full results; training, tournaments and events are stored but never skew your statistics. Assist is spotting + tracking, XP is without premium.
- **Nothing gets lost.** Your whole replay history is read in the background, without the 500-file and 90-day limits. Every database change makes a backup first; a damaged database is moved aside and rebuilt from replays.
- **Privacy.** The app only reads replays and game files and sends nothing; the one connection is the XVM WN8 data, and only after you turn it on. Tomato.gg, the Wargaming key and reading `python.log` are gone.
- **Smoother.** Long operations never freeze the window, reading replays can be stopped, and on-screen motion follows the Windows "reduce motion" setting.

### v2 stage public-docs — self-contained public documents

- Public documents and code comments no longer refer to files kept out of the public repository; the snapshot script stops when one does.
- The public snapshot is committed with the GitHub noreply address of the logged-in account, set only in that repository.

### v2 stage public-prep — public repository

- The public repository is the private tree minus whole excluded files (`scripts/make-public-snapshot.ps1`), scanned for secrets, personal data and game files before every commit; `docs/PUBLIC_RELEASE_REVIEW.md` lists what is left out and why.
- `NOTICE` with the font licenses next to Apache-2.0; README says the project is not affiliated with Wargaming.
- Test data: synthetic arena id and replay file names. CI keeps the unsigned installer and `SHA256SUMS.txt` as a build artifact.

### v2 stage E8-ui-fix — calmer motion

- Tanks: the screen no longer slides in over its loading state; map tiles, their bars and the chart enter once, after the data, at the History tempo; choosing another tank shows it still.
- History: the entrance wave covers all rows visible at first display (up to 20), not just the first 10.

### v2 stage E8 — release

- The version has one source, `src-tauri/Cargo.toml`: Tauri reads it there, the window gets it at build time, and a test keeps `package.json` equal. The first public release is `1.0.0-beta.1`; the title bar and window title read "Afterbattle beta 1.0"; the channel is "Public beta".
- `scripts/release.ps1` builds the installer from a clean tree and writes `SHA256SUMS.txt`; README explains the SmartScreen warning and checking the file with `Get-FileHash`.
- CI: actions pinned to commit SHAs, separate test, audit (`cargo audit`, `pnpm audit --prod`) and installer build jobs; Dependabot also updates the actions (in the private repository only).
- The alpha's manual data cleanup left the test suite (archived outside the public repository, not built).
- Documents match 2.0: README, privacy notice (EN/PL), SECURITY, THREAT_MODEL, KNOWN_ISSUES and `docs/RELEASE_CHECKLIST.md` (replaces the alpha checklist).

### v2.0 stage E5-maps-hd — sharp map pictures, own title bar

- Map pictures are sharp on large screens: headers, moment cards and history rows use the game's loading screens (2560 × 1600), decoded and scaled down in the app to the exact size shown. A picture is never stretched more than 1.25×; when the game has only a small picture, it sits on the right over a blurred copy of itself.
- Fixed pictures being scaled down and then stretched again in tall tiles.
- The window has its own title bar in the app's style: drag anywhere on it, double-click to maximize or restore, minimize, maximize/restore and close (to the tray, as before). The Windows 11 snap layouts flyout is not available.

### v2.0 stage E7 — weekly report

- The Goals screen opens with "Goals for this week": up to three goals set from your four previous weeks (survive your median share plus 5 points, damage with assist on your most played tank plus 10%, XP plus 10%), each with where it comes from and a progress bar. Win rate is never a goal; it appears only as context.
- "This week against the previous four": damage with assist, survival and XP against your own average on the same tanks, with the range of the four weeks before; WN8 with XVM data. Fewer than 15 battles in the week: "too few battles for a trend".
- Last week's goals judged in one line, and the 5-battle challenges finished this week. Weeks are ISO weeks (Monday to Sunday) in local time; when a clock change skips Monday midnight, the week starts at its first valid hour. New command `get_weekly_report`; nothing new is stored.

### v2.0 stage E5-flare-fix2 — sticky navigation, calmer History

- The navigation rail stays in the window on desktop: Settings is always visible at its bottom on every screen, however long; the rail scrolls on its own only when the window is lower than its content. In a narrow window the rail sits above the content and does not stick.
- History motion: the whole screen no longer slides in; only the first 10 rows of the first visit enter, softly (320 ms, 6 px, staggered within about 400 ms). Rows below, rows loaded later, a filter change, switching Battles/Sessions and coming back from a battle show no motion.

### v2.0 stage E5-flare-fix — damage trend numbers match the strip

- The "damage trend" session moment shows the damage of the first and the last battle of the session when every battle had more (less) damage than the one before, the same numbers as the first and the last tile of the strip. Otherwise it keeps the averages of the first and the last third and says so next to the numbers ("Average of the first 3 battles → of the last 3").

### v2.0 stage E5-flare — map-led direction C, Big Shoulders

- Map pictures from the game files carry the screens: the Session and Battle headers sit on a map, battles are tiles with their map, History rows have a map strip. Pictures come from `map/stats` (500×235) with the square `map/` pictures as a fallback, load only when visible and are scaled to the tile before the window keeps them; without game files every tile and header has a gradient placeholder. Loading screens (DDS) are not read.
- Session: a headline sentence from fixed rules (strong, calm, tough or neutral session, same damage reference as the hero numbers), the session numbers on the best battle's map, a "battle by battle" strip (losses dimmed and named, the best battle framed) and session moments (damage trend, the tank to improve, the best battle). New command `get_session_story`.
- Tank: compact header with three numbers against the player's average and a grid of the player's maps on that tank (4+ battles, two best and the weakest named); the chart and the battle table stay.
- Battle: a big header on the map with the damage as the main number, "Your battle" next to the teams and a "You on <map>" card with all the player's battles on that map and a link to History filtered by it. New command `get_map_record`.
- History: day headers ("Today", "Yesterday", a date), survival in words, losses dimmed and named; sessions as cards with a collage of their maps. New command `get_session_maps`. Goals: a tank icon at goal titles.
- Big Shoulders (SIL OFL 1.1, local woff2) replaces Barlow Semi Condensed for numbers and headings; Inter stays for text.
- Motion explains changes only: motion tokens, hover and press feedback, a sliding segmented-control highlight, a focus ring that fades in, a light screen entry, a one-time cascade of tiles and drawing of bars, a count-up of the main number only when a new battle arrives in the running session. Only transform and opacity move, nothing loops, `prefers-reduced-motion` turns movement off.

### v2.0 stage E5-ui — UI corrections after the author's manual test

- Wide screens: up to 1440 px the layout is one column as before; above it the content grows to about 1840 px, centred next to the navigation, with two columns on Session (hero card and insights | battles), Battle (result and statistics | team comparison) and Tank (cards and chart | maps and battles, side by side from 1920 px), and extra History columns (destroyed, survival). Spacing and size tokens are unchanged.
- Tank names: the game's own names are read on demand from its translation files (`res/text/lc_messages/<nation>_vehicles.mo`, keys from `list.xml`) and kept only in memory, in the window language or English (a file without a declared language is the game client's own language and serves both); the UK reads `gb_vehicles.mo`; without them the stored code is shown readable ("Vz 60S Dravec" instead of "czech-Cz21_Vz_60S_Dravec"). Lists show the short name, the Tank header and tooltips the full one; names never wrap. New command `get_vehicle_names`; names are never stored in the database or in an export.
- Attempts and conversion: a card with a three-step funnel (shots → hits → penetrations) with large numbers, captions and transition rates; team and explosion hits on one secondary line; the range is a segmented control (radio group with arrow keys); one "How we count this" dialog lists the skipped battles per indicator instead of five expandable sections.
- Battle pace: one summary line ("2 of 9 battles had a one-sided course") and a small split bar with the name and count of every part in words.
- Session insights fix: a session far below the average on the same tanks was described as "close to your average" when no tank had 3 battles in the session. The hero card and the insights now use one damage reference; when confidence is too low the insights say so.
- Battle screen: the header sits on the map image (placeholder without game files), full page width.
- Session hero card no longer stretches to the cards next to it and shows this session's battles against the average.
- Tank screen: no repeated tank column in its battle table; the chart has a value axis and the player's average as a dashed line; the header names the last battle ("last yesterday").
- Clickable rows: no text selection, pointer cursor, visible keyboard focus. History: the tank filter says when its list failed to load instead of staying empty.

### v2.0 — rebuild gate counts full battles only

- The check before the one-time v10 rebuild ("more than 5% of battles without a replay file") now looks at full battles only. Incomplete battle records from early alpha versions never counted towards statistics; they no longer block the rebuild and stay only in the `.bak` copy. The 5% limit is unchanged.
- Sessions that have no battle after the rebuild are not carried over to the new database (they stay in the `.bak` copy). Sessions with battles, challenges and their history are kept as before.
- The rebuild report in Settings and in the diagnostic report shows separately: full battles without a replay file, skipped incomplete records and skipped empty sessions.

### v2.0 stage E5-fix — corrections after E5

- The Tank screen header shows the vehicle tier: "Medium tank, tier X, 214 battles". The tier comes from the newest battle of that vehicle that has one stored (`tier` in the vehicle summary of `get_vehicle_history_overview` and `get_vehicle_history_report`); when it is unknown the header reads "Medium tank, 214 battles".
- Tests only: the reader of images from the game folder went through a complete mutation run, with new tests for the key pattern, cache replacement and limits, the package filter and the index limit; every screen now has a test for each of its loading, empty, error, no-XVM and too-few-battles states.

### v2.0 stage E5 — UI redesign (with the former E6)

- New interface following the approved mockup: a vertical navigation with Session, Tanks, Goals, History, Maps (greyed out in the private build, hidden in the public build) and Settings; one design-token system with a single orange accent; local Barlow Semi Condensed and Inter fonts (SIL OFL 1.1), no network fonts.
- Screens: Session (average damage against your own average on the same tanks, highlights, battles), Tanks (your last 10 battles against your average of 100 on that tank, damage chart of up to 200 points, best and worst maps, battles in pages of 50), Battle (your result, remaining health, comparison of both teams from team totals only, never a player list), Goals (the existing 5-battle tank challenges), History (battles filtered by tank and map, sessions), Settings with privacy, the rebuild report, WN8 data and diagnostics.
- Tank icons and map images are read on demand from the selected game folder and kept only in a size-limited RAM cache, emptied when the game updates; nothing is written to disk. Placeholders appear when an image is missing. New commands `get_game_image` and `get_battle_view`; the battle history accepts a map filter.
- Every screen has loading, empty, error, no-XVM and too-few-battles states; errors are shown in Polish or English from one dictionary covering every backend code; dialogs close with Escape and keep the focus inside.
- Code structure `app/`, `pages/`, `components/`, `hooks/`, `i18n/`, `lib/`, `styles/`; the old interface, `styles.css` and `alpha.css` are gone. TypeScript types of the IPC DTOs are generated by `ts-rs`; a contract test keeps `api.ts` in line with the registered commands; ESLint (`react-hooks`, `no-floating-promises`) runs with `pnpm lint`.
- Synthetic screenshots of every screen in every state in `docs/status/screens/`.

### v2.0 stage E4 — analysis and WN8

- XVM download: the attempt is recorded when it ends, with its result; an automatic retry waits 1 hour after a failure and 24 hours after a success (D3).
- A bad row in the XVM file (missing field, wrong type, value out of range) is skipped and counted instead of rejecting the whole file, up to 1% of rows; the count is shown in the WN8 status and diagnostics (M8). The parser accepts the live format with `header` after `data` and an extra `url` field (P4 check, 2026-11-12).
- Reference averages, session insights and challenge suggestions qualify each metric on its own: damage, survival and assist work without XVM, WN8 alone needs the expected values (M5, D7). A baseline that is too small carries no signals (M10).
- The attempts-conversion contract accepts feature schema 2 or 3 (M12).
- Per-battle feedback uses two SQL aggregates instead of loading up to 200 full rows (P4).
- The XVM data version is shown once, in the footer and in Settings, not next to values (D3).
- WN8 is checked against an independent calculator in three cases (±0.5).

### v2.0 stage E3 — threads, events, import performance

- Commands that touch files or many rows run off the window thread (`spawn_blocking`); a command that panics reports an error instead of crashing.
- `start_import` and `cancel_import` replace `import_local_history`: the full import runs in the background, reports progress with the `import-progress` event and stops after the current batch on request; the next start continues from the cursor without duplicates.
- Events `data-changed` and `session-ended` replace the 10-second polling; the 30-second timer is gone. Automatic session end has one schedule, in the tracker thread, with an injected clock (tests no longer sleep).
- The replay content hash covers only the JSON blocks, not the whole file.
- Lighter snapshot: WN8 of the last 30 sessions moved to the new command `get_recent_sessions_wn8` (spec §7, 2026-11-12).
- The window shows "Reading replays: x / y" with a "Stop reading" button.

### v2.0 stage E2 — database v10, migrations, recovery, no missing battles, data foundations

- Database schema v10. A database from an older version is first copied unchanged to `afterbattle.v{n}.bak`; if more than 5% of its battles have no replay file left, nothing else happens and the app starts in recovery mode with the code `legacy_rebuild_blocked`. Otherwise a new v10 database replaces it, keeping settings, all sessions (one still active is closed as timed out) and challenges with their frozen data; every battle is then recalculated from its replay in the background, keeps its id and its session, and a report "recalculated / not random / without replay / conflicts" is written (shown in diagnostics). Battles without a replay stay only in the backup.
- From v10 on, migrations are incremental steps driven by `user_version`, with a `.bak` copy before each step and one transaction per step; a database from a newer version is refused (`database_too_new`). The integrity check runs once at start.
- Full import without the 90-day and 500-file limits, in the background, in batches of 100 with a resumable cursor; at every start files missing from the database or changed since (size, modification time) are read. Battles older than the inactivity limit do not create sessions. Non-random replays are remembered and not parsed again. Progress is part of the tracker status.
- A corrupt database file is moved to `afterbattle.corrupt-<time>.sqlite3` and a new database is rebuilt from the replays; "Delete all local data" also removes `.bak` and `.corrupt-*` files.
- One long-lived write connection behind a mutex; read-only reports keep their own connections.
- Replay parser version 9 stores the data foundations for later analysis: extra own-player columns, vehicle tier with the catalog fingerprint, an anonymous 30-row battle roster (`battle_roster`, no nicknames, account ids, clans, platoon ids or in-battle vehicle ids) with per-team aggregates (`battle_team_stats`), and the size of the replay's packet stream. Both new tables are excluded from the export. `PRIVACY.md` and `docs/PRIVACY_PL.md` describe the roster.
- Removed: the bounded team-relative reconciliation, the in-place parser refresh of old rows, the dead WN8 columns and the Tomato source kind. Team-relative contract amendment A1 accepts parsers 7, 8 and 9.

### v2.0 stage E1b — vehicle tier and roles in the catalog

- The vehicle catalog built from the game's `scripts.pkg` now holds each vehicle's tier (1–11) and `role_*` tags (catalog schema 2). Records with a decimal-string `id` are accepted.
- A missing known nation or a new nation folder no longer rejects the whole catalog; both are reported. Settings show tier coverage and warn when the catalog is out of date for the installed game version. Schema 1 catalogs stay usable (without tiers) until the next refresh rebuilds them.

### v2.0 stage E1 — privacy, data loss, less code, correct inputs

- Privacy: XVM expected values are downloaded only after the player enables XVM; startup, the switch and manual refresh share one gate and send nothing while XVM is off. The no-op `refresh_xvm` command is removed.
- Data safety: if the database cannot be opened, the app uses one fixed recovery database next to the real one (no new files in `%TEMP%`), does not import battles in that mode, removes the recovery database on the next successful start, and refuses "Delete all local data" instead of reporting a false success.
- Statistics count only strict random battles (`battleType == 1` and `bonusType == 1`, read as numbers) with full results; one predicate is used by session metrics, baselines, trends, WN8, challenges and battle feedback. Training, tournament, clan, ranked and event battles are stored but never counted.
- Assist is spotting + tracking (stun is separate); XP averages use XP without premium; a replay time in the repeated autumn DST hour is kept.
- Removed: the `python.log` reader, the Tomato.gg integration and profile, the Wargaming `application_id`, and the commands `local_source_status`, `sync_battles`, `import_history`, `configure_profile`, `store_api_key`, `remove_api_key`, `test_api_connection`, `store_wg_application_id`, `remove_wg_application_id`. Their leftover Credential Manager entries are deleted at every start and by "Delete all local data". The `json5` dependency is removed.
- Replay parser version 8. `PRIVACY.md`, `SECURITY.md`, `THREAT_MODEL.md` and README match the code; `docs/PRIVACY_PL.md` is a faithful translation of `PRIVACY.md`.

### v2.0 stage E0 — safety net, dependencies, formatting

- Security: dependency updates clear every `cargo audit` vulnerability (bundled SQLite via `rusqlite` 0.41 / `libsqlite3-sys` 0.39.1, `rustls`, `quinn-proto`, `rustls-webpki`, `webpki-roots`, `zip` 9.5) and `pnpm audit` for all dependencies. The unused `zod` dependency is removed.
- Dependencies: `sha2` 0.11, `serde_json`, `uuid`, `vite`, `@vitejs/plugin-react`, `react` / `react-dom` 19.3, `jsdom` 30, `@testing-library/jest-dom` 7, `vitest` 4.1.12. Minimum Rust version is now 1.89 (required by `rusqlite` 0.41).
- Frontend sources are formatted with Prettier (`pnpm format`, `pnpm format:check`); no logic changes.
- Tests: characterization tests pin current WN8, baseline, trend, challenge and import-gate behavior, including known defects, as the safety net for the following stages. An ignored, read-only `golden_local_replays` test prints aggregate statistics for a local replay folder.
- No change to application behavior.

## 0.1.0-alpha.1 — private alpha

- Local one-block and two-block `.wotreplay` import with Partial → Full upgrade and deduplication.
- Automatic sessions, inactivity timeout, tray operation, history and per-session reports.
- Validated local XVM expected-values cache, safe background updater and deterministic WN8 with coverage.
- Polish and English alpha interface with readable replay identities and WN8 categories.
- First-run guide, About panel, safe copied diagnostics and friendly UI failure state.
- Official Tauri single-instance protection.
- Current-user Windows x64 NSIS packaging under the stable `app.afterbattle.desktop` identifier.

This build is unsigned, has no automatic updater and is intended only for controlled private testing.
