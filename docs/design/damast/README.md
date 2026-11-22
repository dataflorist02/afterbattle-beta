# Makiety: Damast (spec 34)

- historia.png: Historia → Sesje jako kolekcja płytek.
- karta.png: karta 1200×630 do eksportu PNG.
- kucie-klatki.png: cztery klatki wykuwania (p = 0,2 / 0,45 / 0,7 / 1).
- wzor-referencyjny.js: referencyjna implementacja wzoru (CPU, per piksel). Shader i Web Worker mają dawać ten sam obraz; parametry i mapowanie danych są stąd.
- sesje-syntetyczne.json: trzy syntetyczne sesje (cieplej, jak zwykle, chłodniej) do testów i obrazów wzorcowych.

Interaktywna wersja wykuwania jest na płótnie (Damast-Kuznia). W makiecie klatki animacji są w niskiej rozdzielczości; w aplikacji pełna rozdzielczość przez WebGL2 (spec 34 §4).

## HTML
- `html/kuznia.html`: samodzielna, działająca makieta wykuwania (przełączanie sesji, „Wykuj ponownie”, ograniczony ruch). Otwierasz w przeglądarce, bez serwera.
- `html/historia.html`, `html/karta.html`: źródła makiet z obrazami płytek obok (`damast-*.jpg`).
