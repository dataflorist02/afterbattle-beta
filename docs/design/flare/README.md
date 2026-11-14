# Kierunek C z mapami (zaakceptowany 2026-11-14)

Uzupełnienie docs/design/README.md. Oryginał: https://claude.ai/artifact/Wsv91yfaKq3Eh4u6bEUJAL (rzędy 3 i 4).

| Plik | Ekran |
|---|---|
| `Mapy-Sesja.dc.html` | Sesja: nagłówek na tle mapy najlepszej bitwy, pas „Sesja bitwa po bitwie”, momenty sesji |
| `Mapy-Czolg.dc.html` | Czołg: kompaktowy nagłówek, siatka „Twoje mapy na tym czołgu” |
| `Mapy-Bitwa.dc.html` | Bitwa: duży nagłówek na tle mapy, karta „Ty na <mapie>” |
| `Mapy-Historia.dc.html` | Historia: wiersze z pasem grafiki mapy, nagłówki dni |
| `Fonty.dc.html` | Porównanie krojów; wybrany nr 2: Big Shoulders + Inter |

Zasady kierunku:
- Grafiki map z plików gry to główny nośnik charakteru: tła nagłówków, kafelki bitew i map, pasy w wierszach. Zawsze z przyciemnieniem, żeby tekst miał kontrast ≥ 4,5:1.
- Krój: Big Shoulders (300 liczby, 500–600 nagłówki) + Inter dla tekstu.
- Akcent nadal jeden (pomarańcz): ramka i poświata tylko na najlepszym elemencie (najlepsza bitwa, najlepsze mapy).
- Porażki przygaszone i zawsze podpisane słowem.
- Gradienty w makietach to zamienniki grafik; w aplikacji zastępują je prawdziwe grafiki, a gradienty zostają jako zamiennik, gdy plików gry brak.
