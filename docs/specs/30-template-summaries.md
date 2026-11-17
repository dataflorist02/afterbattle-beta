# Spec 30 — Podsumowania z szablonów (bitwa, sesja, tydzień)

- **Status: APPROVED (decyzje autora z 2026-11-16, §11).** Pierwsza wersja podsumowań tekstowych, bez modelu językowego. Zastępuje w roli „pierwszej wersji” spec 28 (LLM odłożony, spec 28 §10).
- Data: 2026-11-16. Autor decyzji: Szymon.
- **Zmiana 2026-11-16 (decyzja autora):** §9 — wycena z historii etapów (rozmiar w liniach, czas agenta i autora) zamiast godzin pracy człowieka; dawna wycena w przypisie jako „praca ręczna”.
- **Zmiana 2026-11-16 (decyzje autora do §11, status APPROVED):** forma zwracania się do gracza w Ustawieniach, domyślnie bezosobowa (§2, §5, §8); EN poza tym wydaniem, karta ukryta w wersji EN (§2, §7, §10); tydzień < 15 bitew — jedno zdanie (§4.3); progi z A-3 bez zmian do rewizji (§4); bez faktu `czas_zycia` (§4.1, backlog w spec 20 §11.6); zdanie oceniające z makiety zastąpione faktem bez oceny (§8). Rozstrzygnięcia w §11.
- **Zmiana 2026-11-16 (poprawki autora przed wydaniem beta 4):** forma neutralna zamiast bezosobowej — bez konstrukcji na „-no/-to”, styl rzeczownikowy (§2, §5, §11 pyt. 1); pytanie o formę na ekranie pierwszego uruchomienia (§2); jedna składnia zdań z odniesieniem: wartość, różnica, odniesienie w nawiasie (§5); fakt `czolg` tylko przy > 50% bitew sesji (§4.2); zero obrażeń jako „0 obrażeń” zamiast „brak” (§5).
- **Zmiana 2026-11-16 (ostatnie poprawki fraz przed beta 4):** liczba przed procentem, jedna nazwa odniesienia i „pkt proc.”, jedna reguła zera, jeden fakt w zdaniu, jedyne zdanie otwierające bitwę (§4.1, §5).
- Podstawa: decyzja autora po badaniu A-3 (spec 28 §10); lista faktów, reguły wyboru, frazy i testy niezmienników z kodu próbnego A-3 (notatki badawcze autora, poza repozytorium publicznym); makieta `Moon-A3-Recap` na płótnie „Afterbattle 2.0 — redesign” (adres w `docs/design/README.md`).

## 1. Cel i zasady

Krótkie podsumowanie bitwy, sesji i tygodnia prostą polszczyzną, w którym każde zdanie ma odnośnik (F1, F2, …) do liczby na ekranie. Tekst składa kod z gotowych fraz według stałych reguł.

- **Bez modelu, bez sieci, bez pobierania, bez nowych uprawnień Tauri.** Nic nowego w PRIVACY i THREAT_MODEL.
- **Deterministycznie.** Te same dane i ten sam identyfikator sesji (bitwy, tygodnia) dają ten sam tekst przy każdym otwarciu ekranu. Nic nie jest zapisywane: tekst liczy się przy wyświetleniu, jak raport tygodniowy (spec 27).
- **Tylko bitwy liczone (D4).** Każda liczba w tekście pochodzi z faktu, a fakt z tych samych obliczeń, które zasilają liczby na ekranie.
- **Nie oceniamy gracza.** Bez jednej liczby „oceny gracza”, bez słów wpływ/wkład/carry/impact (D20), bez słów oceniających („dobrze”, „słabo”, „niestety”, „gratulacje”) i bez przyczyn wyniku. Kierunek („więcej”, „mniej”) tylko przy faktach z odniesieniem i zgodnie z jego znakiem.
- **Wygrane nie są celem (D12).** Odsetek wygranych może być faktem sesji i tygodnia, ale nigdy celem.

