# 34 — Damast: sesja wykuta z bitew

Status: APPROVED (2026-11-21)
Etap: E14, gałąź `v2/e14-damast`, po E13 (Żar, spec 33)
Wydanie: wchodzi do 1.0.0 (decyzja autora z 21.11: Żar i Damast przed 1.0)
Makiety: płótno „Afterbattle 2.0 — redesign”, rząd „Moonshot: Damast” (Damast-Kuznia, Damast-Historia, Damast-Karta)

## 1. Cel

Każda zakończona sesja dostaje własną płytkę stali damasceńskiej. Wzór wynika wyłącznie z bitew tej sesji, więc z płytki da się odczytać przebieg sesji. Płytka nie ocenia gracza (D20): każda stal jest ładna, a różni się tylko barwą nalotu.

## 2. Mapowanie danych na wzór

| Dane | Na płytce |
|---|---|
| Kolejność bitew | Oś pozioma: bitwy od lewej do prawej, każda ma swoją strefę, a strefy płynnie przechodzą jedna w drugą |
| Wynik bitwy `r = 0,6 × WN8 / średnie WN8 gracza + 0,4 × obrażenia / średnie obrażenia gracza na tym tierze` | Nalot strefy: ciepły (słomkowy i brązowy) dla r > 1, chłodny (niebieski) dla r < 1; intensywność rośnie do ±30% |
| Wygrana | Jedno oczko we wzorze w strefie bitwy |
| Tier | Gęstość warstw w strefie (wyższy tier daje drobniejszy wzór) |
| Obrażenia wobec średniej na tierze | Amplituda fali w strefie |
| Najlepsza bitwa (najwyższe r) | Jedna iskra w pasku pod płytką (ekran sesji i karta) |

Zasady:
- **Tier nie podnosi oceny.** Wchodzi do oceny tylko pośrednio: obrażenia porównujemy ze średnią gracza na tym samym tierze, a WN8 sam uwzględnia czołg.
- **Wzór jest deterministyczny.** Ten sam zestaw bitew zawsze daje ten sam wzór; ziarno to suma obrażeń i WN8 sesji.
- **Temperatura sesji** to średnia z r minus 1. Progi bierzemy ze specu 33 (±5% to „jak zwykle”).
- **Średnie gracza** liczymy z ostatnich 100 bitew przed sesją, tak samo jak w trendach (spec 32).
- **Mało danych:** gdy na danym tierze gracz ma mniej niż 10 bitew, porównujemy obrażenia ze średnią ze wszystkich tierów.

## 3. Gdzie się pojawia

1. **Koniec sesji (Sesja):**
   - płytka wykuwa się po zamknięciu sesji (zgodnie z regułą końca sesji);
   - kolejne etapy: bitwy wlewają się po kolei jako rozżarzony metal, potem wykuwa się wzór, a na końcu stal stygnie do nalotu;
   - całość trwa około 4 s;
   - pod płytką: pasek bitew (kropka pełna przy wygranej, pusta przy porażce, plus tier); liczby: WN8 sesji, obrażenia, wygrane i średni tier; przyciski „Zapisz kartę” i „Do historii”.
2. **Historia → Sesje:**
   - siatka płytek;
   - pod każdą płytką pasek bitew bez iskry;
   - podpis: data, liczba bitew, temperatura, WN8;
   - gorący punkt tylko na rekordowej sesji.
3. **Karta 1200×630:**
   - eksport PNG przyciskiem „Zapisz kartę” do folderu, który wybiera użytkownik;
   - nic nie jest wysyłane;
   - bez nicku i ID konta.
4. **Stare sesje:** płytki powstają także dla sesji sprzed aktualizacji, ponieważ wzór liczy się z historii bitew.

Reguły bezpieczeństwa:
- Nic nie dzieje się w trakcie bitwy (Fair Play).
- Wykuwanie zaczyna się dopiero po zamknięciu sesji.

## 4. Renderowanie

- **Główna ścieżka: WebGL2 fragment shader.**
  - rozdzielczość pełna, z uwzględnieniem skalowania ekranu (devicePixelRatio);
  - 60 klatek na sekundę;
  - granice warstw wygładzone (smoothstep na krawędzi warstwy), więc nie ma schodków ani migotania (moiré).
- **Zapasowa ścieżka, gdy WebGL2 jest niedostępny albo działa programowo:**
  - ten sam algorytm w JS uruchamiany w Web Workerze;
  - wynikiem jest statyczny obraz bez animacji.
- **Historia:**
  - płytki liczone raz i zapisane w pamięci podręcznej jako obraz w katalogu danych aplikacji;
  - unieważnienie pamięci, gdy zmieni się wersja algorytmu albo dane sesji;
  - bez renderowania na żywo dla każdej płytki.
- **Ograniczony ruch:** od razu gotowa płytka, bez wykuwania.
- **Wyjątek od reguły ze specu 33** („animujemy tylko opacity i transform”): dotyczy wyłącznie płytki Damastu.
- **Kod:**
  - algorytm wydzielony w `src/features/damast/` (`stats.ts`, `pattern.ts`, `shader.glsl`, `worker.ts`);
  - parametry mają być zgodne z makietą Damast-Kuznia;
  - wersja algorytmu w stałej `DAMAST_VERSION`.

## 5. Odbiór

- [ ] Testy jednostkowe `stats.ts`, na syntetycznych sesjach: r, temperatura, najlepsza bitwa, zapasowa średnia przy mniej niż 10 bitwach na tierze.
- [ ] Test determinizmu: ta sama sesja daje identyczny hash obrazu z JS. Dwie różne sesje dają różne hashe.
- [ ] Testy porównawcze `pattern.ts` (wzorcowe obrazy w niskiej rozdzielczości, syntetyczne dane, w repozytorium).
- [ ] Brak WebGL2 włącza ścieżkę zapasową (test z atrapą kontekstu).
- [ ] Ograniczony ruch: od razu gotowa płytka (test).
- [ ] Golden (2013 bitew, digest `1703f8dbeb68775e`) bez zmian, bo Damast nie zmienia statystyk. Uruchamia autor.
- [ ] Eksport karty: PNG 1200×630, w pliku brak nicku i ID konta.
- [ ] Historia z 200 sesjami: przewijanie płynne, płytki z pamięci podręcznej.
- [ ] Test ręczny autora:
  - płytka na własnej sesji;
  - sprawdzenie na drugim, słabszym komputerze (ścieżka zapasowa lub WebGL);
  - skala ekranu 150%.
- [ ] Wszystkie kontrole z §0 przechodzą.

## 6. Poza zakresem

- Wysyłanie karty gdziekolwiek, galeria online.
- Płytki dla pojedynczych czołgów lub tygodni.
- Dźwięk kucia.

## 7. Szacunek

- Agent: 6–8 h.
- Autor: test ręczny i przegląd około 1 h.
- E13 (Żar) musi być scalony wcześniej, bo E14 korzysta z tokenów `--ember-*` i progów temperatury.
