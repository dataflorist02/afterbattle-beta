# Spec 28 — Podsumowania AI (lokalnie, na żądanie, dobór modelu do sprzętu)

- **Status: DRAFT — LLM odłożony (decyzja autora 2026-11-16, §10).** Pierwsza wersja podsumowań powstaje z szablonów i reguł, bez modelu: [spec 30](30-template-summaries.md). Ten spec zostaje jako plan na wypadek powrotu do LLM; nic z niego nie wchodzi do aplikacji. Rozwija spec 20 §11.3 (debrief z lokalnego LLM).
- Data: 2026-11-14. Autor decyzji: Szymon. Zapisane z rozmowy architektonicznej.
- **Zmiany 2026-11-15** (decyzje autora po biegu 1 sprintu odkrywczego; pomiary A-3 w notatkach badawczych autora, poza repozytorium publicznym): §3.2 poziomy i progi, jawny wybór urządzenia; §4 źródło plików Q4_K_M; §6 naprawa cytatów i nazwy faktów; §7 stan pomiarów. Każda zmiana oznaczona w swoim punkcie.
- **Zmiany 2026-11-16** (decyzje autora po iteracjach v3 i v4 badania A-3; pomiary w notatkach badawczych autora, poza repozytorium publicznym): status „LLM odłożony” i liczby, które o tym zdecydowały (§10); §6 weryfikator słownikowy i reguła sprzeczności z faktami tekstowymi; §7 stan. Bez zmian w kodzie aplikacji.

## 1. Cel
Krótkie podsumowanie sesji lub bitwy prostą polszczyzną, napisane lokalnie przez mały model (rodzina Bielik), w którym każde zdanie ma odnośnik do konkretnej liczby. Funkcja szanuje sprzęt gracza: sama dobiera model, nie zabiera karty grze i działa wyłącznie na prośbę.

## 2. Zasady nadrzędne
- **Wyłączone domyślnie.** Przełącznik w Ustawieniach „Podsumowania AI (lokalnie)”.
- **Tylko na żądanie.** Przycisk „Napisz podsumowanie” na ekranach Sesja i Bitwa; nic nie generuje się automatycznie. Przycisk „Przerwij” w trakcie. Gotowy tekst zapisany przy sesji/bitwie (bez ponownego liczenia).
- **Wyłącznie po bitwie, nic w trakcie gry** (Fair Play). Zero czytania pamięci gry i ruchu sieciowego.
- **Model nie liczy i nie ocenia.** Dostaje listę faktów z identyfikatorami (liczone w Ruście) i układa zdania. Bez jednej liczby „oceny gracza” i bez słów wpływ/impact/carry (D20).
- **Bez chmury.** Całość lokalnie.

## 3. Dobór modelu do sprzętu
### 3.1 Profil sprzętu (lokalnie, nic nie wychodzi z komputera)
- Wolny VRAM i budżet pamięci karty: DXGI (`IDXGIAdapter3::QueryVideoMemoryInfo`) przez crate `windows`, jeśli jest już w drzewie zależności (sprawdzić `cargo tree`); inaczej decyzja autora o zależności.
- RAM całkowity i wolny, liczba rdzeni, obsługa AVX2.
- Czy działa klient WoT: wyłącznie lista procesów (nazwa pliku wykonywalnego), bez dostępu do procesu.

### 3.2 Poziomy (progi po A-3, zmiana 2026-11-15)
| Poziom | Warunek | Model | Urządzenie |
|---|---|---|---|
| Niedostępne | RAM < 8 GB albo brak AVX2 | — | funkcja ukryta z wyjaśnieniem |
| Podstawowy | brak dyskretnej karty, < 2,5 GB wolnego VRAM albo gra działa | Bielik 1.5B Q4_K_M (ok. 1 GB) | CPU |
| Szybki | ≥ 2,5 GB wolnego VRAM i gra nie działa | Bielik 1.5B Q4_K_M | GPU (dyskretna karta, wybrana jawnie) |
| Dokładniejszy | ≥ 4,5 GB wolnego VRAM i gra nie działa | Bielik 4.5B Q4_K_M (ok. 2,9 GB; własna kwantyzacja z fp16, Apache 2.0, §4) | GPU (dyskretna karta, wybrana jawnie) |

