# OxideDB Admin UI

A modern React-based admin interface for OxideDB built with Vite, TypeScript, and TailwindCSS.

## Features

- **Collections Management**: Create, view, and delete collections
- **Records Management**: Full CRUD operations for records within collections
- **Health Monitoring**: Real-time system health status
- **Modern UI**: Clean, responsive design with TailwindCSS
- **Type Safety**: Full TypeScript support
- **Fast Development**: Powered by Vite for instant hot reloading

## Prerequisites

- Node.js 20.19.x, 22.13+, or 24+
- npm or yarn
- OxideDB server running on `localhost:8080`

## Installation

1. Navigate to the UI directory:
```bash
cd ui
```

2. Install dependencies:
```bash
npm install
```

## Development

Start the development server:
```bash
npm run dev
```

The UI will be available at `http://localhost:3000`

## Building for Production

Build the project:
```bash
npm run build
```

Preview the production build:
```bash
npm run preview
```

## Project Structure

```
ui/
├── src/
│   ├── components/     # Reusable UI components
│   │   └── Layout.tsx  # Main layout with sidebar
│   │   
│   ├── pages/          # Route components
│   │   ├── Collections.tsx
│   │   ├── Records.tsx
│   │   ├── Health.tsx
│   │   └── Settings.tsx
│   ├── services/       # API service layer
│   │   └── api.ts
│   ├── types/          # TypeScript type definitions
│   │   └── api.ts
│   ├── App.tsx         # Main app component with routing
│   ├── main.tsx        # Application entry point
│   └── index.css       # Global styles and TailwindCSS
├── public/             # Static assets
├── package.json
├── vite.config.ts      # Vite configuration
├── tailwind.config.js  # TailwindCSS configuration
└── tsconfig.json       # TypeScript configuration
```

## Available Scripts

- `npm run dev` - Start development server
- `npm run build` - Build for production
- `npm run preview` - Preview production build
- `npm run lint` - Run ESLint
- `npm run clean` - Clean build directory

## API Integration

The UI connects to the OxideDB API server at `http://localhost:8080`. Make sure the backend is running before starting the frontend.

## Technology Stack

- **React 19** - UI framework
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **React Router** - Client-side routing
- **TailwindCSS** - Utility-first CSS framework
- **Lucide React** - Icon library
- **ESLint** - Code linting