## 2. Zakres

| Rodzaj | Ekran | Miejsce |
|---|---|---|
| Sesja | Sesja | karta „Podsumowanie sesji” pod rzędem liczb (makieta `Moon-A3-Recap`) |
| Bitwa | Bitwa (w Historii) | sekcja „Podsumowanie”, której miejsce makieta redesignu zostawiła (`docs/design/README.md`) |
| Tydzień | Cele | karta pod nagłówkiem tygodnia (spec 27) |

- **Forma zwracania się** (zmiana 2026-11-16, §11 pyt. 1; poprawka tego samego dnia przed beta 4): ustawienie „Jak się do Ciebie zwracać” — męska („zadałeś”), żeńska („zadałaś”), bez formy (neutralna: „Wygrana na …, czołg przetrwał do końca.”); **domyślnie neutralna**. Ekran pierwszego uruchomienia pyta „Jak się do Ciebie zwracać?” (on / ona / bez formy) i zapisuje odpowiedź do tego ustawienia; do czasu odpowiedzi forma neutralna. W Ustawieniach wybór jak dotąd. Ustawienie obejmuje też istniejące teksty interfejsu w 2. osobie czasu przeszłego (np. moment „Rozkręcałeś się”, „Przeżyłeś”).
- **Języki:** PL. **EN nie w tym wydaniu** (zmiana 2026-11-16, §11 pyt. 2): w wersji EN karta podsumowania jest ukryta.
- **Poza zakresem:** model językowy, generowanie na żądanie, zapisywanie tekstów, eksport i udostępnianie.

## 3. Gdzie w kodzie (propozycja)

**Fakty i wybór faktów w Ruście, frazy i składanie zdań w TypeScripcie.**

| Warstwa | Miejsce | Co robi |
|---|---|---|
| Fakty | Rust, nowy moduł `src-tauri/src/recap.rs`, komenda `get_recap(kind, id)` | lista faktów `F1…Fn` ze stałymi kluczami, wartościami surowymi (liczby, nie tekst), odniesieniem, różnicą, kierunkiem, flagą „ważny”; reguły wyboru (§4) z progami jako stałe; `seed` (§6). Typy przez ts-rs |
| Tekst | TS, nowy katalog `src/lib/recap/` | frazy PL (EN poza tym wydaniem) z wariantami, odmiana, liczby przez `format.ts`, wybór wariantów, kolejność zdań, odnośniki |
| UI | TS, komponent `src/components/Recap.tsx` | lista zdań z odnośnikami, podświetlenie liczby (§8) |

**Uzasadnienie.**
- Liczby, bazy odniesienia (średnie czołgów, ostatnie bitwy), pierwszeństwo bitew liczonych i tydzień ISO już są w Ruście (`session_insights.rs`, `session_story.rs`, `weekly.rs`, wyniki zespołowe). Reguły wyboru operują na surowych danych i progach, tak jak progi z spec 26 i 27, testowane `cargo test`. Przeniesienie ich do TS powieliłoby obliczenia.
- Wszystkie teksty interfejsu i oba języki są w TS (`src/i18n/ui.ts`), a liczby formatuje `src/lib/format.ts` („3 184” z niełamliwą spacją, procenty w całości). Liczba w zdaniu musi wyglądać tak samo jak liczba na ekranie, którą podświetla odnośnik, więc tekst powstaje tam, gdzie liczby są formatowane. Odmiana to sprawa języka, nie danych.
- Odrzucone: wszystko w Ruście (drugie miejsce na teksty i formatowanie, rozjazd z `format.ts`); wszystko w TS (powielenie baz i reguł D4).

## 4. Fakty i reguły wyboru