**Zmiana 2026-11-15 (decyzja autora po A-3):**
- Podstawowy: 1.5B Q4_K_M zamiast 1.5B Q8 (A-3: 43 vs 32 tok/s na CPU, jakość w granicach szumu). Szybki: 1.5B Q4_K_M od 2,5 GB wolnego VRAM zamiast ok. 3 GB (szczyt w A-3: 1,1 GB przy kontekście 4096). Dokładniejszy bez zmian progu (4,5 GB; szczyt 4.5B Q4_K_M w A-3: 3,2 GB).
- **4.5B Q8 odrzucony** (ok. 5 GB VRAM w szczycie), mimo lepszego wyniku weryfikatora przed naprawą cytatów.
- **Przy działającej grze tylko CPU** (poziom Podstawowy), do czasu pomiaru FPS przez autora (procedura w notatkach A-3); dawny zapis „CPU albo mniejszy model” zastąpiony.
- **Wybór urządzenia Vulkan jawnie:** dyskretna karta (z profilu sprzętu §3.1), nigdy pierwsze urządzenie z listy. Na laptopie autora Vulkan0 to zintegrowany GPU, a dyskretna karta to Vulkan1.

- **Pomiar zamiast zgadywania:** po pobraniu modelu jednorazowy krótki test szybkości (tok/s) potwierdza poziom; wynik w Ustawieniach, ręczna zmiana możliwa.
- **Gdy gra działa:** generowanie tylko na CPU (zmiana 2026-11-15, wyżej); nigdy nie zajmujemy VRAM potrzebnej grze.
- **Zwolnienie pamięci:** model ładowany na żądanie, zwalniany po kilku minutach bezczynności i przy zamknięciu okna.

## 4. Pobieranie modelu (nowe połączenie sieciowe)
- Osobna zgoda: ekran z rozmiarem, źródłem (oficjalne repozytorium SpeakLeash na Hugging Face), licencją (Apache 2.0) i informacją, dokąd trafi plik.
- Adres i suma SHA-256 przypięte w kodzie dla każdej wersji modelu; weryfikacja po pobraniu, wznowienie przerwanego pobierania, limit rozmiaru.
- Plik w folderze danych aplikacji, przycisk „Usuń model”, kasowany przez „Usuń wszystkie dane”.
- Model nigdy w instalatorze ani w repo. Wpis w PRIVACY (PL/EN) i THREAT_MODEL, wzorem XVM.
- **Zmiana 2026-11-15 (decyzja autora): źródło plików Q4_K_M.** Oficjalne repozytoria SpeakLeash z GGUF (1.5B i 4.5B v3.0) mają tylko Q8_0 i fp16 (stan 2026-11-15). Pliki Q4_K_M kwantyzuje z fp16 i publikuje autor, zgodnie z Apache 2.0: plik licencji, NOTICE (pochodzenie: SpeakLeash, model bazowy i jego commit), opis zmian (kwantyzacja Q4_K_M z fp16 narzędziem `llama-quantize` z llama.cpp, wersja narzędzia i ustawienia) i SHA-256 każdego pliku. Ekran zgody (pierwszy punkt) pokazuje wtedy to źródło. Aplikacja przypina adres i sumę SHA-256 jak dla każdego modelu. **Przed wydaniem funkcji** sprawdzić, czy SpeakLeash wydał oficjalne Q4; jeśli tak, przejść na oficjalne pliki.

## 5. Uruchamianie
- `llama-server` (llama.cpp, MIT) jako sidecar Tauri, backend Vulkan (działa na kartach NVIDIA/AMD/Intel bez instalowania CUDA), wariant CPU jako zapas.
- Start tylko z Rusta, nasłuch na 127.0.0.1, losowy port, klucz na sesję, zamknięta lista argumentów; WebView bez dostępu do serwera.
- Nowe uprawnienie Tauri (shell sidecar) — minimalne, opisane w THREAT_MODEL.

