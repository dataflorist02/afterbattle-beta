# Spec 28 — Podsumowania AI (lokalnie, na żądanie, dobór modelu do sprzętu)

- **Status: DRAFT (wizja po 2.0).** Rozwija spec 20 §11.3 (debrief z lokalnego LLM). Nie wchodzi do 2.0.0. Realizacja dopiero po sprincie odkrywczym (krok A-3) i decyzji autora.
- Data: 2026-11-14. Autor decyzji: Szymon. Zapisane z rozmowy architektonicznej.
- **Zmiany 2026-11-15** (decyzje autora po biegu 1 sprintu odkrywczego; pomiary A-3 w notatkach badawczych autora, poza repozytorium publicznym): §3.2 poziomy i progi, jawny wybór urządzenia; §4 źródło plików Q4_K_M; §6 naprawa cytatów i nazwy faktów; §7 stan pomiarów. Każda zmiana oznaczona w swoim punkcie.

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
- Pod tekstem: „Napisane lokalnie przez AI, każde zdanie ma odnośnik do liczby”; najechanie na odnośnik podświetla liczbę na ekranie.
- Miary (A-3 i testy regresji): odsetek przyjętych zdań (cel ≥ 95%), zgodność liczb (100% po weryfikatorze), pokrycie ważnych faktów, stabilność między przebiegami, ręczna ocena polszczyzny na 20–30 bitwach.

## 7. Do zmierzenia w sprincie odkrywczym (A-3) przed realizacją
1. Jakość polszczyzny i odsetek przyjętych zdań: 1.5B Q8 vs 4.5B Q4.
2. tok/s na GPU i CPU na komputerze autora (RTX 5070 Laptop 8 GB, Ryzen 9 AI 365).
3. Wpływ na FPS gry przy generowaniu obok uruchomionego klienta (GPU i CPU).
4. Czas pierwszego załadowania modelu i zwolnienia pamięci.
5. Kalibracja progów z §3.2.

**Stan 2026-11-15:** zmierzone na danych syntetycznych (notatki badawcze autora, poza repozytorium publicznym): pkt 1 w części „odsetek przyjętych zdań”, pkt 2, pkt 4 (ładowanie ciepłe i zwolnienie), pkt 5 (progi w §3.2). Do zrobienia przez autora: ręczna ocena polszczyzny (pkt 1), FPS gry przy generowaniu na GPU i CPU (pkt 3), zimne ładowanie po restarcie (pkt 4).

## 8. Szacunek
Debrief z weryfikatorem (spec 20 §11.3): ok. 30–50 h. Dobór modelu do sprzętu, pobieranie z weryfikacją i zarządzanie pamięcią: dodatkowo ok. 8–12 h.

## 9. Otwarte pytania
1. Czy funkcja ma być dostępna w publicznym buildzie od początku, czy najpierw tylko prywatnie?
2. Czy podsumowanie bitwy, sesji i tygodnia (E7) — wszystkie trzy, czy start od sesji?
3. Język podsumowania przy oknie EN: ten sam model (Bielik jest polski) czy inny model dla EN?
