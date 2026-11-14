# Spec 26 — E5-flare: kierunek C z mapami (reguły, progi, teksty)

Zadanie: wewnętrzna lista zadań autora dla E5-flare. Makiety: `docs/design/flare/`. Ten spec ustala wyłącznie reguły i progi, których makiety nie mówią; wygląd wynika z makiet i `docs/design/README.md`.

## 1. Grafiki map (§0)

- Rozpoznanie na kliencie autora (2026-11-14, test `game_folder_map_images_report`): `gui/maps/icons/map/stats/` 58 PNG 500×235 (155–325 kB), `gui/maps/icons/map/` 67 PNG 300×300 (93–239 kB), `map/battleloading/` 60 PNG 230×81, `map/small/` 59 PNG 100×60, `map/screen/` 64 DDS (do 2,7 MB); wszystkie wpisy `Stored`; klucze = `mapKey` (`NN_nazwa`).
- Czytnik: najpierw `map/stats/` (największy obsługiwany wariant), zapas `map/` dla map spoza `stats`. DDS nie są czytane (wymagałyby dekodera). Limity czytnika bez zmian: 1 MiB na obraz, RAM LRU 512 obrazów / 32 MiB w backendzie.
- Okno: kafelki dostają obraz przeskalowany do rozmiaru kafelka (×2 na ekrany o dużej gęstości) przez `createImageBitmap` i płótno; w pamięci okna trzymane są tylko przeskalowane obrazy (LRU 256 adresów `blob:`). Obrazy ładowane leniwie (tylko widoczne, `IntersectionObserver`).

## 2. Nagłówek-zdanie sesji (§2, `session_story.rs`)

Baza odniesienia jak w karcie głównej i wnioskach (E5-ui §4): czołgi sesji z ≥ 30 bitwami odniesienia, wagi = bitwy sesji z obrażeniami, łącznie ≥ 3 bitwy.

| Warunek | Rodzaj | PL | EN |
|---|---|---|---|
| zmiana ≥ +10% | `strong` | „Mocna sesja: obrażenia o X% wyżej niż zwykle” | “Strong session: damage X% above your usual” |
| zmiana ≤ −10% | `tough` | „Trudna sesja: obrażenia o X% niżej niż zwykle” | “Tough session: damage X% below your usual” |
| \|zmiana\| < 10% | `calm` | „Spokojna sesja: wyniki blisko Twojej średniej” | “Calm session: results close to your average” |
| brak bazy | `neutral` | „Sesja: N bitew losowych” | “Session: N random battles” |

Progi 10% liczone z dokładnością do milionowej (granica włącznie).

## 3. Momenty sesji (§2)

Tylko bitwy liczone (D4). Karta znika, gdy reguła nie ma danych.

1. **Trend obrażeń:** ≥ 6 bitew z obrażeniami; średnia ostatniej trzeciej części wobec pierwszej (trzecia część = ⌊n/3⌋); pokazany przy zmianie ≥ 20% (granica włącznie). Liczby na karcie (poprawka E5-flare-fix, 2026-11-14):
   - gdy każda bitwa ma więcej (mniej) obrażeń niż poprzednia — obrażenia **pierwszej i ostatniej bitwy sesji** (te same co na pierwszym i ostatnim kafelku pasa), podpis „Każda kolejna bitwa z większymi (mniejszymi) obrażeniami niż poprzednia”;
   - w przeciwnym razie — **średnie** z pierwszej i ostatniej trzeciej części, podpis mówi to wprost: „Średnio z pierwszych N bitew → z ostatnich N”.
2. **Do poprawy:** najsilniejszy negatywny sygnał wniosków sesji (D7: 30 bitew odniesienia, 3 bitwy sesji na czołgu, progi z `session_insights.rs`); remis — niższe `vehicleId`. Tło: mapa najsłabszej bitwy tego czołgu (dla przetrwania: zniszczonej z najmniejszymi obrażeniami).
3. **Najlepsza bitwa:** najwyższe obrażenia z ≥ 2 bitew; remis — wcześniejsza. Jej mapa jest tłem nagłówka sesji, jej kafelek ma ramkę akcentu.

## 4. Pas „Sesja bitwa po bitwie”

Kafelek na bitwę (chronologicznie), stała szerokość, przewijanie poziome w jednym pasie (przy 9 bitwach w szerokim oknie mieści się bez przewijania). Uzasadnienie: kolejność czasu czytelna w jednej linii, wysokość ekranu stała przy 30 bitwach.

