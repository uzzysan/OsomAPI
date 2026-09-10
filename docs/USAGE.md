# Instrukcja użycia OsomAPI

## Wymagania wstępne

1. **Rust** – zainstaluj przez [rustup.rs](https://rustup.rs/)
2. Dla lokalnych modeli: **Ollama** – zainstaluj z [ollama.com](https://ollama.com/) i pobierz wybrany model:
   ```bash
   ollama pull llama3
   ```
3. Dla modeli chmurowych: **klucz API** od wybranego dostawcy

## Budowanie aplikacji

```bash
cd osom-api
cargo build --release
```

Plik wykonywalny znajdzie się w `./target/release/osom-api`.

## Tryby działania

Aplikacja OsomAPI może działać w dwóch trybach:
1. **Tryb serwera HTTP z panelem Web UI** (`--serve` lub brak parametru `--input`)
2. **Tryb CLI** (przetwarzanie pojedynczego pliku lub wsadowego katalogu)

## Tryb serwera HTTP i panel Web UI

Uruchomienie serwera:
```bash
osom-api --serve
```
Opcjonalnie ze wskazaniem portu i adresu:
```bash
osom-api --serve --host 127.0.0.1 --port 8080 --config config.toml
```

Po uruchomieniu przejdź w przeglądarce pod adres `http://localhost:8080`, aby:
- Przeglądać, dodawać i konfigurować endpointy procesowania danych (np. `/api/v1/process/invoices`).
- Wczytać przykładowe pliki (CSV, JSON, XML) i automatycznie wyciągnąć listę kolumn/pól.
- Wizualnie przypisać pola wejściowe do pól schematu wyjściowego lub oznaczyć pola jako ignorowane.
- Konfigurować schematy wyjściowe (typy, zaokrąglenia decimal, formaty dat).
- Konfigurować miejsce zapisu (JSON, XML, SQLite, PostgreSQL, MySQL).
- Testować zapytania w czasie rzeczywistym (dry-run oraz live process).

### Endpointy REST API serwera

| Metoda | Ścieżka | Opis |
|---|---|---|
| `GET` | `/` | Wbudowany panel Web UI |
| `GET` | `/api/status` | Status serwera, wersja i aktywny model LLM |
| `GET` | `/api/config` | Pobranie bieżącej konfiguracji |
| `POST` | `/api/config` | Zapisanie zaktualizowanej konfiguracji |
| `GET` | `/api/endpoints` | Lista skonfigurowanych endpointów |
| `POST` | `/api/endpoints` | Dodanie lub edycja endpointu |
| `DELETE` | `/api/endpoints/:id` | Usunięcie endpointu |
| `POST` | `/api/preview` | Wczytanie pliku/tekstu i detekcja pól wejściowych |
| `POST` | `/api/process/:endpoint_id` | Przetworzenie danych przez dany endpoint i zapis |
| `POST` | `/api/dry-run/:endpoint_id` | Wygenerowanie promptu dla danego endpointu |

## Tryb CLI

```bash
osom-api --input <plik_lub_katalog> --config <plik_konfiguracyjny>
```

### Parametry CLI

| Flaga | Skrót | Wymagana | Opis | Domyślnie |
|-------|-------|----------|------|-----------|
| `--input` | `-i` | nie | Ścieżka do pliku lub katalogu (uruchamia serwer jeśli pominięto) | – |
| `--config` | `-c` | nie | Ścieżka do pliku konfiguracyjnego | `config.toml` |
| `--serve` | – | nie | Wymuszenie uruchomienia serwera HTTP i Web UI | `false` |
| `--host` | – | nie | Adres nasłuchiwania serwera | z configu (`0.0.0.0`) |
| `--port` | – | nie | Port nasłuchiwania serwera | z configu (`8080`) |
| `--dry-run` | – | nie | Wyświetlenie promptu bez wysyłania zapytania do LLM | `false` |
| `--output` | `-o` | nie | Nadpisanie ścieżki pliku wynikowego | z pliku config |
| `--pattern` | `-p` | nie | Filtr plików przy skanowaniu katalogu (np. `*.csv`) | – |

### Przykłady uruchomienia

**Przetwarzanie pliku CSV z konfiguracją domyślną:**
```bash
osom-api --input faktury.csv
```

**Podgląd wygenerowanego promptu (tryb dry-run):**
```bash
osom-api --input faktury.csv --config config.toml --dry-run
```

**Nadpisanie docelowego pliku wyjściowego:**
```bash
osom-api --input faktury.csv --config config.toml -o /tmp/wyniki_test.json
```

**Przetwarzanie pliku PDF z własną konfiguracją:**
```bash
osom-api --input dokument.pdf --config ./configs/ollama.toml
```

**Przetwarzanie pliku JSON z modelem GPT:**
```bash
osom-api --input dane.json --config ./configs/openai.toml
```

## Wspierane formaty wejściowe

| Rozszerzenie | Format | Opis |
|-------------|--------|------|
| `.json` | JSON | Bezpośrednia deserializacja, pretty-print do LLM |
| `.xml` | XML | Konwersja do JSON z zachowaniem atrybutów i hierarchii |
| `.csv` | CSV | Konwersja wierszy na tablicę obiektów JSON |
| `.pdf` | PDF | Ekstrakcja tekstu z dokumentu |

**Uwaga:** Aplikacja rozpoznaje typ na podstawie rozszerzenia pliku. Upewnij się, że plik ma poprawne rozszerzenie.

## Przykładowy scenariusz: ekstrakcja faktur z PDF

### 1. Przygotuj plik konfiguracyjny `faktury.toml`

```toml
[llm]
provider = "ollama"
model = "llama3"

[output.schema]
name = "faktury"

[[output.schema.fields]]
name = "numer_faktury"
type = "string"
required = true
description = "Numer faktury VAT"

[[output.schema.fields]]
name = "data_wystawienia"
type = { type = "date", format = "%Y-%m-%d" }
required = true
description = "Data wystawienia faktury"

[[output.schema.fields]]
name = "kwota_netto"
type = { type = "decimal", precision = 15, scale = 2 }
required = true
description = "Kwota netto faktury"

[[output.schema.fields]]
name = "kwota_vat"
type = { type = "decimal", precision = 15, scale = 2 }
required = true
description = "Kwota VAT"

[[output.schema.fields]]
name = "kwota_brutto"
type = { type = "decimal", precision = 15, scale = 2 }
required = true
description = "Kwota brutto faktury"

[[output.schema.fields]]
name = "nip_sprzedawcy"
type = "string"
required = true
description = "NIP sprzedawcy"

[[output.schema.fields]]
name = "nip_kupujacego"
type = "string"
required = false
description = "NIP kupującego"

[output.destination]
type = "json"
path = "faktury.json"
pretty = true
```

### 2. Uruchom aplikację

```bash
osom-api --input faktura_vat_123.pdf --config faktury.toml
```

### 3. Sprawdź wynik

```bash
cat faktury.json
```

Przykładowy wynik:
```json
[
  {
    "numer_faktury": "FV/2024/123",
    "data_wystawienia": "2024-03-15",
    "kwota_netto": 1000.00,
    "kwota_vat": 230.00,
    "kwota_brutto": 1230.00,
    "nip_sprzedawcy": "123-456-78-90",
    "nip_kupujacego": "987-654-32-10"
  }
]
```

## Przykładowy scenariusz: konwersja CSV do SQLite

### 1. Przygotuj plik konfiguracyjny `csv-to-sqlite.toml`

```toml
[llm]
provider = "gemini"
api_key = "your-gemini-api-key"
model = "gemini-1.5-flash"

[output.schema]
name = "klienci"

[[output.schema.fields]]
name = "imie"
type = "string"
required = true

[[output.schema.fields]]
name = "nazwisko"
type = "string"
required = true

[[output.schema.fields]]
name = "email"
type = "string"
required = true

[[output.schema.fields]]
name = "telefon"
type = "string"
required = false

[output.destination]
type = "database"
driver = "sqlite"
path = "klienci.db"
table_name = "klienci"
```

### 2. Uruchom aplikację

```bash
osom-api --input klienci.csv --config csv-to-sqlite.toml
```

### 3. Zweryfikuj bazę danych

```bash
sqlite3 klienci.db "SELECT * FROM klienci;"
```

## Rozwiązywanie problemów

### "Nieobsługiwane rozszerzenie pliku"

Upewnij się, że plik wejściowy ma jedno z rozszerzeń: `.json`, `.xml`, `.csv`, `.pdf`. Jeśli plik nie ma rozszerzenia, zmień jego nazwę lub przekonwertuj do obsługiwanego formatu.

### "Błąd ładowania konfiguracji"

Sprawdź:
- Czy plik konfiguracyjny istnieje w podanej ścieżce
- Czy składnia TOML jest poprawna (użyj walidatora online jeśli potrzebujesz)
- Czy schemat zawiera co najmniej jedno pole
- Czy dla dostawców chmurowych podano niepusty `api_key`

### "Błąd LLM: Empty response"

Możliwe przyczyny:
- Nieprawidłowy klucz API (sprawdź logi na poziomie `debug`)
- Model nie obsługuje wybranego endpointu
- Timeout – zwiększ `request_timeout_secs` w konfiguracji

### "Błąd parsowania PDF"

Plik PDF może być skanem (obrazek zamiast tekstu) lub mieć uszkodzoną strukturę. W takim przypadku użyj OCR przed przekazaniem do OsomAPI.

### Logowanie debug

Aby zobaczyć szczegółowe logi, ustaw poziom logowania w konfiguracji:

```toml
[settings]
log_level = "debug"
```

Lub ustaw zmienną środowiskową przed uruchomieniem:
```bash
set RUST_LOG=debug
osom-api --input dane.csv --config config.toml
```

## Wskazówki dla lepszych wyników

1. **Dokładne opisy pól** – im bardziej szczegółowy `description`, tym lepiej model zrozumie co wyciągnąć
2. **Wartości domyślne** – ustawiaj `default` dla opcjonalnych pól, aby uniknąć nulli w wynikach
3. **Normalizacja wielkości liter** – użyj `case_normalization` gdy chcesz ujednolicić format tekstu
4. **Formaty dat** – jawnie podawaj `format` daty, aby model wiedział w jakiej postaci ma zwracać daty
5. **Wybór modelu** – dla złożonych dokumentów PDF zalecane są mocniejsze modele (GPT-4o, Claude Sonnet). Proste ekstrakcje z CSV/JSON mogą działać dobrze na lżejszych modelach (llama3, gemini-flash)
