use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::fmt;
use std::env;

#[macro_use]
extern crate lazy_static;

const DATA_DIR: &str = "data";
const ADDRESS_FILE: &str = "addresses.txt";
const APP_USER_AGENT: &str = "reather-app/0.1 (rust-cli-weather-app; https://github.com/yourusername/reather)"; // Replace with actual repo URL if available

// Hardcoded seed addresses from data/seed.txt
const SEED_ADDRESSES: [&str; 7] = [
    "233 E MAIN ST, BOZEMAN, MT, 59715",
    "1 MANELE RD, LANAI CITY, HI, 96763",
    "52 WHITEHEAD AVE, PORTLAND, ME, 04109",
    "22338 PACIFIC COAST HWY, MALIBU, CA, 90265",
    "58 OCEAN ST, ROCKLAND, ME, 04841",
    "100 SANKATY RD, NANTUCKET, MA, 02554",
    "1600 PENNSYLVANIA AVE NW, WASHINGTON, DC, 20500",
];

lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()
        .expect("Failed to build HTTP client");
}

// Custom Error Type
#[derive(Debug)]
enum AppError {
    Io(io::Error, Option<String>),
    Network(reqwest::Error),
    Api(String), // For API-specific errors (e.g., bad status, missing data)
    JsonParse(serde_json::Error),
    UserInput(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Io(err, path) => {
                if let Some(p) = path {
                    write!(f, "File I/O error for \'{}\': {}", p, err)
                } else {
                    write!(f, "File I/O error: {}", err)
                }
            }
            AppError::Network(err) => write!(f, "Network error: {}. Please check your internet connection.", err),
            AppError::Api(msg) => write!(f, "API error: {}", msg),
            AppError::JsonParse(err) => write!(f, "JSON parsing error: {}", err),
            AppError::UserInput(msg) => write!(f, "Invalid input: {}", msg),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(err, _) => Some(err),
            AppError::Network(err) => Some(err),
            AppError::JsonParse(err) => Some(err),
            _ => None,
        }
    }
}

// Convert io::Error to AppError
impl From<io::Error> for AppError {
    fn from(err: io::Error) -> AppError {
        AppError::Io(err, None)
    }
}
// Helper to create AppError::Io with a path
fn io_error_with_path(err: io::Error, path: &Path) -> AppError {
    AppError::Io(err, Some(path.display().to_string()))
}


// Convert reqwest::Error to AppError
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> AppError {
        AppError::Network(err)
    }
}

// Convert serde_json::Error to AppError
impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> AppError {
        AppError::JsonParse(err)
    }
}

// Structs for deserializing Census Geocoding API response
#[derive(Deserialize, Debug)]
struct GeocodeResponse {
    result: GeocodeResult,
}

#[derive(Deserialize, Debug)]
struct GeocodeResult {
    #[serde(rename = "addressMatches")]
    address_matches: Vec<AddressMatch>,
}

#[derive(Deserialize, Debug)]
struct AddressMatch {
    #[serde(rename = "matchedAddress")]
    matched_address: String,
    coordinates: Coordinates,
}

#[derive(Deserialize, Debug)]
struct Coordinates {
    x: f64, // longitude
    y: f64, // latitude
}

// Structs for deserializing NOAA API /points response
#[derive(Deserialize, Debug)]
struct NWSPointResponse {
    properties: NWSPointProperties,
}

#[derive(Deserialize, Debug)]
struct NWSPointProperties {
    #[serde(rename = "observationStations")]
    observation_stations_url: String, // URL to fetch list of observation stations
    #[serde(rename = "relativeLocation")]
    relative_location: Option<NWSRelativeLocation>, // For station name fallback
    forecast: String, // URL for the zone forecast
}

#[derive(Deserialize, Debug)]
struct NWSRelativeLocation {
    properties: NWSRelativeLocationProperties,
}

#[derive(Deserialize, Debug)]
struct NWSRelativeLocationProperties {
    city: String,
    state: String,
}


// Structs for deserializing NOAA API /stations response (list of stations)
#[derive(Deserialize, Debug)]
struct NWSStationsResponse {
    features: Vec<NWSStationFeature>,
}

#[derive(Deserialize, Debug)]
struct NWSStationFeature {
    properties: NWSStationProperties,
    geometry: Option<NWSGeometry>, // Added to capture station coordinates
}

