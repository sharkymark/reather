# Reather, a weather CLI app

This is a Rust application that allows users to get weather information for US addresses using the US Census API for geocoding and the NOAA Weather API for forecasts.

## Features

* Address geocoding via US Census API
* Stores and manages your frequently used addresses
* Current weather conditions from nearest NOAA weather station
* Local forecast summary with detailed information
* Google Maps links for both addresses and weather stations
* No API keys required

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
