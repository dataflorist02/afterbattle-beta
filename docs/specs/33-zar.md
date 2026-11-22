# Spec 33 — „Żar”: jeden język ruchu i temperatury, mniej tekstu na ekranach

- **Status: APPROVED (2026-11-21).** Etap E13, gałąź `v2/e13-zar`. Wchodzi do 1.0.0 (decyzja autora z 21.11: Żar i Damast przed 1.0; daty z 22.11: rc.3 23.11, 1.0.0 ok. 30.11 z tego samego commita, po 7 dniach od rc.3 bez poważnego błędu — utrata danych, crash, złe liczby; dla tego wydania zamiast dwóch tygodni ze spec 32 §7). Po nim E14 Damast (spec 34).
- Data: 2026-11-21. Autor decyzji: Szymon.
- Rozwija: spec 29 (marka, ruch), spec 31 (tokeny `--motion-*`, lista ruchu), spec 30 i 32 (podsumowania, Przegląd, A-1).
- Makiety i prototyp: płótno „Afterbattle 2.0 — redesign”, rząd „Wow: pięć kierunków wyróżnika”: `Zar-Ruch` (prototyp interaktywny), `Zar-Ekrany` (system na wszystkich ekranach), `Flair-5` (kierunki 1 i 3 wybrane jako baza).

## 1. Zasady

1. **Temperatura.** Jedna skala na całą aplikację: stalowy błękit (chłodniej niż Twoja średnia), stal (jak zwykle), żar (cieplej). Zawsze wobec własnej średniej gracza, nigdy wobec innych graczy. Kierunek, nie ocena (D20): bez dymu, bez czerwieni, bez słów oceniających.
2. **Jeden gorący punkt na ekran.** Naraz żarzy się najwyżej jedno miejsce: to, co najważniejsze albo co właśnie się stało.
3. **Smuga.** Każda nowa rzecz (bitwa, wiersz, ukończony cel) pojawia się z jednym przelotem smugi i stygnącą krawędzią.
4. **Nic w trakcie bitwy.** Fair Play bez zmian.
5. **Ograniczony ruch.** Bez animacji i iskier; temperatura zostaje jako barwa, gorący punkt jako stała obwódka.
6. **Pokazuj, nie tłumacz.** Na ekranach tylko nazwy, liczby i jednostki. Wyjaśnienia „jak to liczymy” są w Ustawieniach → O aplikacji → „Jak to działa” (§4). Przejrzystość zostaje, ale w jednym miejscu. Objaśnienia na żądanie (przycisk ⓘ, „?”) są dozwolone, a ekran domyślnie zostaje bez zdań objaśniających (decyzja autora z 22.11).

## 2. Tokeny (`src/styles/tokens.css`)

- Kolory: `--ember-hot`, `--ember-core`, `--ember-glow`, `--steel`, `--frost` (z palety spec 29; frost = dzisiejsze `--down`).
- Czasy: `--ember-heat` (1600 ms, rozgrzanie i stygnięcie liczby), `--ember-sweep` (520 ms, smuga), `--ember-breathe` (4200 ms, oddech gorącego punktu), `--ember-flash` (1400 ms, ukończenie celu).
- Progi temperatury (wobec średniej gracza, dla danego kontekstu): chłodno ≤ −5%, jak zwykle −5…+5%, ciepło ≥ +5%; natężenie liniowo do ±30% (powyżej bez zmian). Iskry tylko na karcie sesji: od 3 przy +5% do 18 przy +30%.
- **Gorący punkt bez obwódki (zmiana autora z 22.11, po teście ręcznym; zastępuje „stałą obwódkę” z §1 p. 5 i E13).** Gorący punkt świeci treścią: główna liczba w `--ember-core` z miękką poświatą tekstu; oddycha tylko ta poświata (`opacity` jej warstwy, `--ember-breathe`), przy ograniczonym ruchu stoi. Kafel albo wiersz może dostać jedynie ciepły gradient od dołu (radialny, niskie krycie), bez ramki: bez `border`, `outline` ani `box-shadow` w kształcie ramki. Ramka zostaje wyłącznie jako pierścień fokusu klawiatury (`:focus-visible`). Dotyczy całej aplikacji: blok „najlepsza bitwa” w Przeglądzie, kafel najlepszej bitwy na Sesji, rekord w Historii (wiersz i płytka rekordowej sesji), najlepsza mapa na Czołgach, wynik na ekranie Bitwa, cel najbliżej ukończenia; iskra pod płytką Damastu zostaje kropką z poświatą. Zasada „jeden gorący punkt na ekran” bez zmian.
- Wydajność: animowane tylko `opacity` i `transform` na warstwach (`::before/::after`); bez animowania `box-shadow` i `text-shadow` w pętli (prototyp robi to dla szybkości, implementacja nie).