#[derive(Deserialize, Debug)]
struct NWSStationProperties {
    #[serde(rename = "stationIdentifier")]
    station_identifier: String,
    name: String,
}

// Added struct to represent GeoJSON geometry for station coordinates
#[derive(Deserialize, Debug)]
struct NWSGeometry {
    coordinates: Option<Vec<f64>>, // [longitude, latitude]
}

// Structs for deserializing NOAA API /stations/{stationId}/observations/latest response
#[derive(Deserialize, Debug)]
struct WeatherObservationResponse {
    properties: Option<WeatherProperties>, // Make properties itself optional for robustness
}

#[derive(Deserialize, Debug)]
struct WeatherProperties {
    temperature: Option<WeatherValueUnit>,
    #[serde(rename = "heatIndex")]
    heat_index: Option<WeatherValueUnit>,
    #[serde(rename = "textDescription")]
    text_description: Option<String>,
    #[serde(rename = "windDirection")]
    wind_direction: Option<WeatherValueUnit>,
    wind_speed: Option<WeatherValueUnit>,
    #[serde(rename = "windGust")]
    wind_gust: Option<WeatherValueUnit>,
    #[serde(rename = "relativeHumidity")]
    relative_humidity: Option<WeatherValueUnit>,
    #[serde(rename = "cloudLayers")]
    cloud_layers: Option<Vec<CloudLayer>>, // Make cloud_layers optional
    visibility: Option<WeatherValueUnit>,
    #[serde(rename = "barometricPressure")]
    barometric_pressure: Option<WeatherValueUnit>,
    // Not strictly needed for display, but good to have if we want to show observation time
    // timestamp: Option<String>, 
}

#[derive(Deserialize, Debug)]
struct WeatherValueUnit {
    value: Option<f64>, // Value can be null for some fields like heatIndex or windGust
}

#[derive(Deserialize, Debug)]
struct CloudLayer {
    base: Option<CloudBase>,
    amount: Option<String>, // e.g., SKC, FEW, SCT, BKN, OVC
}

#[derive(Deserialize, Debug)]
struct CloudBase {
    value: Option<f64>, // meters
     // unit_code: Option<String>, // Typically "wmoUnit:m"
}

// Structs for deserializing NOAA API Forecast response
#[derive(Deserialize, Debug)]
struct ForecastResponse {
    properties: ForecastProperties,
}

#[derive(Deserialize, Debug)]
struct ForecastProperties {
    periods: Vec<ForecastPeriod>,
}

#[derive(Deserialize, Debug)]
struct ForecastPeriod {
    name: String,
    temperature: f64,
    #[serde(rename = "temperatureUnit")]
    temperature_unit: String,
    #[serde(rename = "detailedForecast")]
    detailed_forecast: String,
    // We're ignoring these fields as they're not used in our display
    #[serde(skip)]
    _wind_direction: Option<String>,
    #[serde(skip)]
    _wind_speed: Option<String>,
}

