# Makiety 1.0 RC (zaakceptowane 2026-11-17)

Źródło: płótno „Afterbattle 2.0 — redesign”, rząd „1.0 RC: ostatni szlif i nowe ekrany” (adres płótna w `docs/design/README.md`). Tu statyczne kopie dla wykonawcy: `*.png` (podgląd) i `*.html` (układ, odstępy, kolory, rozmiary krojów do odczytania).

To są **wzorce układu, nie kod do skopiowania**: implementacja korzysta z tokenów (`src/styles/tokens.css`), istniejących komponentów i danych z backendu. Dane są syntetyczne. Grafiki map i ikony czołgów to zamienniki (gradient z podpisem „z plików gry”, prosta sylwetka); w aplikacji zostają prawdziwe grafiki z folderu gry (D19).

| Plik | Ekran | Spec |
|---|---|---|
| `RC-Przeglad` | Przegląd (start bez trwającej sesji) | 32 §4 |
| `RC-Czolg` | Czołg z porównaniem A-1 | 32 §3 |
| `RC-Sesja` | Sesja, podsumowanie zwinięte | 31 §3 |
| `RC-Podsumowanie` | Podsumowanie rozwinięte | 31 §3 |
| `RC-Ustawienia` | Ustawienia w pięciu grupach | 31 §4 |
| `RC-Start` | Pierwsze uruchomienie (folder, forma, aktualizacje) | 31, 32 §5.1 |
| `RC-Aktualizacja` | Okienko „Dostępna wersja” | 32 §5.2 |

Gdzie makieta i spec się różnią, wygrywa spec. Rozstrzygnięcia po makietach (2026-11-17):
- Podsumowanie rozwinięte: zdania bez numerów.
- Ustawienia: bez „Eksportu anonimowej sesji” i „Licencji” (lista „po 1.0.0”); „Usuń dane” bez czerwonej linii, czerwony tylko przycisk.
- Przegląd: trendy w oknie 100 bitew; „najlepsze czołgi” = percentyl mediany obrażeń czołgu w jego grupie tier × klasa (od 10 bitew, grupy ≥ 200 wyników).
- Liczby w zdaniach i w nawiasach bez łamania wewnątrz liczby (spacja nierozdzielająca, np. „(3 021)”).
- Ruch: lista w spec 31 §2 (makiety są statyczne).

Rozstrzygnięcia po STOP 2 (2026-11-18):
- Bez automatycznego przejścia na Sesję w trakcie pracy; ekran startowy wybierany tylko przy uruchomieniu (spec 32 §4.1). Zdanie „Po pierwszej bitwie nowej sesji aplikacja sama przejdzie na ekran Sesja” usunięte z `RC-Przeglad.html`, `RC-Przeglad.png` przegenerowany z HTML (offline, fonty z `src/assets/fonts`).
