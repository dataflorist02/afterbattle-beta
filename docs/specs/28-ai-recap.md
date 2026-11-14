# Spec 28 — Podsumowania AI (lokalnie, na żądanie, dobór modelu do sprzętu)

- **Status: DRAFT (wizja po 2.0).** Rozwija spec 20 §11.3 (debrief z lokalnego LLM). Nie wchodzi do 2.0.0. Realizacja dopiero po sprincie odkrywczym (krok A-3) i decyzji autora.
- Data: 2026-11-14. Autor decyzji: Szymon. Zapisane z rozmowy architektonicznej.

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

### 3.2 Poziomy (progi do kalibracji w A-3)
| Poziom | Warunek | Model | Urządzenie |
|---|---|---|---|
| Niedostępne | RAM < 8 GB albo brak AVX2 | — | funkcja ukryta z wyjaśnieniem |
| Podstawowy | brak karty lub < ok. 3 GB wolnego VRAM | Bielik 1.5B (Q8, ok. 1,7 GB) | CPU |
| Szybki | ≥ ok. 3 GB wolnego VRAM | Bielik 1.5B | GPU |
| Dokładniejszy | ≥ ok. 4,5 GB wolnego VRAM i gra nie działa | Bielik 4.5B Q4 (ok. 2,8 GB; własna kwantyzacja z fp16, Apache 2.0) | GPU |

- **Pomiar zamiast zgadywania:** po pobraniu modelu jednorazowy krótki test szybkości (tok/s) potwierdza poziom; wynik w Ustawieniach, ręczna zmiana możliwa.
- **Gdy gra działa:** generowanie na CPU albo mniejszym modelem; nigdy nie zajmujemy VRAM potrzebnej grze.
- **Zwolnienie pamięci:** model ładowany na żądanie, zwalniany po kilku minutach bezczynności i przy zamknięciu okna.

## 4. Pobieranie modelu (nowe połączenie sieciowe)
- Osobna zgoda: ekran z rozmiarem, źródłem (oficjalne repozytorium SpeakLeash na Hugging Face), licencją (Apache 2.0) i informacją, dokąd trafi plik.
- Adres i suma SHA-256 przypięte w kodzie dla każdej wersji modelu; weryfikacja po pobraniu, wznowienie przerwanego pobierania, limit rozmiaru.
- Plik w folderze danych aplikacji, przycisk „Usuń model”, kasowany przez „Usuń wszystkie dane”.
- Model nigdy w instalatorze ani w repo. Wpis w PRIVACY (PL/EN) i THREAT_MODEL, wzorem XVM.

## 5. Uruchamianie
- `llama-server` (llama.cpp, MIT) jako sidecar Tauri, backend Vulkan (działa na kartach NVIDIA/AMD/Intel bez instalowania CUDA), wariant CPU jako zapas.
- Start tylko z Rusta, nasłuch na 127.0.0.1, losowy port, klucz na sesję, zamknięta lista argumentów; WebView bez dostępu do serwera.
- Nowe uprawnienie Tauri (shell sidecar) — minimalne, opisane w THREAT_MODEL.

## 6. Jakość i ugruntowanie
- Wyjście wymuszone schematem: tablica `{tekst, cytaty: [F1, …]}`, cytaty ograniczone do identyfikatorów z listy faktów (enum per zapytanie).
- Weryfikator w Ruście (bez modelu) odrzuca zdanie: bez cytatu, z liczbą spoza cytowanych faktów, z kierunkiem („więcej/mniej/lepiej”) sprzecznym ze znakiem różnicy. Odrzucone zdania nie trafiają do tekstu.
- Pod tekstem: „Napisane lokalnie przez AI, każde zdanie ma odnośnik do liczby”; najechanie na odnośnik podświetla liczbę na ekranie.
- Miary (A-3 i testy regresji): odsetek przyjętych zdań (cel ≥ 95%), zgodność liczb (100% po weryfikatorze), pokrycie ważnych faktów, stabilność między przebiegami, ręczna ocena polszczyzny na 20–30 bitwach.

## 7. Do zmierzenia w sprincie odkrywczym (A-3) przed realizacją
1. Jakość polszczyzny i odsetek przyjętych zdań: 1.5B Q8 vs 4.5B Q4.
2. tok/s na GPU i CPU na komputerze autora (RTX 5070 Laptop 8 GB, Ryzen 9 AI 365).
3. Wpływ na FPS gry przy generowaniu obok uruchomionego klienta (GPU i CPU).
4. Czas pierwszego załadowania modelu i zwolnienia pamięci.
5. Kalibracja progów z §3.2.

## 8. Szacunek
Debrief z weryfikatorem (spec 20 §11.3): ok. 30–50 h. Dobór modelu do sprzętu, pobieranie z weryfikacją i zarządzanie pamięcią: dodatkowo ok. 8–12 h.

## 9. Otwarte pytania
1. Czy funkcja ma być dostępna w publicznym buildzie od początku, czy najpierw tylko prywatnie?
2. Czy podsumowanie bitwy, sesji i tygodnia (E7) — wszystkie trzy, czy start od sesji?
3. Język podsumowania przy oknie EN: ten sam model (Bielik jest polski) czy inny model dla EN?