async fn geocode_address(
    address_query: &str,
) -> Result<Option<(String, f64, f64)>, AppError> {
    let benchmark = "Public_AR_Current";
    let format = "json";
    let url = format!(
        "https://geocoding.geo.census.gov/geocoder/locations/onelineaddress?address={}&benchmark={}&format={}", // Corrected URL
        urlencoding::encode(address_query),
        benchmark,
        format
    );

    // println!("Geocoding with URL: {}", url); // Debugging, can be removed

    let response = HTTP_CLIENT.get(&url).send().await.map_err(AppError::Network)?;

    if response.status().is_success() {
        let geocode_data: GeocodeResponse = response.json().await.map_err(|e| AppError::Api(format!("Failed to parse JSON response from geocoding service: {}", e)))?;
        if let Some(first_match) = geocode_data.result.address_matches.into_iter().next() {
            Ok(Some((
                first_match.matched_address, // Return the matched address string
                first_match.coordinates.y, // latitude
                first_match.coordinates.x, // longitude
            )))
        } else {
            Ok(None) // No matches found by the API
        }
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        Err(AppError::Api(format!(
            "Geocoding service returned an error (Status: {}). Details: {}",
            status, error_text
        )))
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Get the executable directory as a fallback location
    let exe_dir = env::current_exe()
        .map(|path| path.parent().map(|p| p.to_path_buf()))
        .unwrap_or(None)
        .unwrap_or_else(|| PathBuf::from("."));
    
    // Check if the data directory exists
    let data_dir_path = PathBuf::from(DATA_DIR);
    
    // Decide which directory to use for addresses.txt
    // Use data directory if it exists, otherwise use executable directory
    let addresses_path = if data_dir_path.exists() {
        data_dir_path.join(ADDRESS_FILE)
    } else {
        // Do NOT create the data directory, just use the executable directory
        exe_dir.join(ADDRESS_FILE)
    };

    if !addresses_path.exists() || addresses_path.metadata().map_err(|e| io_error_with_path(e, &addresses_path))?.len() == 0 {
        println!("\'{}\' is empty or does not exist.", addresses_path.display());
        println!("Would you like to populate it with seed addresses? (yes/no)");
        
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        if user_input.trim().eq_ignore_ascii_case("yes") {
            println!("Processing seed addresses...");
            let mut addresses_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&addresses_path)
                .map_err(|e| io_error_with_path(e, &addresses_path))?;

            for address_to_geocode in SEED_ADDRESSES.iter() {
                println!("Geocoding seed address: {}", address_to_geocode);
                match geocode_address(address_to_geocode).await {
                    Ok(Some((matched_address, lat, lon))) => {
                        writeln!(addresses_file, "{};{};{}", matched_address, lat, lon)
                            .map_err(|e| io_error_with_path(e, &addresses_path))?;
                        println!("  Stored: {};{};{}", matched_address, lat, lon);
                    }
                    Ok(None) => {
                        eprintln!("{}", AppError::Api(format!("Could not geocode seed address: '{}'. Skipping.", address_to_geocode)));
                    }
                    Err(e) => {
                        eprintln!("  Error geocoding seed address \'{}\': {}. Skipping.", address_to_geocode, e);
                    }
                }
            }
            println!("Seed addresses processed and stored in \'{}\'.", addresses_path.display());
        } else {
            println!("Skipping seed address population. You can add addresses manually.");
            File::create(&addresses_path).map_err(|e| io_error_with_path(e, &addresses_path))?; // Create empty addresses.txt
        }
    }

    loop {
        println!("\nMain Menu:");
        println!("1. Enter a new street address");
        println!("2. Choose from stored addresses");
        println!("3. Exit");
        print!("Please enter your choice: ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        match choice.trim() {
            "1" => {
                println!("Enter new address (e.g., 1600 Pennsylvania Ave NW, Washington, DC, 20500):");
                let mut new_address_query = String::new();
                io::stdin().read_line(&mut new_address_query)?;
                let new_address_query = new_address_query.trim();

                if !new_address_query.is_empty() {
                    match geocode_address(new_address_query).await {
                        Ok(Some((matched_address, lat, lon))) => {
                            add_address_to_file(&matched_address, lat, lon, &addresses_path)?;
                            println!("Address geocoded and added: {} (Lat: {}, Lon: {})", matched_address, lat, lon);
                            if let Err(e) = show_address_submenu(matched_address, lat, lon).await {
                                eprintln!("Error in address submenu: {}", e);
                            }
                        }
                        Ok(None) => {
                            eprintln!("{}", AppError::Api(format!("Could not find a match for the address: \'{}\'", new_address_query)));
                        }
                        Err(e) => {
                            eprintln!("Error geocoding address \'{}\': {}", new_address_query, e);
                        }
                    }
                } else {
                    eprintln!("{}", AppError::UserInput("Address cannot be empty. Please try again.".to_string()));
                }
            }
            "2" => {
                let stored_data = load_addresses(&addresses_path)?;
                if stored_data.is_empty() {
                    println!("No stored addresses found. Please add an address first (Option 1).");
                    continue;
                }
                println!("\nStored Addresses:");
                for (i, (addr, _, _)) in stored_data.iter().enumerate() {
                    println!("{}. {}", i + 1, addr);
                }
                println!("{}. Return to Main Menu", stored_data.len() + 1);
                print!("Select an address number or return: ");
                io::stdout().flush()?;

                let mut selection_str = String::new();
                io::stdin().read_line(&mut selection_str)?;
                match selection_str.trim().parse::<usize>() {
                    Ok(n) if n > 0 && n <= stored_data.len() => {
                        let (selected_address, lat, lon) = stored_data[n - 1].clone();
                        println!(); // Add a blank line for separation
                        println!("Selected address: {} (Lat: {}, Lon: {})", selected_address, lat, lon);
                        if let Err(e) = show_address_submenu(selected_address, lat, lon).await {
                             eprintln!("Error in address submenu: {}", e);
                        }
                    }
                    Ok(n) if n == stored_data.len() + 1 => continue,
                    Ok(_) => {
                        eprintln!("{}", AppError::UserInput("Invalid selection number. Please choose from the list.".to_string()));
                    }
                    Err(_) => {
                        eprintln!("{}", AppError::UserInput("Invalid input. Please enter a number corresponding to an address or to return.".to_string()));
                    }
                }
            }
            "3" => {
                println!("Exiting Reather. Goodbye!");
                break;
            }
            _ => eprintln!("{}", AppError::UserInput("Invalid choice. Please enter 1, 2, or 3.".to_string())),
        }
    }

    Ok(())
}

