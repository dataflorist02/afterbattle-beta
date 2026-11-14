import type { Language } from "../types";

/**
 * Error dictionary (spec 20 D10, spec 25 §7): every `code` the backend sends has a Polish and an
 * English text that says what happened and what to do next, without codes or technical words.
 * The completeness test compares these keys with the codes in `src-tauri/src/error.rs`.
 */
export const errorMessages = {
  not_configured: {
    pl: "Nie wybrano folderu gry. Wskaż go w Ustawieniach.",
    en: "No game folder is selected. Choose it in Settings.",
  },
  invalid_api_response: {
    pl: "Nie udało się odczytać tych danych. Spróbuj ponownie za chwilę.",
    en: "This data could not be read. Try again in a moment.",
  },
  no_active_session: {
    pl: "Nie ma trwającej sesji. Zacznij nową albo zagraj bitwę.",
    en: "There is no session in progress. Start one or play a battle.",
  },
  session_not_found: {
    pl: "Tej sesji już nie ma. Odśwież listę sesji.",
    en: "This session no longer exists. Refresh the session list.",
  },
  battle_not_found: {
    pl: "Tej bitwy już nie ma w historii. Wróć do listy bitew.",
    en: "This battle is no longer in your history. Go back to the battle list.",
  },
  database_error: {
    pl: "Zapis lub odczyt danych się nie powiódł. Zamknij aplikację i uruchom ją ponownie.",
    en: "Saving or reading data failed. Close the app and start it again.",
  },
  credential_error: {
    pl: "System Windows nie pozwolił na zapis ustawień. Uruchom aplikację ponownie.",
    en: "Windows did not allow the settings to be saved. Start the app again.",
  },
  filesystem_error: {
    pl: "Nie udało się odczytać lub zapisać pliku. Sprawdź, czy dysk jest dostępny.",
    en: "A file could not be read or written. Check that the drive is available.",
  },
  invalid_game_directory: {
    pl: "W tym folderze nie ma gry World of Tanks. Wybierz folder, w którym jest gra.",
    en: "This folder does not contain World of Tanks. Choose the folder with the game.",
  },
  replay_incomplete: {
    pl: "Gra wciąż zapisuje ostatnią bitwę. Wyniki pojawią się za chwilę same.",
    en: "The game is still saving the last battle. The results will appear on their own shortly.",
  },
  invalid_replay: {
    pl: "Plik ostatniej bitwy ma nieznany format i został pominięty.",
    en: "The last battle file has an unknown format and was skipped.",
  },
  replay_too_large: {
    pl: "Plik bitwy jest zbyt duży, żeby go bezpiecznie odczytać, i został pominięty.",
    en: "The battle file is too large to read safely and was skipped.",
  },
  tracker_error: {
    pl: "Odczyt bitew chwilowo nie działa. Uruchom aplikację ponownie.",
    en: "Reading battles is not working at the moment. Start the app again.",
  },
  unsupported_database_schema: {
    pl: "Plik z danymi jest niekompletny. Aplikacja odtworzy historię z plików bitew przy następnym uruchomieniu.",
    en: "The data file is incomplete. The app will rebuild your history from the battle files on the next start.",
  },
  database_integrity_error: {
    pl: "Plik z danymi jest uszkodzony. Aplikacja odtworzy historię z plików bitew przy następnym uruchomieniu.",
    en: "The data file is damaged. The app will rebuild your history from the battle files on the next start.",
  },
  challenge_already_active: {
    pl: "Masz już aktywny cel. Dokończ go albo porzuć, zanim zaczniesz nowy.",
    en: "You already have an active goal. Finish or abandon it before starting a new one.",
  },
  no_active_challenge: {
    pl: "Nie masz aktywnego celu.",
    en: "You have no active goal.",
  },
  stale_suggestion: {
    pl: "Ta propozycja celu jest już nieaktualna. Odśwież propozycje.",
    en: "This goal suggestion is out of date. Refresh the suggestions.",
  },
  invalid_challenge: {
    pl: "Dane celu są niespójne i cel nie może być użyty. Wybierz inny cel.",
    en: "The goal data is inconsistent and the goal cannot be used. Choose another goal.",
  },
  invalid_window: {
    pl: "Wybierz 10, 30 albo 100 ostatnich bitew.",
    en: "Choose the last 10, 30 or 100 battles.",
  },
  report_invariant_violation: {
    pl: "Wyniki celności są niespójne i nie zostaną pokazane. Spróbuj ponownie po kolejnej bitwie.",
    en: "The accuracy results are inconsistent and will not be shown. Try again after your next battle.",
  },
  network_error: {
    pl: "Brak połączenia z serwerem danych WN8. Aplikacja używa ostatnich pobranych danych.",
    en: "The WN8 data server could not be reached. The app uses the last downloaded data.",
  },
  recovery_mode_delete_refused: {
    pl: "Dane są w trybie awaryjnym, więc usuwanie jest zablokowane. Uruchom aplikację ponownie.",
    en: "Your data is in safe mode, so deleting is blocked. Start the app again.",
  },
  recovery_mode_tracker_disabled: {
    pl: "Dane są w trybie awaryjnym i nowe bitwy nie są zapisywane. Uruchom aplikację ponownie.",
    en: "Your data is in safe mode and new battles are not saved. Start the app again.",
  },
  xvm_disabled: {
    pl: "Pobieranie danych WN8 jest wyłączone. Włącz je w Ustawieniach.",
    en: "Downloading WN8 data is turned off. Turn it on in Settings.",
  },
  database_too_new: {
    pl: "Dane pochodzą z nowszej wersji aplikacji. Zainstaluj najnowszą wersję.",
    en: "Your data comes from a newer version of the app. Install the latest version.",
  },
  legacy_rebuild_blocked: {
    pl: "Wielu dawnych bitew brakuje w plikach gry, więc historia nie została przeliczona. Twoje dane są nietknięte.",
    en: "Many past battles are missing from the game files, so the history was not recalculated. Your data is untouched.",
  },
  unknown_error: {
    pl: "Coś poszło nie tak. Spróbuj ponownie.",
    en: "Something went wrong. Try again.",
  },
} as const satisfies Record<string, Record<Language, string>>;

export type ErrorCode = keyof typeof errorMessages;

export function errorMessage(code: string | undefined, language: Language): string {
  const entry = code && code in errorMessages ? errorMessages[code as ErrorCode] : undefined;
  return (entry ?? errorMessages.unknown_error)[language];
}
