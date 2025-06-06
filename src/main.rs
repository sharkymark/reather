use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::fmt;
use std::env;

mod airports;

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

#[derive(Debug, Deserialize)]
struct EarthquakeFeatureCollection {
    features: Vec<EarthquakeFeature>,
}

#[derive(Debug, Deserialize)]
struct EarthquakeFeature {
    properties: EarthquakeProperties,
    geometry: Option<EarthquakeGeometry>,
}

#[derive(Debug, Deserialize)]
struct EarthquakeProperties {
    mag: Option<f64>,
    place: Option<String>,
    time: Option<i64>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EarthquakeGeometry {
    coordinates: Vec<f64>, // [lon, lat, depth]
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    airports::init_airports()?;
    let airport_count = airports::get_airport_count();
    println!("Reather - a Rust-based Weather App");
    println!("USA airport database loaded: {} airports", airport_count);
    println!("");
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())?)
}

async fn async_main() -> Result<(), AppError> {
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

    // --- NEW: Geocode addresses missing lat/lon in addresses.txt ---
    if addresses_path.exists() {
        let file = File::open(&addresses_path).map_err(|e| io_error_with_path(e, &addresses_path))?;
        let reader = BufReader::new(file);
        let mut lines: Vec<String> = Vec::new();
        let mut changed = false;
        for line in reader.lines() {
            let line = line.map_err(|e| io_error_with_path(e, &addresses_path))?;
            let parts: Vec<&str> = line.split(';').collect();
            if parts.len() == 1 && !parts[0].trim().is_empty() {
                // Only address, missing lat/lon
                let addr = parts[0].trim();
                match geocode_address(addr).await {
                    Ok(Some((matched_address, lat, lon))) => {
                        lines.push(format!("{};{};{}", matched_address.to_uppercase(), lat, lon));
                        changed = true;
                    }
                    Ok(None) => {
                        // Could not geocode, keep as is
                        lines.push(line);
                    }
                    Err(e) => {
                        eprintln!("Error geocoding address '{}': {}. Keeping as is.", addr, e);
                        lines.push(line);
                    }
                }
            } else if parts.len() == 3 {
                // Replace address with uppercase matched address if possible
                let addr = parts[0].trim();
                match geocode_address(addr).await {
                    Ok(Some((matched_address, lat, lon))) => {
                        // Only update if lat/lon match what's in the file
                        let lat_ok = format!("{:.8}", lat) == format!("{:.8}", parts[1].parse::<f64>().unwrap_or(lat));
                        let lon_ok = format!("{:.8}", lon) == format!("{:.8}", parts[2].parse::<f64>().unwrap_or(lon));
                        if lat_ok && lon_ok {
                            lines.push(format!("{};{};{}", matched_address.to_uppercase(), lat, lon));
                            changed = true;
                        } else {
                            lines.push(line);
                        }
                    }
                    _ => lines.push(line),
                }
            } else {
                lines.push(line);
            }
        }
        if changed {
            let mut file = OpenOptions::new().write(true).truncate(true).open(&addresses_path).map_err(|e| io_error_with_path(e, &addresses_path))?;
            for l in &lines {
                writeln!(file, "{}", l).map_err(|e| io_error_with_path(e, &addresses_path))?;
            }
        }
    }

    if !addresses_path.exists() {
        println!("\'{}\' does not exist.", addresses_path.display());
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
            // Write a special marker comment to indicate user explicitly declined seed data
            let mut marker_file = OpenOptions::new()
                .write(true)
                .open(&addresses_path)
                .map_err(|e| io_error_with_path(e, &addresses_path))?;
            writeln!(marker_file, "# User declined seed data").map_err(|e| io_error_with_path(e, &addresses_path))?;
        }
    } else if addresses_path.metadata().map_err(|e| io_error_with_path(e, &addresses_path))?.len() == 0 {
        // File exists but is completely empty - we should ask about seed data
        println!("\'{}\' exists but is empty.", addresses_path.display());
        println!("Would you like to populate it with seed addresses? (yes/no)");
        
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        if user_input.trim().eq_ignore_ascii_case("yes") {
            println!("Processing seed addresses...");
            let mut addresses_file = OpenOptions::new()
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
            // Write a special marker comment to indicate user explicitly declined seed data
            let mut marker_file = OpenOptions::new()
                .write(true)
                .open(&addresses_path)
                .map_err(|e| io_error_with_path(e, &addresses_path))?;
            writeln!(marker_file, "# User declined seed data").map_err(|e| io_error_with_path(e, &addresses_path))?;
        }
    }

    loop {
        println!("\nMain Menu:");
        println!("1. Enter a new street address");
        println!("2. Choose from stored addresses");
        println!("3. Airport search");
        println!("4. Earthquakes");
        println!("5. Tides");
        println!("6. Exit");
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
                airport_search_menu().await?;
            }
            "4" => {
                earthquake_menu().await?;
            }
            "5" => {
                tides_menu().await?;
            }
            "6" => {
                println!("Exiting Reather. Goodbye!");
                break;
            }
            _ => eprintln!("{}", AppError::UserInput("Invalid choice. Please enter 1, 2, 3, 4, 5, or 6.".to_string())),
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
        
        // Skip comment lines (starting with #)
        if line_content.trim().starts_with('#') {
            continue;
        }
        
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
                display_external_links(&address, lat, lon, &station_id, &station_name, station_lat, station_lon).await;
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