fn load_addresses(path: &Path) -> Result<Vec<(String, f64, f64)>, AppError> {
    if !path.exists() {
        return Ok(Vec::new()); // Not an error, just no file yet
    }
    let file = File::open(path).map_err(|e| io_error_with_path(e, path))?;
    let reader = BufReader::new(file);
    let mut addresses = Vec::new();
    for (line_num, line_result) in reader.lines().enumerate() {
        let line_content = line_result.map_err(|e| io_error_with_path(e, path))?;
        let parts: Vec<&str> = line_content.split(';').collect();
        if parts.len() == 3 {
            let lat_result = parts[1].parse::<f64>();
            let lon_result = parts[2].parse::<f64>();
            match (lat_result, lon_result) {
                (Ok(lat), Ok(lon)) => {
                    addresses.push((parts[0].to_string(), lat, lon));
                }
                _ => {
                    eprintln!(
                        "Warning: Malformed data in \'{}\' at line {}: Could not parse latitude/longitude for address \'{}\'. Skipping this entry.",
                        path.display(), line_num + 1, parts[0]
                    );
                }
            }
        } else if !line_content.trim().is_empty() { // Ignore empty lines silently
            eprintln!(
                "Warning: Malformed line in \'{}\' at line {}: \'{}\'. Expected 3 parts separated by semicolons. Skipping this entry.",
                path.display(), line_num + 1, line_content
            );
        }
    }
    Ok(addresses)
}

fn add_address_to_file(address: &str, lat: f64, lon: f64, path: &Path) -> Result<(), AppError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| io_error_with_path(e, path))?;
    writeln!(file, "{};{};{}", address, lat, lon).map_err(|e| io_error_with_path(e, path))?;
    Ok(())
}