## 3. Ekrany

| Ekran | Temperatura | Gorący punkt | Smuga |
|---|---|---|---|
| Sesja | tło karty liczb wg serii, iskry | najlepsza bitwa sesji (oddech) | nowy kafelek bitwy |
| Przegląd | karta ostatniej sesji jak Sesja | najlepsza bitwa na końcu karty | — |
| Historia | termogram: pasek temperatury przy każdej sesji | rekord w widocznym zakresie | nowy wiersz |
| Czołgi | kropka temperatury z ostatnich 10 bitew; linia wykresu nagrzewa się nad średnią | najlepsza mapa na czołgu | — |
| Bitwa | jednorazowe rozgrzanie lub schłodzenie wobec średniej na czołgu | wynik, gdy najlepsza w sesji | — |
| Cele | paski od stali do żaru wg postępu; ukończenie = błysk | cel najbliżej ukończenia | ukończony cel |
| Nawigacja | — | kropka żaru przy Sesji, gdy sesja trwa | — |
| Ustawienia | celowo chłodne; rozbłysk przycisku i „Zapisano” | — | — |
| Ładowanie | bez zmian (żar w logo) | — | — |

Liczby po zmianie: count-up 600 ms (bez zmian), potem rozgrzanie i stygnięcie (`--ember-heat`).

## 4. Mniej tekstu na ekranach, „Jak to działa” w Ustawieniach

- Przegląd tekstów wszystkich ekranów: usunąć zdania objaśniające, zostawić etykiety, liczby, jednostki i krótkie podpisy okna („ostatnie 100 bitew”). Przykłady do usunięcia lub skrócenia: „Percentyl Twojej mediany obrażeń w grupie tier × klasa. 50 = środek graczy.”; „Najwyższy percentyl mediany obrażeń czołgu na tle graczy…, od 10 bitew.”; podpisy „Kafelki: grafiki map…”, „Znacznik: Twoja mediana ze wszystkich…”. Legenda A-1 zostaje jako znaki (kropka, kreska, pasek), bez zdań.
- Stany puste i błędy zostają z jednym zdaniem „co zrobić” (to instrukcja, nie objaśnienie).
- Nowa podsekcja Ustawienia → O aplikacji → **„Jak to działa”**: krótkie hasła rozwijane po kliknięciu: które bitwy się liczą, średnia odniesienia, WN8 i XVM, porównanie z graczami (A-1, progi 200/10), podsumowania (szablony, odnośniki), temperatura (skala i progi), prywatność (co zostaje na komputerze, aktualizacje).
- Przy liczbie, która może budzić pytanie, wolno dać ikonę „?” prowadzącą prosto do właściwego hasła w „Jak to działa” (zamiast zdania obok).
- Testy: lista dozwolonych tekstów objaśniających na ekranach (pusta poza stanami pustymi i błędami); każde hasło „Jak to działa” ma odpowiadający test treści (liczby progów zgodne ze stałymi w kodzie).

## 5. Akceptacja

- Kontrole z §0; golden bez zmian (2013 / `1703f8dbeb68775e`): spec nie zmienia obliczeń.
- Zrzuty każdego ekranu w trzech temperaturach (chłodno, jak zwykle, ciepło) i z ograniczonym ruchem.
- Pomiar: przejścia i reakcje bez pogorszenia wobec tabeli E11; brak animacji w pętli poza jednym oddechem gorącego punktu.
- Test ręczny autora: sesja na żywo (nowe bitwy, rekord, cel), Historia z termogramem, „Jak to działa”.

## 6. Wycena (z historii etapów)

Ok. 2 500–3 500 linii, 3–4 h pracy agenta w 3 etapach (tokeny i Sesja/Przegląd; Historia, Czołgi, Bitwa, Cele; teksty i „Jak to działa”), ok. 1 h przeglądu autora.
