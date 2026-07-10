# Dokumentacja konfiguracji OsomAPI

Plik konfiguracyjny definiuje źródło modelu LLM, strukturę danych wyjściowych oraz miejsce zapisu wyników. OsomAPI obsługuje formaty **TOML** oraz **JSON**.

## Lokalizacja pliku

Domyślnie aplikacja szuka pliku `config.toml` w katalogu roboczym. Można wskazać inną ścieżkę przez flagę `--config`:

```bash
osom-api --input dane.csv --config ./sciezka/do/mojego-config.toml
```

## Sekcja `[llm]` – konfiguracja modelu

Wybierz jednego dostawcę poprzez pole `provider`.

### Ollama (lokalny)

```toml
[llm]
provider = "ollama"
model = "llama3"
url = "http://localhost:11434"
```

- `model` – nazwa modelu zainstalowanego w Ollama
- `url` – adres lokalnej instancji Ollama (opcjonalne, domyślnie `http://localhost:11434`)

### OpenAI (GPT)

```toml
[llm]
provider = "openai"
api_key = "sk-..."
model = "gpt-4o-mini"
```

- `api_key` – klucz API ze strony OpenAI
- `model` – domyślnie `gpt-4o-mini`

### Google Gemini

```toml
[llm]
provider = "gemini"
api_key = "..."
model = "gemini-1.5-flash"
```

- `api_key` – klucz API ze Google AI Studio
- `model` – domyślnie `gemini-1.5-flash`

### Anthropic (Claude)

```toml
[llm]
provider = "anthropic"
api_key = "sk-ant-..."
model = "claude-3-5-sonnet-20241022"
```

- `api_key` – klucz API z Anthropic Console
- `model` – domyślnie `claude-3-5-sonnet-20241022`

### GitHub Copilot

```toml
[llm]
provider = "copilot"
api_key = "ghu_..."
model = "gpt-4o"
```

- `api_key` – token GitHub Copilot
- `model` – domyślnie `gpt-4o`

## Sekcja `[output.schema]` – schemat danych wyjściowych

Definiuje strukturę, której model LLM ma się trzymać przy ekstrakcji danych.

```toml
[output.schema]
name = "faktury"
case_normalization = "lower"
```

- `name` – nazwa logiczna schematu (używana w promptach LLM)
- `case_normalization` – opcjonalna normalizacja wielkości liter tekstowych wartości. Dostępne opcje:
  - `lower` – małe litery (`hello world`)
  - `upper` – wielkie litery (`HELLO WORLD`)
  - `title` – format tytułu (`Hello World`)
  - `snake` – snake_case (`hello_world`)
  - `camel` – camelCase (`helloWorld`)
  - `pascal` – PascalCase (`HelloWorld`)

### Definiowanie pól (`[[output.schema.fields]]`)

Każde pole wymaga co najmniej `name` i `type`.

#### Typy prostych

**String:**
```toml
[[output.schema.fields]]
name = "nazwa_produktu"
type = "string"
required = true
description = "Pełna nazwa produktu"
```

**Integer:**
```toml
[[output.schema.fields]]
name = "ilosc"
type = "integer"
required = true
description = "Liczba sztuk"
```

**Decimal (liczba zmiennoprzecinkowa):**
```toml
[[output.schema.fields]]
name = "cena_brutto"
type = { type = "decimal", precision = 19, scale = 2 }
required = true
description = "Cena brutto w PLN"
```

- `precision` – maksymalna liczba cyfr (domyślnie 19)
- `scale` – liczba miejsc po przecinku (domyślnie 4)

**Boolean:**
```toml
[[output.schema.fields]]
name = "czy_aktywny"
type = "boolean"
required = true
description = "Czy produkt jest aktywny"
```

**Date:**
```toml
[[output.schema.fields]]
name = "data_utworzenia"
type = { type = "date", format = "%Y-%m-%d" }
required = true
description = "Data utworzenia rekordu"
```

- `format` – format daty w notacji strftime (domyślnie `%Y-%m-%d`)

