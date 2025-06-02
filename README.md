# Reather - a Rust-based Weather App

Reather is a modern, portable CLI app for weather, airport, and real estate info in the USA. It features robust airport detection using the full OurAirports database, and provides rich external links for any address or weather station.

## Features

- **Fast, portable Rust CLI**
- **Automatic airport detection**: Uses the full OurAirports CSV (downloaded at startup) to detect US airports by IATA or ICAO code, with no hardcoded lists
- **Flightradar24, official airport, and Wikipedia links**: For any weather station at a US airport, shows Flightradar24, official airport website, and Wikipedia links in the external links submenu
- **Google Maps and Zillow links**: For any address or station
- **Zillow links for all US airports**: Always shows a Zillow real estate link for any airport in a US state (including military/heliport/remote fields), using robust geocoding and state abbreviation logic
- **No warnings, robust and idiomatic Rust code**
- **Portable data storage**: Uses `data/addresses.txt` if present, or creates `addresses.txt` in the executable directory
- **Seed addresses**: Built-in for easy first use

## Usage

```
cargo run
```

On startup, you'll see:

```
Reather - a Rust-based Weather App
USA airport database loaded: <count> airports

Main Menu:
1. Enter a new street address
2. Choose from stored addresses
3. Airport Search
4. Exit
```

When you select an address or airport, you'll get a submenu with options for current conditions, local forecast, and external links. If the nearest weather station is at a US airport, you'll see:

- Flightradar24 link
- Official airport website (if available)
- Wikipedia link (if available)
- Zillow real estate link (for all US airports, including military/remote)

## Data Sources
- [OurAirports CSV](https://ourairports.com/data/)
- [NOAA/NWS API](https://www.weather.gov/documentation/services-web-api)
- [Google Maps](https://maps.google.com)
- [Zillow](https://www.zillow.com)
- [Flightradar24](https://www.flightradar24.com)

## Portability

- No data directory required: runs from any location
- Adaptive storage: uses `data/addresses.txt` if present, otherwise creates `addresses.txt` in the executable directory
- Built-in seed addresses for easy setup

## Requirements
- Rust (latest stable)
- Internet connection (for weather, airport, and geocoding data)

## License
MIT
