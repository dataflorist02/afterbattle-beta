# Makieta redesignu Afterbattle 2.0 (zaakceptowana 2026-11-11)

Źródło prawdy dla etapu E5 „Redesign UI”. Oryginał na płótnie: https://claude.ai/artifact/Wsv91yfaKq3Eh4u6bEUJAL

## Pliki

| Plik | Ekran |
|---|---|
| `Main.dc.html` | Sesja (ekran startowy) |
| `Czolgi.dc.html` | Czołg (lista czołgów i szczegóły) |
| `Bitwa.dc.html` | Szczegóły bitwy (w sekcji Historia) |
| `Cele.dc.html` | Cele tygodnia |
| `canvas.json` | układ płótna, tylko informacyjnie |

Pliki to format Design Component z płótna (HTML + `<helmet>` ze stylami + blok `<script type="text/x-dc">`). Nie renderują się samodzielnie poza płótnem (brak `support.js`). Czytaj je jako specyfikację: struktura DOM, inline style, klasy w `<helmet>`, kolejność treści, teksty po polsku.

## Makiety 1.0 RC (`rc/`, zaakceptowane 2026-11-17)

Statyczne kopie planszy „1.0 RC” z tego samego płótna: `rc/*.png` (podgląd) i `rc/*.html` (układ do odczytania): Przegląd, Czołg z porównaniem A-1, Sesja, rozwinięte podsumowanie, Ustawienia, pierwsze uruchomienie, okienko aktualizacji. Spis i rozstrzygnięcia po makietach w `rc/README.md`; gdzie makieta i spec 31/32 się różnią, wygrywa spec.

## Zasady, które makieta ustala

- Nawigacja: pionowy pasek z okrągłymi ikonami i podpisami: Sesja, Czołgi, Cele, Historia, Mapy (wyszarzone w buildzie prywatnym, ukryte w publicznym), Ustawienia na dole (prywatność w środku).
- Kolory: tło `#0A101C` z dwiema delikatnymi poświatami w tle (niebieska, pomarańczowa), karty `rgba(255,255,255,0.035)` z obramowaniem `rgba(255,255,255,0.07)`, promień 20 px. Tekst `#EEF1F6`, drugorzędny `#9AA6B8`, trzeciorzędny `#8693A6`.
- Jeden akcent: pomarańcz (`#F08A3C`, linki i wykres `#F3A36B`). Poświata i pomarańczowa ramka tylko na jednym kluczowym elemencie ekranu. Fiolet usunięty. Druga drużyna w porównaniach: stonowany niebieski `#6F87B8`.
- Typografia: liczby i nagłówki `Barlow Semi Condensed` (liczby w wadze 300, nagłówki 500), tekst `Inter`. Docelowo fonty dołączone lokalnie (CSP bez sieci), nie z Google Fonts.
- Bez pigułek, wersalików, nadtytułów, numeracji sekcji i kropek w tytułach. Kolor nigdy jedynym nośnikiem informacji („Wygrana”/„Porażka” słowami).
- Dostępność: prawdziwe `<a>`/`<button>`, cele kliknięć ≥ 44 px, `aria-label` na wykresach i elementach graficznych, kontrast ≥ 4,5:1.

## Miejsca na grafiki z gry

Ramki z przerywaną linią i podpisem „z plików gry” to miejsca na ikony czołgów i grafiki map, czytane w locie z instalacji gry (D19). W repo nigdy nie ma grafik WG; w testach i zrzutach do dokumentacji są zamienniki.

## Dane na makiecie

Średnia 2 702 obrażeń pochodzi z testu golden autora; pozostałe liczby są przykładowe. Ekran Czołg pokazuje porównanie z innymi graczami (tabele odniesienia, po v2.0); w v2.0 to samo miejsce porównuje ze średnią gracza na tym czołgu. Sekcja „Podsumowanie” w ekranie Bitwa jest poza v2.0 (§11.3 speca); miejsce w układzie zostaje.
