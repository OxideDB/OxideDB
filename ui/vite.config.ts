import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

const manualChunkGroups = {
  'react-vendor': ['react', 'react-dom', 'react-router-dom'],
  'radix-ui': [
    '@radix-ui/react-accordion',
    '@radix-ui/react-alert-dialog',
    '@radix-ui/react-aspect-ratio',
    '@radix-ui/react-avatar',
    '@radix-ui/react-checkbox',
    '@radix-ui/react-collapsible',
    '@radix-ui/react-context-menu',
    '@radix-ui/react-dialog',
    '@radix-ui/react-dropdown-menu',
    '@radix-ui/react-hover-card',
    '@radix-ui/react-label',
    '@radix-ui/react-menubar',
    '@radix-ui/react-navigation-menu',
    '@radix-ui/react-popover',
    '@radix-ui/react-progress',
    '@radix-ui/react-radio-group',
    '@radix-ui/react-scroll-area',
    '@radix-ui/react-select',
    '@radix-ui/react-separator',
    '@radix-ui/react-slider',
    '@radix-ui/react-slot',
    '@radix-ui/react-switch',
    '@radix-ui/react-tabs',
    '@radix-ui/react-toast',
    '@radix-ui/react-toggle',
    '@radix-ui/react-toggle-group',
    '@radix-ui/react-tooltip',
  ],
  charts: ['recharts'],
  forms: ['react-hook-form', '@hookform/resolvers', 'zod'],
  icons: ['lucide-react'],
  'date-utils': ['date-fns', 'react-day-picker'],
  'styling-utils': [
    'clsx',
    'class-variance-authority',
    'tailwind-merge',
    'next-themes',
  ],
  'ui-utils': [
    'cmdk',
    'sonner',
    'vaul',
    'embla-carousel-react',
    'input-otp',
    'react-resizable-panels',
  ],
} satisfies Record<string, string[]>

function manualChunks(id: string) {
  const normalizedId = id.replaceAll('\\', '/')
  const nodeModulesSegment = '/node_modules/'
  const nodeModulesIndex = normalizedId.lastIndexOf(nodeModulesSegment)

  if (nodeModulesIndex === -1) {
    return undefined
  }

  const packagePath = normalizedId.slice(nodeModulesIndex + nodeModulesSegment.length)

  for (const [chunkName, packages] of Object.entries(manualChunkGroups)) {
    if (packages.some((pkg) => packagePath === pkg || packagePath.startsWith(`${pkg}/`))) {
      return chunkName
    }
  }

  return undefined
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  base: '/admin/',
  resolve: {
    alias: {
      "@": resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    sourcemap: false,
    minify: 'esbuild',
    emptyOutDir: true,
    chunkSizeWarningLimit: 1000,
    rollupOptions: {
      onwarn(warning, warn) {
        if (warning.code === 'UNUSED_EXTERNAL_IMPORT') return;
        if (warning.code === 'MODULE_LEVEL_DIRECTIVE') return;
        warn(warning);
      },
      output: {
        manualChunks,
      }
    }
  },
  server: {
    port: 3000,
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, '')
      }
    }
  },
  logLevel: 'warn'
})
