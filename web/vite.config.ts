import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  resolve: {
    conditions: ['browser'],
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8787',
      },
      '/ws': {
        target: 'http://127.0.0.1:8787',
        ws: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/lib/starmap/test-setup.ts'],
  },
})
