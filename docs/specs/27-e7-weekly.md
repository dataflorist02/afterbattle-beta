# Spec 27 — E7: raport tygodniowy (reguły, progi, teksty)

Zakres: spec 20 E7, wewnętrzna lista zadań autora dla E7–E8, makieta `docs/design/Cele.dc.html` w języku kierunku C (`docs/design/flare/README.md`). Moduł `src-tauri/src/weekly.rs`, komenda `get_weekly_report`. Tylko bitwy liczone (D4). Nic nie jest zapisywane: raport liczy się z bitew przy każdym zapytaniu.

## 1. Tydzień

- Tydzień ISO w czasie lokalnym: poniedziałek 00:00 do następnego poniedziałku 00:00 (koniec wyłącznie). Zmiana czasu w tygodniu nie przesuwa granic (test ze strefą z przejściem na czas letni).
- Raport: bieżący tydzień i 4 poprzednie.
- Pewność tygodnia: < 15 bitew — „za mało bitew na trend” (bez wskaźnika), 15–29 — wstępna, ≥ 30 — dobra.

## 2. Wskaźnik tygodnia

Dla obrażeń z asystą, przetrwania i XP (`original_xp`): suma wartości bitew tygodnia przez sumę średnich tych samych czołgów, w procentach (100% = jak zwykle). Średnia czołgu: jego ostatnie ≤ 100 bitew liczonych przed początkiem danego tygodnia, co najmniej 10; czołgi bez takiej średniej nie wchodzą. Przetrwanie: przeżyte bitwy wobec oczekiwanej liczby (suma odsetków przetrwania czołgów). WN8 tygodnia (tylko z danymi XVM): wartość WN8 bitew tygodnia i 4 poprzednich. Wygrane: tylko kontekst, nie cel (D12).

## 3. Cele tygodnia (katalog, D12)

Cele liczone dla tygodnia z jego 4 poprzednich tygodni; bez nowych typów wyzwań (to cele raportu, nie wyzwania na 5 bitew). Gdy 4 poprzednie tygodnie mają razem < 15 bitew — brak celów, komunikat „za mało bitew”. Wygrane nigdy nie są celem (test: typy celów i JSON).

| Cel | Warunek | Cel liczbowo | Wyjaśnienie | Ocena od |
|---|---|---|---|---|
| Przetrwanie | ≥ 2 z 4 tygodni ma ≥ 15 bitew | mediana tygodniowych odsetków przetrwania + 5 pkt (najwyżej 100%) | „Twoja mediana z 4 tygodni (X%) plus 5 punktów” | 10 bitew w tygodniu |
| Obrażenia z asystą na czołgu | najczęściej grany czołg 4 tygodni, ≥ 15 bitew na nim (remis — niższe `vehicleId`) | średnia na nim × 1,10, zaokrąglona | „Twoja średnia z 4 tygodni (X) plus 10%” | 5 bitew na tym czołgu |
| XP | ≥ 15 bitew z XP w 4 tygodniach | średnia × 1,10, zaokrąglona | „Twoja średnia z 4 tygodni (X) plus 10%” | 10 bitew w tygodniu |

Stan celu: za mało bitew (poniżej progu oceny), osiągnięty (≥ cel), blisko (≥ 95% celu), poniżej. Kolejność: przetrwanie, czołg, XP.

## 4. Poprzedni tydzień i wyzwania

Poprzedni tydzień oceniany tymi samymi regułami: cele z 4 tygodni przed nim, stan z jego bitew. Wyzwania na 5 bitew zakończone w bieżącym tygodniu: liczba ukończonych, nieudanych i porzuconych.

## 5. Teksty (PL / EN)

Nagłówek „Cele na ten tydzień” / “Goals for this week”; podtytuł z zakresem dat i liczbą bitew. „Ten tydzień wobec poprzednich czterech” / “This week against the previous four”. „Za mało bitew na trend (N z 15)”. „Poprzedni tydzień: K z M celów osiągnięte w N bitwach.”