Przeniesione z A-3 (te same klucze, frazy i progi; progi są stałymi w `recap.rs` do strojenia przez autora). Progi z A-3 przyjęte bez zmian; rewizja po 2 tygodniach używania (§11 pyt. 4). Fakt bez danych (np. pole `None`) nie powstaje. Identyfikatory nadawane po kolei tylko faktom pozostawionym (F1, F2, …), bo odnośniki widzi gracz.

### 4.1 Bitwa

| Klucz | Dane w aplikacji | Ważny | Pomijany, gdy (reguła z A-3) |
|---|---|---|---|
| `wynik` | `outcome` | tak | — |
| `mapa` | `map_name` | — | — (łączony z wynikiem) |
| `czolg` | `tank_name`, tier | — | — |
| `obrazenia` | `damage` wobec średniej czołgu (baza z `baseline.rs`, jak na ekranie) | tak | — |
| `asysta` | `assist` wobec średniej czołgu | czołg lekki | czołg inny niż lekki i asysta < 800 |
| `zniszczone` | `frags` | ≥ 2 | 0 |
| `wykryte` | `spots` | — | czołg inny niż lekki i < 3 |
| `strzaly` | `shots`, trafienia, przebicia | — | — |
| `pancerz` | `blocked` | czołg ciężki > 1000 | < 1000 (ciężki), < 500 (inne) |
| `przetrwanie` | `survived` | tak | — |
| `miejsce` | miejsce w drużynie według obrażeń (wyniki zespołowe) | ≤ 3 | > 3 |
| `udzial` | udział w obrażeniach drużyny (wyniki zespołowe) | — | < 20% albo gdy zostaje `miejsce` (ten sam sens) |

**Zdanie otwierające** (zmiana 2026-11-16): zawsze wynik na początku i tylko ten wariant — „Przegrana na Test Valley, czołg Synth 44 M zniszczony.”, „Wygrana na Synthetic Hills, czołg Alpha przetrwał do końca.”; gdy przetrwanie ma własne zdanie: „Remis na Port Delta, czołg Kolos IV.”

Czasu życia czołgu w bitwie aplikacja dziś nie ma (`duration_seconds` to długość bitwy). Fakt `czas_zycia` z A-3 nie wchodzi (zmiana 2026-11-16, §11 pyt. 5); pole czasu życia czołgu jest w backlogu „przy najbliższej zmianie schematu” (spec 20 §11.6).

### 4.2 Sesja

| Klucz | Dane | Ważny | Pomijany, gdy |
|---|---|---|---|
| `liczba_bitew` | bitwy liczone sesji | tak | — |
| `odsetek_wygranych` | wygrane / bitwy wobec ostatnich 100 bitew przed sesją | tak | — |
| `obrazenia` | obrażenia na czołgach sesji wobec ich średnich (to samo odniesienie co nagłówek „o 20% wyżej niż zwykle”) | tak | brak bazy (jak nagłówek „neutral”) |
| `przetrwanie_odsetek` | odsetek przetrwanych wobec ostatnich 100 bitew | — | różnica < 5 pkt proc. |
| `czolg` | najczęściej grany czołg; przy 100% bitew zdanie „Cała sesja na czołgu X.” | — | żaden czołg nie ma więcej niż 50% bitew sesji (np. remis 1 z 2) (zmiana 2026-11-16) |
| `najlepsza` | najlepsza bitwa: obrażenia **i mapa w jednym fakcie** (A-3: dwa osobne fakty dawały dwa zdania o tym samym) | tak | mniej niż 2 bitwy z obrażeniami (`MIN_BEST_BATTLES`) |
| `wn8` | WN8 sesji wobec 30 dni | tak | dane WN8 wyłączone |

Pominięte zawsze (ten sam sens co inny fakt): wygrane/przegrane obok odsetka wygranych, czas gry obok liczby bitew. Przy 0% przetrwania fraza „nie przeżyłeś do końca żadnej bitwy” zamiast „0% bitew” (decyzja autora 2026-11-16).

### 4.3 Tydzień

