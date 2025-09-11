# Vibe Ensemble Website

This directory contains the static website for Vibe Ensemble MCP Server.

## Structure

- `index.html` - Main website with project description and features
- `install.sh` - Cross-platform installation script for Linux/macOS
- `install.ps1` - Installation script for Windows PowerShell
- `netlify.toml` - Netlify configuration for deployment and routing
- `static/` - Static assets (logos, favicon, etc.)

## Netlify Configuration

The site is configured for deployment on Netlify with:

- Static site hosting from root directory
- Special routing for `get.vibeensemble.dev` subdomain that serves install scripts
- Proper content types and caching headers for scripts
- Security headers for HTML pages

## Installation Scripts

Both scripts automatically:
- Detect the user's platform and architecture
- Fetch the latest release from GitHub
- Download the appropriate binary package
- Install to `~/.local/bin` (Linux/macOS) or `%USERPROFILE%\.local\bin` (Windows)
- Provide PATH setup instructions

### Usage

**Linux/macOS:**
```bash
curl -fsSL https://get.vibeensemble.dev/install.sh | sh
```

**Windows PowerShell:**
```powershell
iwr -useb https://get.vibeensemble.dev/install.ps1 | iex
```

## Local Development

To test the website locally, serve the files with any static web server:

```bash
# Using Python
python -m http.server 8000

# Using Node.js
npx serve .

# Using PHP
php -S localhost:8000
```

Then visit `http://localhost:8000` to view the site.

### Local File Viewing

The website uses relative paths (`static/logo.png` instead of `/static/logo.png`) to ensure compatibility when viewing directly from the filesystem (file:// protocol). This allows the site to work both when served from a web server and when opening the HTML file directly in a browser.