async fn display_external_links(address_str: &str, addr_lat: f64, addr_lon: f64, station_id: &str, station_name: &str, station_lat: Option<f64>, station_lon: Option<f64>) {
    println!("\n--- External Links (Maps, Flights, Real Estate) ---");
    
    // Address link
    println!("Address: {}", address_str);
    println!("  Google Maps: https://www.google.com/maps?q={},{}&ll={},{}&z=17&t=k", addr_lat, addr_lon, addr_lat, addr_lon);
    
    // Extract ZIP code and add Zillow link if available
    if let Some(zip_code) = airports::extract_zip_code(address_str) {
        println!("  Zillow: {}", airports::generate_zillow_url(&zip_code));
    }

    if let (Some(s_lat), Some(s_lon)) = (station_lat, station_lon) {
        println!("\n");
        println!("Weather Station: {} ({})", station_name, if station_id.starts_with("UNKNOWN_STATION") {"ID N/A"} else {station_id});
        println!("  Google Maps: https://www.google.com/maps?q={},{}", s_lat, s_lon);
        if !station_id.starts_with("UNKNOWN_STATION") {
            // Try to find the best public airport code (IATA preferred, else ICAO, never internal codes)
            let mut flightradar_code = String::new();
            let mut airport_name = None;
            // Try IATA via get_airport_code_from_station
            if let Some(iata) = airports::get_airport_code_from_station(station_id) {
                if iata.len() == 3 && iata.chars().all(|c| c.is_ascii_alphanumeric()) {
                    flightradar_code = iata.clone();
                    if let Some(airport) = airports::get_airport_by_iata(&iata) {
                        airport_name = Some(airport.name.clone());
                    }
                }
            }
            // If not found, try ICAO
            if flightradar_code.is_empty() && station_id.len() == 4 && station_id.chars().all(|c| c.is_ascii_alphanumeric()) {
                if let Some(airport) = airports::get_airport_by_icao(station_id) {
                    let icao = airport.ident.trim();
                    if icao.len() == 4 && icao.chars().all(|c| c.is_ascii_alphanumeric()) && !icao.starts_with("US-") && !icao.starts_with("MT") {
                        flightradar_code = icao.to_string();
                        airport_name = Some(airport.name.clone());
                    }
                }
            }
            if !flightradar_code.is_empty() {
                println!("  This weather station is at a verified airport{}.", airport_name.as_ref().map(|n| format!(" ({} )", n)).unwrap_or_default());
                println!("  Flightradar24: {}", airports::generate_flightradar24_url(&flightradar_code));
                if let Some(airport) = airports::get_airport_by_iata(&flightradar_code) {
                    if !airport.home_link.trim().is_empty() {
                        println!("  Official Airport Website: {}", airport.home_link.trim());
                    }
                    if !airport.wikipedia_link.trim().is_empty() {
                        println!("  Wikipedia: {}", airport.wikipedia_link.trim());
                    }
                } else if let Some(airport) = airports::get_airport_by_icao(&flightradar_code) {
                    if !airport.home_link.trim().is_empty() {
                        println!("  Official Airport Website: {}", airport.home_link.trim());
                    }
                    if !airport.wikipedia_link.trim().is_empty() {
                        println!("  Wikipedia: {}", airport.wikipedia_link.trim());
                    }
                }
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

async fn airport_search_menu() -> Result<(), AppError> {
    use std::io::Write;
    let us_states = [
        "AL","AK","AZ","AR","CA","CO","CT","DE","FL","GA","HI","ID","IL","IN","IA","KS","KY","LA","ME","MD","MA","MI","MN","MS","MO","MT","NE","NV","NH","NJ","NM","NY","NC","ND","OH","OK","OR","PA","RI","SC","SD","TN","TX","UT","VT","VA","WA","WV","WI","WY"
    ];
    let us_only;
    let passenger_only;
    // Ask user if they want US states only
    loop {
        println!("\n--- Airport Search ---");
        println!("Do you want to search only US states? (Y/n): ");
        io::stdout().flush()?;
        let mut us_only_input = String::new();
        io::stdin().read_line(&mut us_only_input)?;
        let us_only_input = us_only_input.trim();
        if us_only_input.is_empty() || us_only_input.eq_ignore_ascii_case("y") {
            us_only = true;
            break;
        } else if us_only_input.eq_ignore_ascii_case("n") {
            us_only = false;
            break;
        } else {
            println!("Please enter Y for US states only, or N for all airports.");
        }
    }
    // Ask user if they want passenger airports only
    loop {
        println!("Do you want to search only passenger airports (scheduled service)? (Y/n): ");
        io::stdout().flush()?;
        let mut passenger_only_input = String::new();
        io::stdin().read_line(&mut passenger_only_input)?;
        let passenger_only_input = passenger_only_input.trim();
        if passenger_only_input.is_empty() || passenger_only_input.eq_ignore_ascii_case("y") {
            passenger_only = true;
            break;
        } else if passenger_only_input.eq_ignore_ascii_case("n") {
            passenger_only = false;
            break;
        } else {
            println!("Please enter Y for passenger airports only, or N for all airports.");
        }
    }
    loop {
        println!("\n--- Airport Search ---");
        println!("Search by airport code, state, municipality, or name. Use * as a wildcard: 'Rome*' for names starting with Rome, '*Rome' for names ending with Rome, '*Rome*' for names containing Rome, or 'Rome' for exact match.");
        print!("Enter search term (or just press Enter to return to main menu): ");
        io::stdout().flush()?;
        let mut search = String::new();
        io::stdin().read_line(&mut search)?;
        let search = search.trim();
        if search.is_empty() {
            break;
        }
        let mut results = airports::search_airports(search);
        if us_only {
            results.retain(|a| {
                let region = a.iso_region.trim();
                region.starts_with("US-") && us_states.contains(&region[3..].to_uppercase().as_str())
            });
        }
        if passenger_only {
            results.retain(|a| a.scheduled_service == "yes");
        }
        if results.is_empty() {
            println!("No airports found matching '{}'.", search);
            continue;
        }
        'result_loop: loop {
            println!("\nAirports found:");
            for (i, airport) in results.iter().enumerate() {
                println!("{}. {} ({}) - {}, {}", i + 1, airport.name, airport.ident, airport.municipality, airport.iso_region);
            }
            print!("\nSelect an airport by number, or type 's' to start search over, or 'm' to return to main menu: ");
            io::stdout().flush()?;
            let mut sel = String::new();
            io::stdin().read_line(&mut sel)?;
            let sel = sel.trim();
            if sel.eq_ignore_ascii_case("s") {
                break;
            } else if sel.eq_ignore_ascii_case("m") {
                return Ok(());
            } else if let Ok(idx) = sel.parse::<usize>() {
                if idx > 0 && idx <= results.len() {
                    show_airport_details(&results[idx - 1]).await?;
                    // After showing details, offer to select another, start over, or return
                    loop {
                        println!("\nOptions:");
                        println!("1. Select another airport from the filtered list");
                        println!("2. Start search over again");
                        println!("3. Return to main menu");
                        print!("Enter your choice: ");
                        io::stdout().flush()?;
                        let mut opt = String::new();
                        io::stdin().read_line(&mut opt)?;
                        match opt.trim() {
                            "1" => continue 'result_loop, // re-show the list and prompt again
                            "2" => break 'result_loop,    // start search over
                            _ => return Ok(()), // return to main menu
                        }
                    }
                } else {
                    println!("Invalid selection.");
                }
            } else {
                println!("Invalid input.");
            }
        }
    }
    Ok(())
}

async fn show_airport_details(airport: &airports::Airport) -> Result<(), AppError> {
    println!("\nLatitude: {}, Longitude: {}", airport.latitude_deg, airport.longitude_deg);
    println!("\nAirport Weather Conditions:");
    println!("Station ID: {}", airport.ident);
    println!("Labeled as: {}", airport.name);
    println!("Station Name: {}", airport.municipality);
    let lat = airport.latitude_deg.parse::<f64>().ok();
    let lon = airport.longitude_deg.parse::<f64>().ok();
    let mut temp = None;
    let mut wind_speed = None;
    let mut wind_dir = None;
    let mut conditions = None;
    let mut forecast = None;
    let mut found_weather = false;
    let mut station_lat_lon: Option<(Option<f64>, Option<f64>)> = None;
    if let (Some(lat), Some(lon)) = (lat, lon) {
        if let Ok(Some((station_id, station_name, station_lat, station_lon, forecast_url))) = find_nearest_station(lat, lon).await {
            // Store the station's coordinates
            station_lat_lon = Some((station_lat, station_lon));
            // Fetch current conditions
            if let Ok(response) = HTTP_CLIENT.get(&format!("https://api.weather.gov/stations/{}/observations/latest", station_id)).send().await {
                if let Ok(obs) = response.json::<WeatherObservationResponse>().await {
                    if let Some(props) = obs.properties {
                        temp = props.temperature.and_then(|t| t.value);
                        wind_speed = props.wind_speed.and_then(|w| w.value);
                        wind_dir = props.wind_direction.and_then(|w| w.value);
                        conditions = props.text_description;
                        found_weather = temp.is_some() || wind_speed.is_some() || wind_dir.is_some() || conditions.is_some();
                    }
                }
            }
            // Fetch forecast
            if let Ok(response) = HTTP_CLIENT.get(&forecast_url).send().await {
                if let Ok(forecast_data) = response.json::<ForecastResponse>().await {
                    if let Some(first) = forecast_data.properties.periods.first() {
                        forecast = Some(first.detailed_forecast.clone());
                    }
                }
            }
            println!("\nStation ID: {}", station_id);
            println!("Labeled as: {}", airport.name);
            println!("Station Name: {}", station_name);
        }
    }
    if found_weather {
        println!("Current Conditions:");
        println!("Temperature: {}", temp.map(|t| format!("{:.1} °F", t * 9.0/5.0 + 32.0)).unwrap_or("None".to_string()));
        println!("Wind Speed: {}", wind_speed.map(|w| format!("{:.1} mph", w * 2.23694)).unwrap_or("None".to_string()));
        println!("Wind Direction: {}", wind_dir.map(|w| format!("{:.0}", w)).unwrap_or("None".to_string()));
        if let Some(cond) = &conditions {
            println!("Current Conditions: {}", cond);
        }
        else {
            println!("N/A");
        }
        if let Some(forecast) = &forecast {
            println!("Forecast: {}", forecast);
        }
    } else {
        println!("No weather data available for this airport (may be international, a military or remote field).\n");
    }
    println!("\n");

    if let (Some(lat), Some(lon)) = (lat, lon) {
        println!("Google Maps (Airport Location): https://www.google.com/maps?q={},{}", lat, lon);
        
        // Add the weather station location link if available
        if let Some((Some(station_lat), Some(station_lon))) = station_lat_lon {
            println!("Google Maps (Weather Station Location): https://www.google.com/maps?q={},{}", station_lat, station_lon);
        }
        
        // Only show Flightradar24 if weather was found (i.e., likely a public airport)
        if found_weather {
            // Prefer IATA, then ICAO, then skip if neither is valid
            let iata = airport.iata_code.trim();
            let icao = airport.ident.trim();
            let flightradar_code = if iata.len() == 3 && iata.chars().all(|c| c.is_ascii_alphanumeric()) {
                iata
            } else if icao.len() == 4 && icao.chars().all(|c| c.is_ascii_alphanumeric()) {
                icao
            } else {
                ""
            };
            if !flightradar_code.is_empty() {
                println!("Flightradar24: https://www.flightradar24.com/airport/{}", flightradar_code);
            }
        }
        // Zillow links for US states only (always print after other output)
        let us_states = [
            "AL","AK","AZ","AR","CA","CO","CT","DE","FL","GA","HI","ID","IL","IN","IA","KS","KY","LA","ME","MD","MA","MI","MN","MS","MO","MT","NE","NV","NH","NJ","NM","NY","NC","ND","OH","OK","OR","PA","RI","SC","SD","TN","TX","UT","VT","VA","WA","WV","WI","WY"
        ];
        let state_name_to_abbr = [
            ("Alabama", "AL"), ("Alaska", "AK"), ("Arizona", "AZ"), ("Arkansas", "AR"), ("California", "CA"), ("Colorado", "CO"), ("Connecticut", "CT"), ("Delaware", "DE"), ("Florida", "FL"), ("Georgia", "GA"), ("Hawaii", "HI"), ("Idaho", "ID"), ("Illinois", "IL"), ("Indiana", "IN"), ("Iowa", "IA"), ("Kansas", "KS"), ("Kentucky", "KY"), ("Louisiana", "LA"), ("Maine", "ME"), ("Maryland", "MD"), ("Massachusetts", "MA"), ("Michigan", "MI"), ("Minnesota", "MN"), ("Mississippi", "MS"), ("Missouri", "MO"), ("Montana", "MT"), ("Nebraska", "NE"), ("Nevada", "NV"), ("New Hampshire", "NH"), ("New Jersey", "NJ"), ("New Mexico", "NM"), ("New York", "NY"), ("North Carolina", "NC"), ("North Dakota", "ND"), ("Ohio", "OH"), ("Oklahoma", "OK"), ("Oregon", "OR"), ("Pennsylvania", "PA"), ("Rhode Island", "RI"), ("South Carolina", "SC"), ("South Dakota", "SD"), ("Tennessee", "TN"), ("Texas", "TX"), ("Utah", "UT"), ("Vermont", "VT"), ("Virginia", "VA"), ("Washington", "WA"), ("West Virginia", "WV"), ("Wisconsin", "WI"), ("Wyoming", "WY")
        ];
        let mut zillow_printed = false;
        // Await both lookups before printing
        let county_state = get_county_state_from_latlon(lat, lon).await;
        let city_state = get_city_state_from_latlon(lat, lon).await;
        // Remove debug output for Zillow troubleshooting
        if let Some(county_state) = county_state {
            if let Some(state_abbr) = county_state.split('-').last() {
                let state_abbr = state_abbr.trim();
                let state_abbr = state_name_to_abbr.iter().find_map(|(name, abbr)| {
                    if state_abbr.eq_ignore_ascii_case(name) { Some(*abbr) } else { None }
                }).unwrap_or(state_abbr);
                if us_states.contains(&state_abbr) {
                    println!("Zillow (county): https://www.zillow.com/homes/for_sale/{}", county_state.replace(' ', "+"));
                    zillow_printed = true;
                }
            }
        }
        if let Some(city_state) = city_state {
            if let Some(state_abbr) = city_state.split('-').last() {
                let state_abbr = state_abbr.trim();
                let state_abbr = state_name_to_abbr.iter().find_map(|(name, abbr)| {
                    if state_abbr.eq_ignore_ascii_case(name) { Some(*abbr) } else { None }
                }).unwrap_or(state_abbr);
                if us_states.contains(&state_abbr) {
                    println!("Zillow (city): https://www.zillow.com/homes/for_sale/{}", city_state.replace(' ', "+"));
                    zillow_printed = true;
                }
            }
        }
        if !zillow_printed {
            println!("No Zillow links available for this location.");
        }
    }
    Ok(())
}

use serde_json::Value;

async fn get_county_state_from_latlon(lat: f64, lon: f64) -> Option<String> {
    let url = format!("https://geo.fcc.gov/api/census/block/find?latitude={}&longitude={}&format=json", lat, lon);
    match HTTP_CLIENT.get(&url).send().await {
        Ok(resp) => {
            if let Ok(json) = resp.json::<Value>().await {
                let county = json.get("County").and_then(|c| c.get("name")).and_then(|v| v.as_str());
                let state = json.get("State").and_then(|s| s.get("name")).and_then(|v| v.as_str());
                if let (Some(county), Some(state)) = (county, state) {
                    return Some(format!("{}-{}", county, state));
                }
            }
        }
        Err(_) => {}
    }
    None
}

async fn get_city_state_from_latlon(lat: f64, lon: f64) -> Option<String> {
    let url = format!("https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={}&lon={}", lat, lon);
    match HTTP_CLIENT.get(&url)
        .header("User-Agent", APP_USER_AGENT)
        .send().await {
        Ok(resp) => {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(addr) = json.get("address") {
                    let city = addr.get("city").or_else(|| addr.get("town")).or_else(|| addr.get("village")).and_then(|v| v.as_str());
                    let state = addr.get("state").and_then(|v| v.as_str());
                    if let (Some(city), Some(state)) = (city, state) {
                        return Some(format!("{}-{}", city, state));
                    }
                }
            }
        }
        Err(_) => {}
    }
    None
}

// --- Earthquake menu and logic ---

async fn earthquake_menu() -> Result<(), AppError> {
    use std::io::Write;
    println!("\n--- Earthquakes ---");
    println!("Select minimum magnitude:");
    println!("1. All Earthquakes");
    println!("2. M5.0+");
    println!("3. M6.0+");
    println!("4. M7.0+");
    print!("Enter your choice (1-4, or Enter to return): ");
    io::stdout().flush()?;
    let mut mag_choice = String::new();
    io::stdin().read_line(&mut mag_choice)?;
    let mag_choice = mag_choice.trim();
    if mag_choice.is_empty() { return Ok(()); }
    let mag_val = match mag_choice {
        "1" => 0.0,
        "2" => 5.0,
        "3" => 6.0,
        "4" => 7.0,
        _ => {
            println!("Invalid choice.");
            return Ok(());
        }
    };
    println!("Select time period:");
    println!("1. Past 24 hours");
    println!("2. Past 48 hours");
    println!("3. Past 7 days");
    print!("Enter your choice (1-3, or Enter to return): ");
    io::stdout().flush()?;
    let mut time_choice = String::new();
    io::stdin().read_line(&mut time_choice)?;
    let time_choice = time_choice.trim();
    if time_choice.is_empty() { return Ok(()); }
    let (url, filter_hours) = match time_choice {
        "1" => ("https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_day.geojson", 24),
        "2" => ("https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_day.geojson", 48),
        "3" => ("https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_week.geojson", 168),
        _ => {
            println!("Invalid choice.");
            return Ok(());
        }
    };
    println!("\nFetching earthquake data from USGS...");
    match fetch_and_display_earthquakes_filtered(url, mag_val, filter_hours).await {
        Ok(_) => {},
        Err(e) => eprintln!("Error fetching earthquake data: {}", e),
    }
    Ok(())
}

async fn fetch_and_display_earthquakes_filtered(url: &str, min_mag: f64, max_age_hours: u64) -> Result<(), AppError> {
    let resp = HTTP_CLIENT.get(url).send().await?.error_for_status()?;
    let data: EarthquakeFeatureCollection = resp.json().await?;
    if data.features.is_empty() {
        println!("No earthquakes found for this selection.");
        return Ok(());
    }
    use chrono::{Utc, TimeZone};
    let now = Utc::now();
    let max_age = chrono::Duration::hours(max_age_hours as i64);
    let mut shown = 0;
    println!("\nRecent Earthquakes:");
    for feature in data.features.iter() {
        let mag = feature.properties.mag.unwrap_or(-999.0);
        if mag < min_mag { continue; }
        let time_ms = feature.properties.time.unwrap_or(0);
        let event_time = chrono::Utc.timestamp_millis_opt(time_ms).single();
        if let Some(event_time) = event_time {
            if now.signed_duration_since(event_time) > max_age { continue; }
        } else {
            continue;
        }
        shown += 1;
        let mag_str = if mag < 0.0 { "?".to_string() } else { format!("{:.1}", mag) };
        let place = feature.properties.place.as_deref().unwrap_or("Unknown location");
        let time = feature.properties.time.map(|t| format_utc_time(t)).unwrap_or("?".to_string());
        let url = feature.properties.url.as_deref().unwrap_or("");
        let (lat, lon, depth) = if let Some(geom) = &feature.geometry {
            let coords = &geom.coordinates;
            if coords.len() >= 3 {
                (Some(coords[1]), Some(coords[0]), Some(coords[2]))
            } else if coords.len() == 2 {
                (Some(coords[1]), Some(coords[0]), None)
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };
        println!("{}. M{} | {} | {}", shown, mag_str, place, time);
        if let (Some(lat), Some(lon)) = (lat, lon) {
            println!("    Location: {:.3}, {:.3} | Depth: {} km", lat, lon, depth.map(|d| format!("{:.1}", d)).unwrap_or("?".to_string()));
            println!("    Google Maps: https://www.google.com/maps?q={},{}&ll={},{}&z=7", lat, lon, lat, lon);
        }
        if !url.is_empty() {
            println!("    More info: {}", url);
        }
    }
    if shown == 0 {
        println!("No earthquakes found for this selection.");
    }
    Ok(())
}

fn format_utc_time(ms_since_epoch: i64) -> String {
    use std::time::{UNIX_EPOCH, Duration};
    use chrono::{DateTime, Utc};
    let dt = UNIX_EPOCH + Duration::from_millis(ms_since_epoch as u64);
    let datetime: DateTime<Utc> = DateTime::<Utc>::from(dt);
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

// --- Tides menu and logic ---

#[derive(Debug, Deserialize)]
struct NWSNoaaTideStation {
    id: String,
    name: String,
    state: String,
    lat: f64,
    lon: f64,
}

async fn tides_menu() -> Result<(), AppError> {
    use std::io::Write;
    println!("\n--- Tides ---");
    println!("1. Lookup tides by address");
    println!("2. Lookup tides by airport (US only)");
    println!("3. Return to main menu");
    print!("Enter your choice: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    match choice.trim() {
        "1" => tides_by_address().await?,
        "2" => tides_by_airport().await?,
        _ => return Ok(()),
    }
    Ok(())
}

async fn tides_by_address() -> Result<(), AppError> {
    // Use the same address file path logic as async_main
    let data_dir_path = PathBuf::from(DATA_DIR);
    let addresses_path = if data_dir_path.exists() {
        data_dir_path.join(ADDRESS_FILE)
    } else {
        // Fallback: use current directory (where binary is run)
        PathBuf::from(ADDRESS_FILE)
    };
    let addresses = load_addresses(&addresses_path)?;
    if addresses.is_empty() {
        println!("No stored addresses found. Please add an address first.");
        return Ok(());
    }
    println!("\nStored Addresses:");
    for (i, (addr, _, _)) in addresses.iter().enumerate() {
        println!("{}. {}", i + 1, addr);
    }
    println!("{}. Return to Tides Menu", addresses.len() + 1);
    print!("Select an address number or return: ");
    io::stdout().flush()?;
    let mut selection_str = String::new();
    io::stdin().read_line(&mut selection_str)?;
    let n = match selection_str.trim().parse::<usize>() {
        Ok(n) if n > 0 && n <= addresses.len() => n - 1,
        _ => return Ok(()),
    };
    let (address, lat, lon) = &addresses[n];
    println!("\nSelected address: {} (Lat: {}, Lon: {})", address, lat, lon);
    let state = extract_state_from_address(address).unwrap_or_else(|| {
        println!("Could not extract state from address. Defaulting to closest station by coordinates.");
        "".to_string()
    });
    find_and_display_tide_station(*lat, *lon, Some(&state)).await
}

async fn tides_by_airport() -> Result<(), AppError> {
    use std::io::Write;
    println!("\n--- Airport Tides Lookup (US only) ---");
    print!("Enter airport code, name, or city (wildcards supported): ");
    io::stdout().flush()?;
    let mut search = String::new();
    io::stdin().read_line(&mut search)?;
    let search = search.trim();
    if search.is_empty() {
        return Ok(());
    }
    let mut results = airports::search_airports(search);
    let us_states = [
        "AL","AK","AZ","AR","CA","CO","CT","DE","FL","GA","HI","ID","IL","IN","IA","KS","KY","LA","ME","MD","MA","MI","MN","MS","MO","MT","NE","NV","NH","NJ","NM","NY","NC","ND","OH","OK","OR","PA","RI","SC","SD","TN","TX","UT","VT","VA","WA","WV","WI","WY"
    ];
    results.retain(|a| a.iso_region.starts_with("US-") && us_states.contains(&a.iso_region[3..].to_uppercase().as_str()));
    if results.is_empty() {
        println!("No US airports found matching '{}'.", search);
        return Ok(());
    }
    for (i, airport) in results.iter().enumerate() {
        println!("{}. {} ({}) - {}, {}", i + 1, airport.name, airport.ident, airport.municipality, airport.iso_region);
    }
    print!("Select an airport by number or return: ");
    io::stdout().flush()?;
    let mut sel = String::new();
    io::stdin().read_line(&mut sel)?;
    let idx = match sel.trim().parse::<usize>() {
        Ok(idx) if idx > 0 && idx <= results.len() => idx - 1,
        _ => return Ok(()),
    };
    let airport = &results[idx];
    let lat = airport.latitude_deg.parse::<f64>().unwrap_or(0.0);
    let lon = airport.longitude_deg.parse::<f64>().unwrap_or(0.0);
    let state = extract_state_from_airport(airport).unwrap_or_else(|| {
        println!("Could not extract state from airport. Defaulting to closest station by coordinates.");
        "".to_string()
    });
    find_and_display_tide_station(lat, lon, Some(&state)).await
}

fn extract_state_from_address(address: &str) -> Option<String> {
    // Try to extract state abbreviation from address (e.g., "PORTLAND, ME, 04109")
    let parts: Vec<&str> = address.split(',').map(|s| s.trim()).collect();
    for part in parts.iter().rev() {
        if part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(part.to_uppercase());
        }
    }
    None
}

fn extract_state_from_airport(airport: &airports::Airport) -> Option<String> {
    // Try to extract state abbreviation from iso_region (e.g., "US-ME")
    if airport.iso_region.starts_with("US-") && airport.iso_region.len() == 5 {
        Some(airport.iso_region[3..].to_string())
    } else {
        None
    }
}

async fn find_and_display_tide_station(lat: f64, lon: f64, state: Option<&str>) -> Result<(), AppError> {
    // Fetch NOAA tide stations list (filter by state if possible)
    let stations_url = "https://api.tidesandcurrents.noaa.gov/mdapi/prod/webapi/stations.json?type=tidepredictions&format=json".to_string();
    let resp = HTTP_CLIENT.get(&stations_url).send().await.map_err(AppError::Network)?;
    let json: serde_json::Value = resp.json().await.map_err(AppError::Network)?;
    let mut stations = Vec::new();
    if let Some(arr) = json["stations"].as_array() {
        for s in arr {
            let id = s["id"].as_str().unwrap_or("").to_string();
            let name = s["name"].as_str().unwrap_or("").to_string();
            let state_val = s["state"].as_str().unwrap_or("").to_string();
            // NOAA returns lat/lon as floats, not strings, if available
            let lat = s["lat"].as_f64().unwrap_or_else(|| s["lat"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0));
            let lon = s["lng"].as_f64().unwrap_or_else(|| s["lng"].as_str().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0));
            stations.push(NWSNoaaTideStation { id, name, state: state_val, lat, lon });
        }
    }
    if stations.is_empty() {
        println!("No tide stations available from NOAA API.");
        return Ok(());
    }
    // Filter by state if provided
    let filtered: Vec<_> = if let Some(state) = state {
        if !state.is_empty() {
            let filtered: Vec<_> = stations.iter().filter(|s| s.state.eq_ignore_ascii_case(state)).collect();
            if filtered.is_empty() {
                println!("No tide stations found in state '{}', searching all stations...", state);
                stations.iter().collect()
            } else {
                filtered
            }
        } else {
            stations.iter().collect()
        }
    } else {
        stations.iter().collect()
    };
    if filtered.is_empty() {
        println!("No tide stations found (even after fallback). NOAA data may be incomplete.");
        return Ok(());
    }
    // Find nearest station
    let nearest = filtered.iter().min_by(|a, b| {
        let da = haversine_distance(lat, lon, a.lat, a.lon);
        let db = haversine_distance(lat, lon, b.lat, b.lon);
        da.partial_cmp(&db).unwrap()
    });
    if let Some(station) = nearest {
        println!("\nNearest NOAA Tide Station with predictions:");
        println!("  {} ({})", station.name, station.id);
        println!("  State: {}", station.state);
        println!("  Location: {:.4}, {:.4}", station.lat, station.lon);
        if station.lat != 0.0 && station.lon != 0.0 {
            println!("  Station location: https://www.google.com/maps?q={},{}", station.lat, station.lon);
        } else {
            println!("  Google Maps: (Coordinates unavailable)");
        }
        // Always show Google Maps for the address/airport location
        println!("  Reference location: https://www.google.com/maps?q={},{}", lat, lon);
        // Fetch and display tide predictions (no need to show time zone info, NOAA returns local time)
        fetch_and_display_tide_predictions(&station.id).await?;
    } else {
        println!("No tide station found.");
    }
    Ok(())
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // km
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians(); // Fixed: match Python logic
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

// Remove timezone logic, just print the time as returned by NOAA
async fn fetch_and_display_tide_predictions(station_id: &str) -> Result<(), AppError> {
    use chrono::{Utc};
    let today = Utc::now().date_naive();
    let tomorrow = today.succ_opt().unwrap();
    for day in [today, tomorrow] {
        let url = format!("https://api.tidesandcurrents.noaa.gov/api/prod/datagetter?product=predictions&application=NOS.COOPS.TAC.WL&begin_date={}&end_date={}&datum=MLLW&station={}&time_zone=lst_ldt&units=english&interval=hilo&format=json", day.format("%Y%m%d"), day.format("%Y%m%d"), station_id);
        let resp = HTTP_CLIENT.get(&url).send().await.map_err(AppError::Network)?;
        let json: serde_json::Value = resp.json().await.map_err(AppError::Network)?;
        println!("\nTide predictions for {} (local station time):", day);
        if let Some(preds) = json["predictions"].as_array() {
            for p in preds {
                let t = p["t"].as_str().unwrap_or("");
                let v = p["v"].as_str().unwrap_or("");
                let typ = p["type"].as_str().unwrap_or("");
                // Format time as am/pm
                let t_ampm = if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M") {
                    dt.format("%Y-%m-%d %I:%M %p").to_string()
                } else {
                    t.to_string()
                };
                println!("  {}: {} ft ({})", t_ampm, v, typ);
            }
        } else {
            println!("  No predictions available.");
        }
    }
    Ok(())
}