Fakty z raportu tygodniowego (spec 27): liczba bitew i pewność tygodnia; wskaźniki obrażeń z asystą, przetrwania i XP (100% = jak zwykle); stan celów (osiągnięty, blisko, poniżej); poprzedni tydzień („K z M celów”); wyzwania na 5 bitew; odsetek wygranych jako kontekst.

| Reguła | Kiedy |
|---|---|
| tylko zdanie o liczbie bitew: „za mało bitew w tym tygodniu” (jedno zdanie, wyjątek od §7 pkt 9; zmiana 2026-11-16, §11 pyt. 3) | pewność „za mało bitew na trend” (< 15 bitew, próg z E7) |
| wskaźnik pomijany | różnica od 100% mniejsza niż 5 punktów |
| cel pomijany | stan „za mało bitew” |
| wyzwania pomijane | 0 ukończonych, nieudanych i porzuconych |

## 5. Frazy, odmiana i liczby

- Każdy fakt ma frazę z liczbą, np. „zadałeś przeciwnikom 2 028 obrażeń, o 69 więcej niż Twoja średnia na tym czołgu”, „pancerz Twojego czołgu zatrzymał 1 850 obrażeń od przeciwników”, „zniszczyłeś 2 pojazdy przeciwnika”. Żargon gry („zablokowałeś”, „asysta”, „fragi”) nie trafia do tekstu.
- **Odmiana w kodzie, nie w szablonach:** forma rzeczownika i czasownika wybierana z liczby przez `Intl.PluralRules("pl")` (one: 1; few: 2–4 bez 12–14; many: 0, 5+; other: ułamki → „6,4 punktu procentowego”). Trzy formy podaje fraza (np. „pojazd / pojazdy / pojazdów”, „trafił / trafiły / trafiło”).
- **Liczby wyłącznie przez `format.ts`:** `formatNumber` (separator tysięcy jak w aplikacji, „3 184”), `formatPercent` (procenty w całości, jak na ekranach), różnice procentowe w punktach procentowych w całości. Tekst nie formatuje liczb sam.
- **Forma zwracania się** (zmiana 2026-11-16, §11 pyt. 1; poprawka przed beta 4): czas przeszły w 2. osobie ma rodzaj („zadałeś / zadałaś”). Fraza zapisuje czasownik raz, tematem; kod tworzy z niego formę męską (temat + „łeś”) i żeńską (temat + „łaś”). **Forma neutralna** (domyślna) nie ma czasownika w 2. osobie ani konstrukcji bezosobowej na „-no/-to” („Zremisowano”, „Przeżyto”, „Rozkręcano się”, „Zagrano”): styl rzeczownikowy („Wygrana na …, czołg przetrwał do końca.”, „Remis na …”, „Coraz lepiej z każdą bitwą.”); warianty z czasownikiem są w niej pomijane, każde zdanie ma wariant neutralny. Dotyczy też istniejących tekstów interfejsu (momenty sesji, przetrwanie w bitwie). Test: żaden tekst w formie neutralnej nie zawiera czasownika z listy form na „-no/-to”. Testy obejmują wszystkie trzy formy.
- **Zdania z odniesieniem** (zmiany 2026-11-16): jedna składnia — wartość bieżąca, różnica, odniesienie w nawiasie na końcu: „Średnio 3 414 obrażeń, o 13% więcej niż zwykle na tych czołgach (3 021).” Odsetek nie jest dopełnieniem: najpierw liczba, potem procent — „Wygrałeś 7 z 12 bitew, czyli 58%, o 7 pkt proc. więcej niż w ostatnich 100 bitwach (51%).”, „Czołg przetrwał do końca 5 z 12 bitew, czyli 42%, …”. **Jedna nazwa odniesienia** dla każdego rodzaju faktu: „w ostatnich 100 bitwach” (wygrane, przetrwanie), „zwykle na tych czołgach” (obrażenia sesji), „zwykle na tym czołgu” (obrażenia i asysta w bitwie), „w 30 dniach przed sesją” (WN8); punkty procentowe zawsze „pkt proc.”. **Zero** z odniesieniem, jedna reguła: „<zero>; zwykle <odniesienie>.” bez różnicy — „Czołg nie przetrwał do końca żadnej bitwy sesji; zwykle 38%.”, „0 obrażeń; zwykle na tym czołgu 2 900.”. Testy: pierwsza liczba to wartość bieżąca, jedyny nawias to odniesienie po dozwolonej nazwie (lista w teście); zero w podanym formacie.
- **Jeden fakt = jedno zdanie** (zmiana 2026-11-16): żadnych faktów sklejonych średnikiem (najlepsza bitwa i czołg, zniszczone i miejsce, liczba bitew i wygrane tygodnia). Średnik najwyżej jeden i tylko w formacie zera (test). Liczba bitew sesji jest w zdaniu o wygranych („7 z 12 bitew”), gdy każda bitwa ma wynik. Gdy faktów jest więcej niż 5 zdań, odpadają od końca fakty nieważne (w sesji najpierw czołg, w bitwie ostatni fakt z budżetu). Wyjątki: zdanie otwierające bitwę (wynik, mapa, czołg i jego stan to jedno zdanie, §4.1) oraz zdania-listy tygodnia (wskaźniki, cele).
- Długość: 3–5 zdań, najwyżej 25 słów w zdaniu. Kolejność: wynik z mapą i przetrwaniem, obrażenia, potem fakty ważne, potem pozostałe.