## 6. Jakość i ugruntowanie
- Wyjście wymuszone schematem: tablica `{tekst, cytaty: [F1, …]}`, cytaty ograniczone do identyfikatorów z listy faktów (enum per zapytanie).
- Weryfikator w Ruście (bez modelu) odrzuca zdanie: bez cytatu, z liczbą spoza cytowanych faktów, z kierunkiem („więcej/mniej/lepiej”) sprzecznym ze znakiem różnicy. Odrzucone zdania nie trafiają do tekstu.
- **Zmiana 2026-11-15 (decyzja autora): naprawa cytatów przed weryfikacją.** W Ruście, bez modelu: liczba ze zdania, której nie ma w cytowanych faktach, dopisuje cytat **tylko wtedy, gdy pasuje do dokładnie jednego faktu** z listy zapytania. Gdy pasuje do kilku faktów albo do żadnego, zdanie jest odrzucane. Zdanie bez żadnego cytatu nie jest naprawiane. Potem działa weryfikator jak wyżej. (A-3: 95,5–99,7% przyjętych zdań po naprawie wobec 70–92% bez niej; modele niemal nie zmyślają liczb, tylko je źle cytują.)
- **Zmiana 2026-11-15 (decyzja autora): nazwy faktów jednoznaczne co do kierunku.** Każdy fakt o walce mówi, kto komu: np. „Obrażenia zadane przez Ciebie przeciwnikom”, „Twoje strzały / Twoje trafienia w przeciwników / Twoje przebicia”, „Obrażenia od przeciwników zatrzymane przez Twój pancerz”. Prompt wymaga, by każda liczba pochodziła z faktu zacytowanego w tym samym zdaniu, i ostrzega przed zamianą „zadane / otrzymane”.
- **Zmiana 2026-11-16 (decyzja autora): weryfikator słownikowy.** Zdanie jest odrzucane, gdy ma wyraz spoza słownika języka polskiego. Wyjątki: nazwy własne z faktów danego zapytania (słowa z nazw mapy i czołgu, cyfra rzymska tieru, klasa pojazdu) i zamknięta lista terminów gry (`tier`, `WN8`). Zdanie jest też odrzucane, gdy wyraz ze słownika różni się o jedną literę (wstawienie, usunięcie, zamiana) od terminu gry albo od słowa nazwy własnej z co najmniej 4 literami i nie jest formą tego samego hasła („tiger” obok „tier” jest odrzucany; „Doliny” obok „Dolina” przechodzi). Nic nie jest poprawiane, bez łatek typu „-łś” → „-łeś”. Ograniczenie: poprawne słowo w złym miejscu przechodzi.
  - Słownik: SJP.PL, wersja „odmiany słów” (każda forma fleksyjna osobno), wydanie 2026-11-01, z [sjp.pl/sl/odmiany](https://sjp.pl/sl/odmiany/). Licencje do wyboru: GPL 2, LGPL 2.1, Apache 2.0, CC BY 4.0 (strona i `README.txt` w paczce; sprawdzone 2026-11-16). Dla aplikacji: Apache 2.0 albo CC BY 4.0, z informacją o źródle.
  - `sjp-odm-20261101.zip`: 12 701 206 B, SHA-256 `d1837a8b9a44107694f57c04e4564e3baa0135f86f74c00786d71902e4ae99ad`. `odm.txt` z paczki: 71 570 874 B (237 457 haseł, 4 548 886 form), SHA-256 `8ba9ad794657b062841eaf384a423dd9f4ec83e6d59fce998cf80e1f271e08c1`.
  - Wynik A-3: bez fałszywych odrzuceń na 1096 zdaniach modeli z wcześniejszych pomiarów; odrzuca wszystkie zniekształcenia zauważone w ocenie ręcznej („rozegrałś”, „wygrałłeś”, „Rozgałęźyłeś”, „tiger VIII”).
- **Zmiana 2026-11-16 (decyzja autora): reguła sprzeczności z faktami tekstowymi.** Fakty bez liczby (wynik bitwy, przetrwanie czołgu) weryfikator liczb nie sprawdza. Zdanie bitwy, które mówi odwrotnie niż taki fakt („czołg przetrwał do końca” przy fakcie „czołg zniszczony”, „wygrana” przy „przegranej”), jest odrzucane niezależnie od cytatów. W A-3 to jedyne błędy treści, które w v4 przechodziły wszystkie pozostałe kontrole (model przepisywał zdanie ze wzorca).
- Pod tekstem: „Napisane lokalnie przez AI, każde zdanie ma odnośnik do liczby”; najechanie na odnośnik podświetla liczbę na ekranie.
- Miary (A-3 i testy regresji): odsetek przyjętych zdań (cel ≥ 95%), zgodność liczb (100% po weryfikatorze), pokrycie ważnych faktów, stabilność między przebiegami, ręczna ocena polszczyzny na 20–30 bitwach.

## 7. Do zmierzenia w sprincie odkrywczym (A-3) przed realizacją
1. Jakość polszczyzny i odsetek przyjętych zdań: 1.5B Q8 vs 4.5B Q4.
2. tok/s na GPU i CPU na komputerze autora (RTX 5070 Laptop 8 GB, Ryzen 9 AI 365).
3. Wpływ na FPS gry przy generowaniu obok uruchomionego klienta (GPU i CPU).
4. Czas pierwszego załadowania modelu i zwolnienia pamięci.
5. Kalibracja progów z §3.2.

**Stan 2026-11-15:** zmierzone na danych syntetycznych (notatki badawcze autora, poza repozytorium publicznym): pkt 1 w części „odsetek przyjętych zdań”, pkt 2, pkt 4 (ładowanie ciepłe i zwolnienie), pkt 5 (progi w §3.2). Do zrobienia przez autora: ręczna ocena polszczyzny (pkt 1), FPS gry przy generowaniu na GPU i CPU (pkt 3), zimne ładowanie po restarcie (pkt 4).

**Stan 2026-11-16:** pkt 1 zmierzony w części jakości tekstu (iteracje v3 i v4, §10). Pkt 3 i zimne ładowanie z pkt 4 przestają być pilne, bo LLM jest odłożony.

## 8. Szacunek
Debrief z weryfikatorem (spec 20 §11.3): ok. 30–50 h. Dobór modelu do sprzętu, pobieranie z weryfikacją i zarządzanie pamięcią: dodatkowo ok. 8–12 h.

## 9. Otwarte pytania
1. Czy funkcja ma być dostępna w publicznym buildzie od początku, czy najpierw tylko prywatnie?
2. Czy podsumowanie bitwy, sesji i tygodnia (E7) — wszystkie trzy, czy start od sesji?
3. Język podsumowania przy oknie EN: ten sam model (Bielik jest polski) czy inny model dla EN?

Pytania 1–3 dotyczą LLM i czekają na jego ewentualny powrót. Dla szablonów rozstrzyga je spec 30.

## 10. Decyzja 2026-11-16: LLM odłożony, pierwsza wersja z szablonów

**Decyzja autora (2026-11-16):** pierwsza wersja podsumowań powstaje z szablonów i reguł, bez modelu ([spec 30](30-template-summaries.md)). LLM jest odłożony. Ślepa ocena „LLM czy szablon” nie jest obowiązkowa, bo 75% zdań najlepszego wariantu LLM jest identycznych ze zdaniami szablonu.

**Liczby, które zdecydowały** (A-3, iteracje v3 i v4, 2026-11-16; 20 syntetycznych pozycji — 10 bitew i 10 sesji — × 3 przebiegi na dyskretnej karcie; notatki badawcze autora, poza repozytorium publicznym):

1. **Fakty jako gotowe frazy i reguły wyboru faktów** (v3) usunęły zdania bez sensu: zdania o faktach zbędnych w danej bitwie spadły z 15–24% do 0–1,3%, podsumowania z powtórzonym sensem z 33–47% do 2–23%. Odmiana liczebników trafiła do kodu.
2. **Styl osobowy psuł formy czasowników** („rozegrałś”, „wygrałłeś”, „Rozgałęźyłeś” zamiast „rozegrałeś”): 28–31% zdań 1.5B (Q4_K_M i Q8_0, więc nie kwantyzacja) i 11% zdań 4.5B Q4_K_M. Po weryfikatorze słownikowym całe podsumowania przechodziły w 3–15% (1.5B) i 57% (4.5B).
3. **Styl bezosobowy** (frazy i wzorce bez czasowników w 2. osobie) usunął zniekształcenia całkowicie: 99,6–100% zdań przyjętych po naprawie cytatów i słowniku. Najlepszy wariant, 4.5B Q4_K_M: 98,3% podsumowań bez żadnego odrzucenia, 0 powtórzeń sensu, 0 sprzeczności, ok. 2,9 s na podsumowanie, 3,1 GB VRAM. Warianty 1.5B w 2 z 30 bitew przepisywały ze wzorca „czołg przetrwał do końca” przy zniszczonym czołgu.
4. **Szablon bez modelu** (te same frazy i ten sam wybór faktów, kolejność zdań ustalona w kodzie): 100% zdań przyjętych przez weryfikator i słownik, pełne pokrycie ważnych faktów, 0 powtórzeń sensu, 0 sprzeczności, poniżej 1 ms na podsumowanie, bez pobierania i bez VRAM.
5. **LLM nie dawał nic ponad szablon:** 198 z 265 zdań najlepszego wariantu (75%) było identycznych ze zdaniami szablonu, a 6 z 60 podsumowań w całości. Podobieństwo tekstu do szablonu wynosiło 0,82. Różnice to skrócone odniesienie („niż średnia”), inny dobór faktów i inne łączenie zdań; wszystko to da się zrobić w szablonie (spec 30 §5).

**Co zostaje z tego speca na wypadek powrotu LLM:** dobór modelu do sprzętu (§3), pobieranie (§4), uruchamianie (§5) i weryfikator (§6) razem z regułami z 2026-11-16: słownikiem i sprzecznościami. Te dwie reguły i lista faktów z frazami przechodzą też do spec 30 jako testy niezmienników szablonu.
