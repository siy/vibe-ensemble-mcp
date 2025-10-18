# Vibe Ensemble Dashboard

Web-based monitoring dashboard for the Vibe Ensemble MCP server.

## Features

- **Project Management**: View all projects with configuration details
- **Ticket Monitoring**: Real-time ticket status and progress tracking
- **Comment History**: Full ticket comment history for debugging
- **Real-time Updates**: SSE integration for live status updates
- **Dark/Light Mode**: System-aware theme with manual toggle
- **Minimalist Design**: Clean, responsive UI using Pico CSS

## Tech Stack

- **Frontend**: Solid.js (reactive SPA)
- **Build Tool**: Vite + TypeScript
- **Styling**: Pico CSS (classless, semantic)
- **Backend**: Axum REST API + SSE

## Development

### Prerequisites

- Node.js 18+ and npm
- Rust server running on localhost:3276

### Setup

```bash
cd dashboard
npm install
```

### Development Server

```bash
npm run dev
```

This starts Vite dev server on http://localhost:3000 with proxying to the Rust server.

### Build for Production

```bash
npm run build
```

Outputs to `dashboard/dist/` which is embedded in the Rust binary via `rust-embed`.

## Project Structure

```
dashboard/
├── src/
│   ├── index.tsx              # App entry point
│   ├── App.tsx                # Main app component
│   ├── api.ts                 # Backend API client
│   └── components/
│       ├── ThemeToggle.tsx    # Dark/light mode switcher
│       ├── ProjectSelector.tsx # Project dropdown
│       ├── ProjectDetails.tsx  # Project config display
│       ├── TicketList.tsx      # Ticket table
│       └── TicketDetails.tsx   # Ticket + comments view
├── index.html                  # HTML entry point
├── package.json                # Dependencies
├── vite.config.ts              # Vite configuration
└── tsconfig.json               # TypeScript configuration
```

## API Endpoints

The dashboard consumes these REST API endpoints:

- `GET /api/projects` - List all projects
- `GET /api/projects/:id` - Get project details
- `GET /api/projects/:id/tickets` - List project tickets
- `GET /api/projects/:id/tickets/:ticket_id` - Get ticket with comments
- `GET /sse` - Real-time event stream

## Accessing the Dashboard

After building the dashboard and running the Rust server:

```
http://localhost:3276/dashboard
```

The dashboard is embedded in the server binary and served at the `/dashboard` route.