## 6. Różnorodność

- Każda fraza i każde zdanie łączące fakty ma **2–4 warianty** o tym samym znaczeniu (np. odniesienie „niż Twoja średnia na tym czołgu” / „niż zwykle na tym czołgu”; „Wygrałeś bitwę na mapie X i przeżyłeś do końca.” / „Bitwa na mapie X zakończyła się wygraną, a Twój czołg przetrwał.”). Wszystkie warianty przechodzą te same testy (§7).
- **Wybór deterministyczny:** `seed` z Rusta to stabilny skrót identyfikatora (sesja: jej identyfikator; bitwa: `arena_unique_id` albo identyfikator bitwy; tydzień: data początku). Wariant = funkcja (`seed`, klucz frazy, miejsce w tekście). Ten sam ekran zawsze pokazuje ten sam tekst.
- **Bez powtórzeń w jednym tekście:** ten sam wariant i to samo pierwsze słowo nie wracają w jednym podsumowaniu (np. dwa zdania od „Zadałeś”); gdy wybrany wariant by powtórzył, bierze się następny.
- **Test „10 kolejnych sesji nie brzmi identycznie”:** 10 syntetycznych sesji o tych samych faktach i kolejnych identyfikatorach daje co najmniej 6 różnych tekstów, żadne dwa kolejne nie są identyczne, a pierwsze zdanie ma co najmniej 2 różne warianty. Drugi test: ten sam identyfikator → identyczny tekst.

## 7. Testy niezmienników (CI)

Generator syntetycznych danych w testach (ziarno stałe): co najmniej 200 bitew, 100 sesji i 50 tygodni o różnych klasach pojazdów, wynikach, zerach i brakach danych. Każdy tekst w trzech formach zwracania się (męska, żeńska, bezosobowa), z każdym wariantem fraz, musi spełniać:

1. **Każda liczba z faktu:** każda liczba w zdaniu (także z separatorem tysięcy i przecinkiem dziesiętnym) jest liczbą faktu zacytowanego w tym zdaniu, po zaokrągleniu do zapisu w zdaniu. Odnośniki zdania wskazują fakty pozostawione.
2. **Kierunek:** „więcej / mniej / wyżej / niżej” tylko przy cytowanym fakcie z odniesieniem i zgodnie ze znakiem różnicy.
3. **Brak sprzeczności z faktami tekstowymi:** zdanie nie mówi „przetrwał / przeżyłeś” przy zniszczonym czołgu, „został zniszczony” przy przetrwaniu ani „wygrana” przy przegranej (i odwrotnie). Reguła z spec 28 §6 (2026-11-16).
4. **Słownik (PL):** każdy wyraz każdego wariantu frazy, w każdej formie odmiany (liczby 0–25, 100–105, 1 000–1 005, ułamki) i trzech formach zwracania się, jest na liście `src/lib/recap/words.pl.txt` albo jest nazwą własną z faktów. Lista powstaje raz skryptem z SJP.PL (wydanie, SHA-256 i licencja jak w spec 28 §6; w aplikacji na CC BY 4.0, z informacją o źródle w plikach licencji). CI nie pobiera słownika: test działa na liście w repozytorium, a skrypt sprawdza, że każde słowo listy jest w SJP.PL. Reguła „zniekształcony termin” (spec 28 §6) dotyczy modeli i tu nie jest potrzebna, bo tekst nie powstaje z tokenów.
5. **Brak powtórzeń sensu:** żaden sens (np. przetrwanie, wynik, najlepsza bitwa) nie pojawia się w dwóch zdaniach jednego tekstu.
6. **Brak faktów pominiętych:** żadne zdanie nie mówi o fakcie, który reguła z §4 usunęła.
7. **Słowa zakazane i oceniające:** brak słów z D20 i słownika ocen.
8. **Pokrycie:** każdy fakt ważny jest w tekście.
9. **Forma:** 3–5 zdań (tydzień z < 15 bitwami: 1 zdanie, §4.3), ≤ 25 słów, zdanie zaczyna się wielką literą i kończy kropką.

Dodatkowo: testy Rusta dla każdej reguły z §4 (próg włącznie i wyłącznie), test kolejności i ciągłości identyfikatorów, test zgodności typów ts-rs. EN: poza tym wydaniem (§2); test, że w wersji EN karta jest ukryta.

## 8. Interfejs (według makiety `Moon-A3-Recap`)

- **Sesja:** karta „Podsumowanie sesji” pod rzędem liczb, lista zdań, po każdym zdaniu odnośniki F1… jak na makiecie. Najechanie lub fokus na odnośniku podświetla tę liczbę w rzędzie liczb, gdy jest na ekranie; inaczej pokazuje podpowiedź z treścią faktu.
- **Bitwa i Cele:** ta sama karta w miejscach z §2.
- **Zmiany względem makiety:** bez przycisków „Napisz podsumowanie”, „Napisz od nowa” i „Przerwij”, bez stanów „Przed napisaniem” i „W trakcie”, bez etykiety modelu i czasu („Bielik 1.5B · karta graficzna · 1,4 s”). Tekst jest od razu. Przypis zamiast „Napisane lokalnie przez AI…”: „Złożone z Twoich liczb według stałych reguł. Każde zdanie ma odnośnik do liczby; najechanie podświetla ją wyżej.” Zdanie typu „Słabszy punkt to…” z makiety nie wchodzi, bo ocenia (§1); w jego miejscu jest zwykły fakt bez oceny (D20), np. odsetek przetrwania z różnicą wobec ostatnich 100 bitew (zmiana 2026-11-16, §11 pyt. 6).
- Nagłówek sesji („Mocna sesja: obrażenia o 20% wyżej niż zwykle”) zostaje z `session_story` bez zmian.
- Dostępność: odnośniki to prawdziwe przyciski z `aria-label` („Fakt F1: średnie obrażenia 3 240”), cel kliknięcia ≥ 44 px (obszar większy niż widoczny znaczek), kontrast ≥ 4,5:1, lista jako `<ol>`.
- Bez nowego przełącznika w Ustawieniach poza formą zwracania się (§2, §5); podsumowanie jest zawsze widoczne, gdy są fakty i język to PL. W wersji EN karta jest ukryta.

## 9. Wycena

