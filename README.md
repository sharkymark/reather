# Reather - Rust CLI Weather, Airport, Real Estate, and Earthquake App

Reather is a Rust-based command-line application that provides:
- Robust airport search (with wildcard support)
- Weather and forecast for addresses and airports
- Real estate and external links for locations
- USGS Earthquake data with filtering by magnitude and time
- User-friendly address management (auto-geocoding, normalization)

## Features

### Main Menu
- Enter a new street address (auto-geocoded and normalized)
- Choose from stored addresses
- Airport search (wildcard, US/passenger filters)
- Earthquakes (USGS, filter by magnitude and time)
- Exit

### Airport Search
- Search by code, state, municipality, or name
- Wildcard support: `Rome*`, `*Rome`, `*Rome*`, `Rome`
- Filter for US states and/or passenger airports
- Displays airport, weather, forecast, and external links

### Address Management
- `addresses.txt` stores addresses with lat/lon
- Addresses without lat/lon are geocoded and normalized at startup
- Normalization: stored as uppercase, matched address if lat/lon match
- `addresses.txt` is ignored by git (user data is safe)

### Earthquakes (USGS)
- Menu for minimum magnitude: All, 5.0+, 6.0+, 7.0+
- Menu for time period: 24 hours, 48 hours, 7 days
- Results filtered by magnitude and time
- Each earthquake shows:
  - Magnitude, location, time
  - Coordinates, depth
  - Google Maps link
  - USGS event link

## Usage

1. Build and run:
   ```sh
   cargo build
   cargo run
   ```
2. Follow the interactive menu prompts.

## Data Files
- `data/addresses.txt`: User addresses (auto-managed, not tracked by git)
- `data/airports.csv`: Airport database (auto-managed)

## Dependencies
- Rust (2021 edition)
- [reqwest](https://crates.io/crates/reqwest)
- [serde](https://crates.io/crates/serde)
- [tokio](https://crates.io/crates/tokio)
- [chrono](https://crates.io/crates/chrono)

## Notes
- User data in `addresses.txt` is never overwritten by git operations.
- Earthquake data is fetched from [USGS GeoJSON feeds](https://earthquake.usgs.gov/earthquakes/feed/v1.0/geojson.php).
- For best results, ensure you have an internet connection.

---

MIT License
