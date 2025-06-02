// This file contains utility functions related to airports and other external links
// It's designed to be imported into the main application

use std::collections::HashMap;
use std::sync::OnceLock;
use serde::Deserialize;

const AIRPORTS_CSV_URL: &str = "https://raw.githubusercontent.com/davidmegginson/ourairports-data/main/airports.csv";

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Airport {
    pub id: String,
    pub ident: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    pub latitude_deg: String,
    pub longitude_deg: String,
    pub elevation_ft: String,
    pub continent: String,
    pub iso_country: String,
    pub iso_region: String,
    pub municipality: String,
    pub scheduled_service: String,
    pub gps_code: String,
    pub iata_code: String,
    pub local_code: String,
    pub home_link: String,
    pub wikipedia_link: String,
    pub keywords: String,
}

static AIRPORTS: OnceLock<HashMap<String, Airport>> = OnceLock::new();

pub fn init_airports() -> Result<(), Box<dyn std::error::Error>> {
    let map = load_airports()?;
    AIRPORTS.set(map).map_err(|_| "AIRPORTS already initialized".into())
}

fn load_airports() -> Result<HashMap<String, Airport>, Box<dyn std::error::Error>> {
    let mut map = HashMap::new();
    let resp = reqwest::blocking::get(AIRPORTS_CSV_URL)?;
    let bytes = resp.bytes()?;
    let mut rdr = csv::Reader::from_reader(bytes.as_ref());
    for result in rdr.deserialize() {
        if let Ok(airport) = result {
            let airport: Airport = airport;
            // Insert by IATA code if present
            if !airport.iata_code.is_empty() {
                let iata = airport.iata_code.trim().to_uppercase();
                map.insert(iata, airport.clone());
            }
            // Insert by ICAO (ident) if present and not already in map
            if !airport.ident.is_empty() {
                let icao = airport.ident.trim().to_uppercase();
                if !map.contains_key(&icao) {
                    map.insert(icao, airport.clone());
                }
            }
        }
    }
    Ok(map)
}

fn get_airports() -> &'static HashMap<String, Airport> {
    AIRPORTS.get().expect("AIRPORTS not initialized. Call init_airports() before using airport functions.")
}

pub fn get_airport_by_iata(iata: &str) -> Option<&'static Airport> {
    let iata = iata.trim().to_uppercase();
    get_airports().get(&iata)
}

// Add a public function to get airport by ICAO code (ident)
pub fn get_airport_by_icao(icao: &str) -> Option<&'static Airport> {
    let icao = icao.trim().to_uppercase();
    get_airports().get(&icao)
}

// Add a public function to get the airport count
pub fn get_airport_count() -> usize {
    AIRPORTS.get().map(|m| m.len()).unwrap_or(0)
}

// Check if a given IATA code is a valid airport code
pub fn is_valid_airport_code(code: &str) -> bool {
    get_airport_by_iata(code).is_some()
}

/// Returns the IATA code if the station_id is a valid airport ICAO or IATA code.
pub fn get_airport_code_from_station(station_id: &str) -> Option<String> {
    let station_id = station_id.trim().to_uppercase();
    // If station_id is 4 letters and starts with 'K', treat as ICAO (e.g., KDCA)
    if station_id.len() == 4 && station_id.starts_with('K') {
        let iata_code = &station_id[1..];
        if is_valid_airport_code(iata_code) {
            return Some(iata_code.to_string());
        }
    }
    // Also check if the station_id itself is a valid IATA code (for non-ICAO stations)
    if is_valid_airport_code(&station_id) {
        return Some(station_id.to_string());
    }
    // If the station_id is a valid ICAO code, return its IATA code if present
    if let Some(airport) = get_airport_by_icao(&station_id) {
        if !airport.iata_code.is_empty() {
            return Some(airport.iata_code.clone());
        }
    }
    None
}

// Generate a Flightradar24 URL for an airport code
pub fn generate_flightradar24_url(airport_code: &str) -> String {
    format!("https://www.flightradar24.com/airport/{}", airport_code)
}

// Extracts ZIP code from an address string
pub fn extract_zip_code(address_str: &str) -> Option<String> {
    // Look for 5-digit zip code at the end of the address
    let parts: Vec<&str> = address_str.split(',').collect();
    if let Some(last_part) = parts.last() {
        // Try to find a 5-digit sequence in the last part (usually STATE, ZIP)
        let trimmed = last_part.trim();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        if let Some(last_word) = words.last() {
            // Check if it's a 5-digit number (US ZIP code)
            if last_word.len() == 5 && last_word.chars().all(|c| c.is_digit(10)) {
                return Some(last_word.to_string());
            }
        }
    }
    None
}

// Generate a Zillow URL for a ZIP code
pub fn generate_zillow_url(zip_code: &str) -> String {
    format!("https://www.zillow.com/homes/for_sale/{}", zip_code)
}