Wycena z historii etapów (zmiana 2026-11-16, decyzja autora; zastępuje wycenę w godzinach pracy człowieka).

| Miara | Wartość | Podstawa |
|---|---|---|
| Rozmiar zmian | ok. 2 000–2 800 linii | E7 (raport tygodniowy): +1 910; hotfix beta 3: +1 022; E9 łącznie: ok. +2 900 |
| Czas agenta | ok. 2,5–3,5 h[^reczna] | w tym: Sesja 1–1,5 h, Bitwa 30–45 min, Tydzień 30–45 min |
| Mutacje (tylko nowa logika) | ok. 20–30 min | dodatkowo do czasu agenta |
| Czas autora | ok. 1–1,5 h | przeglądy, test ręczny, frazy |

[^reczna]: Dla porównania: praca ręczna ok. 35–46 h bez EN, 38–50 h z EN (dawna wycena tego speca z 2026-11-16, przed zmianą): Rust `recap.rs` z faktami, regułami i `seed` 8–10 h; frazy PL z wariantami i formami rodzaju 8–11 h; frazy EN 3–4 h; składanie tekstu i wybór wariantów 4–6 h; testy niezmienników z listą słów SJP.PL 7–9 h; UI na trzech ekranach 6–8 h; dokumentacja i przegląd 2 h. Spec 28 (LLM z doborem modelu) dla porównania: ok. 38–62 h, plus pobieranie modeli i nowe uprawnienia.

## 10. Kolejność realizacji (propozycja)

1. Sesja PL (fakty, frazy, testy niezmienników, karta na ekranie Sesja) — najwięcej danych i makieta.
2. Bitwa PL.
3. Tydzień PL.
4. ~~EN dla wszystkich trzech.~~ Poza tym wydaniem (zmiana 2026-11-16, §11 pyt. 2).

## 11. Rozstrzygnięte (decyzje autora, 2026-11-16)

Do 2026-11-16 ta sekcja nazywała się „Otwarte pytania”; pytania 1–5 i dwie dodatkowe decyzje rozstrzygnięte przez autora 2026-11-16.

1. **Forma rodzaju → ustawienie „Jak się do Ciebie zwracać”:** męska / żeńska / ~~bezosobowa~~ bez formy (neutralna, poprawka przed beta 4: bez „-no/-to”, pytanie na ekranie pierwszego uruchomienia), **domyślnie neutralna**. Obejmuje też istniejące teksty w 2. osobie (np. moment „Rozkręcałeś się”). Odmiana w kodzie z tematu czasownika; testy wszystkich trzech form (§2, §5).
2. **EN: nie w tym wydaniu.** W wersji EN karta podsumowania jest ukryta (§2, §8, §10).
3. **Tydzień przy „za mało bitew na trend”:** próg jak w E7 (< 15 bitew) → jedno zdanie „za mało bitew w tym tygodniu” (§4.3).
4. **Progi z A-3 bez zmian;** rewizja po 2 tygodniach używania (§4).
5. **Czas życia czołgu: bez faktu.** Wpis do backlogu „przy najbliższej zmianie schematu” (spec 20 §11.6) (§4.1).
6. **Zdanie oceniające z makiety → fakt bez oceny (D20)** (§8).
7. **§9: wycena z historii etapów:** ok. 2 000–2 800 linii, czas agenta ok. 2,5–3,5 h, autora ok. 1–1,5 h; dawne 35–46 h w przypisie jako „praca ręczna” (wpisane wcześniej tego dnia, `54d625b`).

Treść pytań sprzed decyzji: (1) forma rodzaju z domyślną męską czy frazy bez rodzaju; (2) EN w pierwszym wydaniu czy później; (3) przy „za mało bitew na trend” jedno zdanie czy brak karty; (4) progi z A-3 jak są czy strojone na danych autora; (5) pole czasu życia przy parsowaniu powtórki czy bez tego faktu.