async fn find_nearest_station(lat: f64, lon: f64) -> Result<Option<(String, String, Option<f64>, Option<f64>, String)>, AppError> {
    let points_url = format!("https://api.weather.gov/points/{},{}" , lat, lon); // Corrected URL
    // println!("Fetching station grid from: {}", points_url); // Debugging

    let points_response = HTTP_CLIENT.get(&points_url).send().await.map_err(AppError::Network)?;

    if !points_response.status().is_success() {
        let status = points_response.status();
        let text = points_response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        return Err(AppError::Api(format!(
            "NWS Points API request failed (Status: {}). URL: {}. Details: {}",
            status, points_url, text
        )));
    }

    let points_data: NWSPointResponse = points_response.json().await.map_err(|e| {
        AppError::Api(format!("Failed to parse JSON response from NWS Points API (URL: {}): {}", points_url, e))
    })?;
    
    let stations_url = points_data.properties.observation_stations_url; // Restored correct URL
    let forecast_url = points_data.properties.forecast;
    // println!("Fetching stations from: {}", stations_url); // Debugging

    let stations_response = HTTP_CLIENT.get(&stations_url).send().await.map_err(AppError::Network)?;

    if !stations_response.status().is_success() {
        let status = stations_response.status();
        let text = stations_response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        return Err(AppError::Api(format!(
            "NWS Stations API request failed (Status: {}). URL: {}. Details: {}",
            status, stations_url, text
        )));
    }

    let stations_data: NWSStationsResponse = stations_response.json().await.map_err(|e| {
         AppError::Api(format!("Failed to parse JSON response from NWS Stations API (URL: {}): {}", stations_url, e))
    })?;

    if let Some(first_station_feature) = stations_data.features.into_iter().next() {
        let station_id = first_station_feature.properties.station_identifier;
        let station_name = first_station_feature.properties.name;
        
        let (station_lat, station_lon) = 
            if let Some(geometry) = first_station_feature.geometry {
                if let Some(coords) = geometry.coordinates {
                    if coords.len() == 2 {
                        (Some(coords[1]), Some(coords[0])) // lat, lon
                    } else {
                        eprintln!("Warning: Station {} geometry coordinates array does not have 2 elements.", station_id);
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
        Ok(Some((station_id, station_name, station_lat, station_lon, forecast_url)))
    } else {
        // Attempt to use relative location as a fallback name if no stations are found
        let fallback_name = points_data.properties.relative_location
            .map(|loc| format!("area of {}, {}", loc.properties.city, loc.properties.state))
            .unwrap_or_else(|| "the specified location".to_string());
        println!("No observation stations found directly listed for {}. The weather data might be from a broader area.", fallback_name);
        // It might be better to return Ok(None) here if no station is truly found,
        // or a specific error/message indicating no stations.
        // For now, let's stick to the previous logic of a fallback if the API itself provides one.
        // The current structure implies `UNKNOWN_STATION_API_EMPTY` is a specific case.
        // Let's refine this: if `features` is empty, it means no stations.
        Ok(None)
    }
}

async fn show_address_submenu(address: String, lat: f64, lon: f64) -> Result<(), AppError> {
    println!(
        "\nOperating for address: {} (Lat: {}, Lon: {})",
        address, lat, lon
    );

    let mut station_id = "UNKNOWN_STATION".to_string();
    let mut station_name = "Unknown Station Name".to_string();
    let mut station_lat: Option<f64> = None;
    let mut station_lon: Option<f64> = None;
    let mut forecast_url: Option<String> = None;

    match find_nearest_station(lat, lon).await {
        Ok(Some((id, name, s_lat, s_lon, f_url))) => {
            station_id = id;
            station_name = name;
            station_lat = s_lat;
            station_lon = s_lon;
            forecast_url = Some(f_url);
            if let (Some(s_lat_val), Some(s_lon_val)) = (station_lat, station_lon) {
                println!("Found nearest station: {} ({}) - Lat: {}, Lon: {}", station_name, station_id, s_lat_val, s_lon_val);
            } else {
                println!("Found nearest station: {} ({}) (Coordinates not available from API)", station_name, station_id);
            }
        }
        Ok(None) => {
            eprintln!("{}", AppError::Api(format!("Could not find any nearby weather observation stations for the address at Lat: {}, Lon: {}", lat, lon)));
            // station_id remains "UNKNOWN_STATION", which fetch_and_display_weather handles
        }
        Err(e) => {
            eprintln!("Error finding nearest station for Lat: {}, Lon: {}: {}", lat, lon, e);
            // station_id remains "UNKNOWN_STATION"
        }
    }

    loop {
        println!("\n--- Submenu for {} ---", address);
        println!("1. Get Current Conditions");
        println!("2. Get Local Forecast");
        println!("3. External Links (Maps, Flights, Real Estate)");
        println!("4. Return to Main Menu");
        print!("Please enter your choice: ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;

        match choice.trim() {
            "1" => {
                if let Err(e) = fetch_and_display_weather(&station_id, &station_name).await {
                    eprintln!("Error fetching weather: {}", e);
                }
            }
            "2" => {
                if let Some(url) = &forecast_url {
                    if let Err(e) = fetch_and_display_local_forecast(url, &station_name).await {
                        eprintln!("Error fetching local forecast: {}", e);
                    }
                } else {
                    eprintln!("Forecast URL not available for this location.");
                }
            }
            "3" => {
                display_external_links(&address, lat, lon, &station_id, &station_name, station_lat, station_lon);
            }
            "4" => {
                println!("Returning to Main Menu...");
                break;
            }
            _ => eprintln!("{}", AppError::UserInput("Invalid choice, please try again.".to_string())),
        }
    }
    Ok(())
}

async fn fetch_and_display_local_forecast(forecast_url: &str, station_name: &str) -> Result<(), AppError> {
    println!("\nFetching local forecast for area near {}...", station_name);

    let response = HTTP_CLIENT.get(forecast_url).send().await.map_err(AppError::Network)?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        return Err(AppError::Api(format!(
            "NWS Forecast API request failed (Status: {}). URL: {}. Details: {}",
            status, forecast_url, text
        )));
    }

    let forecast_data: ForecastResponse = response.json().await.map_err(|e| {
        AppError::Api(format!("Failed to parse JSON response from NWS Forecast API (URL: {}): {}", forecast_url, e))
    })?;

    // Show detailed forecast for next 24 hours
    println!("\n--- Local Forecast for area near {} ---", station_name);
    
    if !forecast_data.properties.periods.is_empty() {
        // Get the first period (usually "Today" or "Tonight")
        let first_period = &forecast_data.properties.periods[0];
        println!("\n{} ({}°{})", first_period.name, first_period.temperature, first_period.temperature_unit);
        println!("{}", first_period.detailed_forecast);
    } else {
        println!("No forecast periods available for this location.");
    }

    Ok(())
}

fn display_external_links(address_str: &str, addr_lat: f64, addr_lon: f64, station_id: &str, station_name: &str, station_lat: Option<f64>, station_lon: Option<f64>) {
    println!("\n--- External Links (Maps, Flights, Real Estate) ---");
    
    // Address link
    println!("Address: {}", address_str);
    println!("  Google Maps: https://www.google.com/maps?q={},{}&ll={},{}&z=17&t=k", addr_lat, addr_lon, addr_lat, addr_lon);
    
    // Extract ZIP code and add Zillow link if available
    if let Some(zip_code) = extract_zip_code(address_str) {
        println!("  Zillow: {}", generate_zillow_url(&zip_code));
    }

    if let (Some(s_lat), Some(s_lon)) = (station_lat, station_lon) {
        println!("\n");
        println!("Weather Station: {} ({})", station_name, if station_id.starts_with("UNKNOWN_STATION") {"ID N/A"} else {station_id});
        println!("  Google Maps: https://www.google.com/maps?q={},{}&ll={},{}&z=17&t=k", s_lat, s_lon, s_lat, s_lon);
        
        // Check if the station is at an airport and add Flightradar24 link if it is
        if !station_id.starts_with("UNKNOWN_STATION") {
            if let Some(airport_code) = get_airport_info(station_id) {
                println!("  This weather station is at an airport.");
                println!("  Flightradar24: {}", generate_flightradar24_url(&airport_code));
            }
        }
    } else {
        println!("Weather Station: {} ({}) (Coordinates not available for map link)", station_name, if station_id.starts_with("UNKNOWN_STATION") {"ID N/A"} else {station_id});
    }
}

async fn fetch_and_display_weather(station_id: &str, station_name: &str) -> Result<(), AppError> {
    if station_id.starts_with("UNKNOWN_STATION") { // Covers UNKNOWN_STATION and UNKNOWN_STATION_API_EMPTY
        eprintln!("Cannot fetch weather: Station ID is unknown or no station was found.");
        return Ok(()); // Not an error in program flow, but an inability to proceed.
    }
    let url = format!(
        "https://api.weather.gov/stations/{}/observations/latest", // Corrected URL
        station_id
    );
    // println!("Fetching weather from: {}", url); // Debugging

    let response = HTTP_CLIENT.get(&url).send().await.map_err(AppError::Network)?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
        return Err(AppError::Api(format!(
            "NWS Observations API request failed (Status: {}). URL: {}. Details: {}",
            status, url, text
        )));
    }

    // Now that we know the request was successful, we can parse the JSON.
    let weather_data_response: WeatherObservationResponse = response.json().await.map_err(|e| {
        AppError::Api(format!("Failed to parse JSON response from NWS Observations API (URL: {}): {}", url, e))
    })?;

    if let Some(properties) = weather_data_response.properties {
        println!(
            "\n--- Current Conditions at {} ({}) ---",
            station_name,
            station_id
        );

        // Temperature (Celsius to Fahrenheit)
        let temp_str = properties.temperature.as_ref()
            .and_then(|t| t.value.map(|c| format!("{:.1} °F", c * 9.0/5.0 + 32.0)))
            .unwrap_or_else(|| "N/A".to_string());
        println!("Temperature: {}", temp_str);

        // Heat Index (Celsius to Fahrenheit)
        let heat_index_str = properties.heat_index.as_ref()
            .and_then(|hi| hi.value.map(|c| format!("{:.1} °F", c * 9.0/5.0 + 32.0)))
            .unwrap_or_else(|| "N/A".to_string());
        println!("Heat Index: {}", heat_index_str);

        // Conditions
        println!("Conditions: {}", properties.text_description.as_deref().unwrap_or("N/A"));

        // Wind (m/s to mph)
        let wind_dir_str = properties.wind_direction.as_ref()
            .and_then(|wd| wd.value.map(|v| format!("{:.0}", v)));
        let wind_speed_str = properties.wind_speed.as_ref()
            .and_then(|ws| ws.value.map(|mps| format!("{:.1} mph", mps * 2.23694)));
        
        let wind_str = match (wind_dir_str, wind_speed_str) {
            (Some(dir), Some(speed)) => {
                let gust_str = properties.wind_gust.as_ref()
                    .and_then(|wg| wg.value.map(|mps_gust| format!(" (gusts to {:.1} mph)", mps_gust * 2.23694)))
                    .unwrap_or_else(|| "".to_string());
                format!("{} deg at {}{}", dir, speed, gust_str)
            }
            _ => "N/A".to_string(),
        };
        println!("Wind: {}", wind_str);
        
        // Humidity
        let humidity_str = properties.relative_humidity.as_ref()
            .and_then(|rh| rh.value.map(|v| format!("{:.1} %", v)))
            .unwrap_or_else(|| "N/A".to_string());
        println!("Humidity: {}", humidity_str);

        // Ceiling (meters to feet)
        // Prioritize "SKC" or "CLR" if present, otherwise use the first layer with a base value.
        let ceiling_str = properties.cloud_layers.as_ref().and_then(|layers| {
            if layers.iter().any(|layer| matches!(layer.amount.as_deref(), Some("SKC") | Some("CLR"))) {
                Some("Clear (>12,000 ft)".to_string())
            } else {
                layers.iter().find_map(|layer| {
                    layer.base.as_ref().and_then(|b| b.value.map(|meters| format!("{:.0} ft", meters * 3.28084)))
                })
            }
        }).unwrap_or_else(|| "N/A".to_string());
        println!("Ceiling: {}", ceiling_str);

        // Visibility (meters to miles)
        let visibility_str = properties.visibility.as_ref()
            .and_then(|v| v.value.map(|meters| format!("{:.1} mi", meters * 0.000621371)))
            .unwrap_or_else(|| "N/A".to_string());
        println!("Visibility: {}", visibility_str);

        // Pressure (Pascals to inHg)
        let pressure_str = properties.barometric_pressure.as_ref()
            .and_then(|p| p.value.map(|pa| format!("{:.2} inHg", pa * 0.0002953)))
            .unwrap_or_else(|| "N/A".to_string());
        println!("Pressure: {}", pressure_str);

    } else {
        println!("Weather data properties are missing in the API response for station {}.", station_id);
    }
    Ok(())
}

// Extracts the zip code from a US address string.
// Expects format like "123 MAIN ST, CITY, STATE, 12345" or similar.
fn extract_zip_code(address_str: &str) -> Option<String> {
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

// Determines if a weather station is at an airport and returns its airport code
fn get_airport_info(station_id: &str) -> Option<String> {
    // Most US airport weather stations have IDs starting with K followed by the 3-letter IATA code
    // Example: KBOS for Boston Logan International Airport
    if station_id.len() == 4 && station_id.starts_with('K') {
        let iata_code = &station_id[1..4];
        return Some(iata_code.to_string());
    }
    
    // Some stations at smaller airports might follow other patterns
    // Future enhancement: Add more pattern matching or a lookup table
    
    None
}

// Generates a Flightradar24 URL for the given airport code
fn generate_flightradar24_url(airport_code: &str) -> String {
    format!("https://www.flightradar24.com/airport/{}", airport_code)
}

// Generates a Zillow URL for the given ZIP code
fn generate_zillow_url(zip_code: &str) -> String {
    format!("https://www.zillow.com/homes/for_sale/{}", zip_code)
}
