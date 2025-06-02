# Reather, a weather CLI app

This is a Rust application that allows users to get weather information for US addresses using the US Census API for geocoding and the NOAA Weather API for forecasts.

## Features

* Address geocoding via US Census API
* Stores and manages your frequently used addresses
* Current weather conditions from nearest NOAA weather station
* Local forecast summary with detailed information
* External links including:
  * Google Maps for both addresses and weather stations
  * Flightradar24 for weather stations at airports
  * Zillow real estate listings based on ZIP code
* No API keys required
* Portable design - works anywhere without installation requirements

## Usage

1. **First Run**: 
   - When first run, you'll be prompted to populate with seed addresses
   - Answer 'yes' to add example addresses or 'no' to start with an empty list

2. **Main Menu**:
   - Add a new address
   - Choose an existing address for weather information
   - Delete addresses from your saved list
   - Exit the application

3. **Weather Information**:
   - View current conditions from the nearest weather station
   - See detailed forecast information
   - Access external links for maps, flights, and real estate information

## Portability

Reather is designed to be portable and can run from any location:

* **No data directory required**: The application can run without needing a `data` directory to be present
* **Adaptive storage**: 
  - If run from the project directory with a `data` folder, it will use `data/addresses.txt`
  - If run from any other location, it creates and uses `addresses.txt` in the same directory as the executable
* **Built-in seed addresses**: Seed addresses are hardcoded into the application for ease of use

## Installation

### Pre-built Binary

For convenience, a pre-built binary is included in this repository. You can download and use it directly without setting up a Rust development environment:

1. **Download the binary**:
   - Go to the [GitHub repository](https://github.com/yourusername/reather)
   - Navigate to the `/target/release/` directory
   - Download the `reather` executable

2. **Make it executable** (on macOS/Linux):
   ```bash
   chmod +x path/to/reather
   ```

3. **Run the application**:
   ```bash
   ./reather
   ```

### Building from Source

If you prefer to build the application yourself:

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/reather.git
   cd reather
   ```

3. **Build the release version**:
   ```bash
   cargo build --release
   ```

4. **Run the application**:
   ```bash
   ./target/release/reather
   ```

The release build will be optimized for performance and is recommended for regular usage.

## Development Container

This project is configured to run within a Dev Container. The setup includes:

*   Rust (latest)
*   Other development tools (git, curl, htop etc.)
*   LLDB for debugging
*   VS Code extensions:
    *   Rust Analyzer
    *   GitHub Copilot
*   GitHub Copilot VS Code extension

**Non-root User:**
The Dockerfile creates a non-root user (`coder`) with UID 1000 and GID 1000. This user is used to run the application and perform development tasks within the container.

## Debugging

Debugging is best done in the development container, which has all the required packages already installed (LLDB, GDB).

### Setting up breakpoints and debugging your code:

1. **Set breakpoints** by clicking in the margin next to line numbers to create a red dot
2. **Start debugging** by pressing F5 or clicking the debug icon in the sidebar and then the green play button
3. **Step through code** using:
   - F10 to step over (execute current line and move to next)
   - F11 to step into function calls
   - Shift+F11 to step out of the current function
   - F5 to continue execution until the next breakpoint

4. **Inspect variables** in the Debug panel while execution is paused
   - Local variables are automatically displayed
   - Add expressions to watch in the Watch panel
   - View the call stack to trace execution path

The launch configurations are already set up in the `.vscode/launch.json` file.

## License

MIT License

Copyright (c) 2025 Mark Milligan

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