## 5. Czołg — mapy (§3)

Mapy z ≥ 4 bitwami liczonymi (wcześniej 5), od najwyższej średniej obrażeń. „Najlepiej”: dwie pierwsze (gdy map ≥ 3; przy 1–2 mapach tylko pierwsza), „Najsłabiej”: ostatnia (gdy map ≥ 3). Pasek: średnia mapy wobec średniej gracza na czołgu (100% = 1,25 × średnia), kolor akcentu, gdy ≥ średnia.

## 6. Bitwa — „Ty na <mapie>” (§4)

Wszystkie bitwy liczone gracza na mapie, każdym czołgiem: liczba, % wygranych (z bitew o znanym wyniku; remis to nie wygrana), średnie obrażenia. Klucz mapy spoza wzorca `[a-z0-9_]{1,128}` daje kartę bez bitew. Link do Historii z filtrem tej mapy.

## 7. Historia (§5)

Nagłówki dni: „Dziś”, „Wczoraj”, data. Przetrwanie słownie: „Przeżył” / „Zniszczony” (EN “Survived” / “Destroyed”). Karta sesji: do 4 różnych map, od najnowszej bitwy (`get_session_maps`, najwyżej 50 sesji na zapytanie).

## 8. Ruch (§7a)

Tokeny: `--motion-fast` 100 ms, `--motion-base` 200 ms, `--motion-enter` 250 ms, `--motion-count` 600 ms; `--ease-out`, `--ease-in`. Tylko `transform` i `opacity`; nic się nie zapętla; `prefers-reduced-motion: reduce` wyłącza ruch (zostają zmiany koloru i przezroczystości). Odliczanie liczby głównej tylko, gdy w trwającej sesji przybyła nowa bitwa.

Poprawka E5-flare-fix2 (2026-11-14): Historia nie ma wejścia całego ekranu (`screen-still`); kaskada tylko dla pierwszych 10 wierszy pierwszej strony przy pierwszym wejściu (`--motion-row` 320 ms, przesunięcie 6 px, `--ease-out`, odstęp 36 ms, ostatni start po 324 ms); wiersze niżej, doładowane, po zmianie filtra, po przełączeniu Bitwy/Sesje i po powrocie z ekranu Bitwa — bez ruchu. Powrót z ekranu Bitwa na dowolny ekran — bez wejścia ekranu.

**Zmiana E8-ui-fix (decyzja autora, 2026-11-14) — reguła wejścia list i kafelków:**

- **Fala** obejmuje wiersze widoczne przy pierwszym wyświetleniu: `ceil(wysokość obszaru listy / wysokość wiersza)`, najwyżej 20 (na 2560×1440 ok. 14). Obszar listy liczy się od górnej krawędzi pierwszego wiersza do dołu okna, wysokość wiersza — jako odstęp między pierwszymi dwoma wierszami (albo wysokość pierwszego). Pomiar przed pierwszym malowaniem; bez pomiaru (0) — bez ruchu. Wiersze niżej — bez animacji. Zastępuje stałe „pierwsze 10 wierszy” z E5-flare-fix2.
- **Tempo** jak w Historii: `--motion-row` 320 ms, przesunięcie 6 px, `--ease-out`, odstęp 36 ms między kolejnymi elementami fali.
- **Start dopiero po wczytaniu danych**, tylko przy pierwszym wejściu na ekran; stan ładowania nie ma ruchu (ekran nie wjeżdża: `screen-still` także dla Czołgów). Zmiana filtra, wybór innego czołgu, doładowanie, przełączenie zakładki, powrót z ekranu Bitwa — bez ruchu.
- **Czołgi:** kafelki map wchodzą falą (najwyżej 20), paski kafelków i linia wykresu rysują się raz, razem z falą (`--motion-row`); lista czołgów i tabela bitew bez animacji.
- Wyłącznie `transform` i `opacity`; `prefers-reduced-motion: reduce` — żadnej fali ani rysowania.

## 9. Pasek nawigacji (E5-flare-fix2)

Na desktopie `position: sticky`, `top: 0`, wysokość okna; Ustawienia na dole paska zawsze widoczne; własne przewijanie paska tylko, gdy okno jest niższe niż jego zawartość. Układ wąski (≤ 760 px): pasek jako rząd nad treścią, bez przyklejania.
