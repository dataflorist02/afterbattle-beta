# Marka Afterbattle

Zatwierdzone przez autora 2026-11-15 (spec 29 §3–§5). Projekt na płótnie Claude Design „Afterbattle 2.0 — redesign”, rząd „Logo”.

## Znak

Litera A jako kontur z poprzeczką w kształcie znaku wzrostu. Kontur ze stali, rozgrzana tylko poprzeczka: analiza „po bitwie” i trend w górę. Znak jest oryginalny: bez czołgów, luf, celowników i innych motywów z gry.

| Plik | Do czego |
|---|---|
| `afterbattle-icon.svg` / `afterbattle-icon-1024.png` | ikona aplikacji (kontur na granatowej płytce, gruba linia); źródło `pnpm tauri icon` |
| `afterbattle-mark.svg` | znak na ciemnym tle, od ok. 48 px |
| `afterbattle-mark-small.svg` | poniżej 48 px, druk jednobarwny |
| `afterbattle-mark-light.svg` | jasne tło |
| `afterbattle-mark-dark-192.png`, `afterbattle-mark-light-192.png` | README |
| `afterbattle-loading-reference.svg` | wzorzec ekranu ładowania (zamrożony na 62%) |

Geometria (viewBox 64): kontur `32,9 53,55 43.5,55 32,29 20.5,55 11,55`, poprzeczka `24,47 32,40.5 40,47`.

## Kolory

Trzy rodziny: granat (tła), stal (tekst i linie), żar (jedyny akcent). Tokeny w `src/styles/tokens.css`; tabela nowych tokenów w spec 29 §3.

- Granat: `#070b14`, `#0a101c`, `#0d1422`, `#121927`, `#16223a`, `#1e3156`
- Stal: `#eef1f6`, `#dce2ea`, `#c9d1dc`, `#9aa6b8`, `#8693a6`, `#8b97ab`
- Żar: `#f08a3c` (akcent), `#f7b27a`, `#f3a36b` (link), `#e0661f`, `#9a3a1a`, `#ffebd0`
- Znaczenia: wzrost `#f7b27a`, spadek `#6e8fc6`

Zasady: jeden akcent na ekranie; tekst w stali; gradient żaru tylko w znaku, ekranie ładowania i postępie; porażka bez czerwieni.

## Krój

Big Shoulders (liczby, nagłówki, nazwa w logo: 300, rozstrzelona) i Inter (tekst). Oba na licencji OFL, pliki w `src/assets/fonts`.
