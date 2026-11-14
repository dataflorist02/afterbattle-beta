# Informacja o prywatności

Tłumaczenie [`PRIVACY.md`](../PRIVACY.md). W razie rozbieżności obowiązuje wersja angielska.

Afterbattle nie ma serwera prowadzonego przez autora, telemetrii, reklam, analityki, rejestracji konta, chmurowej AI ani automatycznego wysyłania raportów o awariach. Podstawowe śledzenie działa bez internetu.

## Odczytywane pliki lokalne

Po wyraźnym zatwierdzeniu instalacji World of Tanks backend w Ruście może odczytywać:

- ustabilizowane pliki `.wotreplay` w kanonicznym folderze `replays`, czytając wyłącznie zadeklarowane bloki JSON metadanych i wyników, z limitem 64 MiB na plik;
- `res/packages/scripts.pkg` zatwierdzonej instalacji, czytając wyłącznie listy pojazdów (`scripts/item_defs/vehicles/<nacja>/list.xml`), aby ustalić klasę pojazdu i klucze tłumaczeń nazw pojazdów.
- `res/text/lc_messages/<nacja>_vehicles.mo` zatwierdzonej instalacji (własne pliki tłumaczeń gry z nazwami pojazdów), na żądanie, lokalnie i bez sieci, z limitami rozmiaru i liczby wpisów sprawdzanymi przed odczytem. Nazwy są trzymane tylko w pamięci i pokazywane w oknie; nigdy nie trafiają do bazy, na dysk ani do eksportu.
- `res/packages/gui*.pkg` zatwierdzonej instalacji, czytając na żądanie wyłącznie ikony pojazdów (`gui/maps/icons/vehicle/`), grafiki map (`gui/maps/icons/map/stats/` i `gui/maps/icons/map/`) oraz ekrany ładowania map (`gui/maps/icons/map/screen/`), lokalnie i bez sieci. Ekrany ładowania są dekodowane i zmniejszane do pokazywanego rozmiaru w pamięci. Grafiki są trzymane tylko w pamięci (najwyżej 64 MB, czyszczone po aktualizacji gry) i nigdy nie są zapisywane na dysk, eksportowane ani dołączane do aplikacji.

Frontend nigdy nie dostaje surowej zawartości plików. Afterbattle nie czyta `python.log`, danych garażu, kredytów, złota, obligacji, czatu, zrzutów ekranu ani niepowiązanych plików. Natywny obserwator plików działa razem z okresowym skanem awaryjnym. Nieobsługiwane lub niejednoznaczne dane są oznaczane jako częściowe lub N/D i nie są zgadywane. Do statystyk wliczają się wyłącznie bitwy losowe.

## Przechowywanie lokalne

SQLite przechowuje znormalizowane metadane bitew, opcjonalne statystyki bojowe, sesje, dane do wniosków, język, limit bezczynności i niewrażliwy stan przetwarzania powtórek. Stan przetwarzania zawiera jednokierunkowy klucz ścieżki źródła, rozmiar, czas modyfikacji, liczbę bloków i odcisk treści. **Nie** przechowuje surowych bajtów powtórek, surowych wpisów z logów, surowych odpowiedzi API, pełnej ścieżki do WoT, nazw plików ani identyfikatorów kont.

Dla każdej bitwy losowej gracza SQLite przechowuje też lokalny, anonimowy skład bitwy: pojazd i statystyki bitwy każdego uczestnika, bez nicków, ID kont i klanów; skład jest wyłączony z eksportu.

Przy aktualizacji bazy ze starszej wersji jest ona najpierw kopiowana bez zmian do `afterbattle.v{n}.bak` obok; nowa baza jest potem odtwarzana z powtórek. Uszkodzony plik bazy jest przenoszony do `afterbattle.corrupt-<czas>.sqlite3`, a nowa baza jest odtwarzana z powtórek. Jeśli bazy nie da się otworzyć z innego powodu (na przykład zapisała ją nowsza wersja albo zbyt wiele jej bitew nie ma już pliku powtórki), Afterbattle startuje w trybie awaryjnym z pustą bazą awaryjną pod jedną stałą ścieżką obok właściwej bazy (nigdy w systemowym folderze tymczasowym). W trybie awaryjnym bitwy nie są importowane, a „Usuń wszystkie dane lokalne” jest odrzucane, więc prawdziwa historia nigdy nie jest zgłaszana jako usunięta, gdy nadal istnieje. Baza awaryjna jest usuwana przy następnym udanym starcie.

Menedżer poświadczeń Windows przechowuje tylko zatwierdzoną ścieżkę do WoT. Wcześniejsze wersje mogły też przechowywać profil i klucz Tomato.gg oraz samodzielny `application_id` Wargaming; te integracje zostały usunięte, a każdy start kasuje ich pozostałe wpisy. Wartości poświadczeń nigdy nie trafiają do frontendu; dostaje on tylko flagi stanu i nazwę wyświetlaną folderu gry.

## Opcjonalne połączenia sieciowe

Jedyna funkcja sieciowa jest domyślnie wyłączona:

- XVM: po osobnym włączeniu statyczny plik wartości oczekiwanych jest pobierany wyłącznie z `https://static.modxvm.com/wn8-data-exp/json/wg/wn8exp.json`, najwyżej raz dziennie. Gdy XVM jest wyłączony, żadne żądanie nie jest wysyłane. Nie jest wysyłany żaden identyfikator gracza ani bitwy.

Przekierowania są wyłączone, rozmiary odpowiedzi i czasy oczekiwania są ograniczone, a sam WebView nie ma uprawnień do internetu.

## Eksport

Eksport sesji pomija nick, ID konta, poświadczenia, pełne ścieżki, lokalne odciski i ID aren, UUID sesji, dokładne godziny, ID i nazwy pojazdów oraz nazwy map. Zawiera czas trwania, statystyki, etykiety jakości danych i wnioski. Przejrzyj go przed publikacją, bo rzadki ciąg statystyk może być rozpoznawalny dla kogoś, kto już zna tę sesję.

## Usuwanie

**Usuń wszystkie dane lokalne** najpierw zatrzymuje dostęp do skonfigurowanego źródła, potem usuwa ścieżkę do WoT oraz pozostałe wpisy Tomato.gg lub Wargaming z Menedżera poświadczeń. W jednej transakcji usuwa stan przetwarzania powtórek, bitwy (razem z anonimowymi składami), sesje oraz ustawienia profilu i historii, potem skraca plik WAL i kompaktuje bazę oraz usuwa kopie `afterbattle.v{n}.bak` i pliki `afterbattle.corrupt-*`. Preferencje języka i limitu bezczynności zostają. Pliki należące do World of Tanks nigdy nie są zmieniane ani usuwane. W trybie awaryjnym usuwanie jest odrzucane (patrz Przechowywanie lokalne).

Złośliwe oprogramowanie działające na tym samym koncie lub w pełni przejęte konto Windows nadal może odczytać pamięć procesu, pliki lokalne lub poświadczenia systemowe; to wykracza poza ochronę, jaką może dać aplikacja desktopowa działająca na tym samym koncie.