**DateTime:**
```toml
[[output.schema.fields]]
name = "czas_modyfikacji"
type = { type = "datetime", format = "%Y-%m-%dT%H:%M:%S" }
required = false
description = "Czas ostatniej modyfikacji"
```

#### Typy złożone

**Object (obiekt zagnieżdżony):**
```toml
[[output.schema.fields]]
name = "adres"
type = { type = "object", fields = [
    { name = "ulica", type = "string", required = true },
    { name = "miasto", type = "string", required = true },
    { name = "kod_pocztowy", type = "string", required = true }
]}
required = true
description = "Adres dostawy"
```

**Array (tablica):**
```toml
[[output.schema.fields]]
name = "tagi"
type = { type = "array", item_type = "string" }
required = false
description = "Etykiety przypisane do produktu"
```

Tablice mogą zawierać również złożone typy:
```toml
[[output.schema.fields]]
name = "pozycje"
type = { type = "array", item_type = { type = "object", fields = [
    { name = "nazwa", type = "string", required = true },
    { name = "cena", type = { type = "decimal", precision = 10, scale = 2 }, required = true }
]}}
required = true
```

### Wspólne atrybuty pól

| Atrybut | Wymagany | Opis |
|---------|----------|------|
| `name` | tak | Nazwa pola w wynikowym JSON/XML |
| `type` | tak | Typ danych (patrz powyżej) |
| `required` | nie | Czy pole jest wymagane (domyślnie `true`) |
| `description` | nie | Opis dla modelu LLM jak interpretować to pole |
| `default` | nie | Wartość domyślna jeśli model nie znajdzie danych |

## Sekcja `[output.destination]` – miejsce zapisu

### JSON

```toml
[output.destination]
type = "json"
path = "wyniki.json"
pretty = true
```

- `path` – ścieżka do pliku wyjściowego
- `pretty` – czy formatować z wcięciami (domyślnie `true`)

### XML

```toml
[output.destination]
type = "xml"
path = "wyniki.xml"
pretty = true
```

- `path` – ścieżka do pliku wyjściowego
- `pretty` – czy formatować z wcięciami (domyślnie `true`)

### Baza danych (SQLite)

```toml
[output.destination]
type = "database"
driver = "sqlite"
path = "wyniki.db"
table_name = "faktury"
```

- `driver` – obecnie obsługiwany tylko `sqlite`
- `path` – ścieżka do pliku bazy SQLite
- `table_name` – nazwa tabeli (opcjonalna, domyślnie `osom_records`)

**Uwaga:** PostgreSQL i MySQL są zdefiniowane w typach ale nie w pełni zaimplementowane w tej wersji.

## Sekcja `[settings]` – opcjonalne ustawienia aplikacji

```toml
[settings]
request_timeout_secs = 120
max_retries = 3
log_level = "info"
```

- `request_timeout_secs` – timeout dla żądań HTTP do LLM (domyślnie 120s)
- `max_retries` – maksymalna liczba ponownych prób (domyślnie 3)
- `log_level` – poziom logowania: `trace`, `debug`, `info`, `warn`, `error` (domyślnie `info`)

## Przykładowy kompletny plik konfiguracyjny

```toml
[llm]
provider = "openai"
api_key = "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
model = "gpt-4o-mini"

[output]
[output.schema]
name = "produkty"
case_normalization = "snake"

[[output.schema.fields]]
name = "id"
type = "integer"
required = true
description = "Unikalny identyfikator produktu"

[[output.schema.fields]]
name = "nazwa"
type = "string"
required = true
description = "Nazwa produktu"

[[output.schema.fields]]
name = "cena"
type = { type = "decimal", precision = 10, scale = 2 }
required = true
description = "Cena detaliczna w PLN"

[[output.schema.fields]]
name = "kategoria"
type = "string"
required = false
description = "Kategoria produktu"
default = "inne"

[[output.schema.fields]]
name = "data_dodania"
type = { type = "date", format = "%Y-%m-%d" }
required = true
description = "Data dodania produktu do oferty"

[output.destination]
type = "json"
path = "produkty.json"
pretty = true

[settings]
request_timeout_secs = 180
log_level = "debug"
```
