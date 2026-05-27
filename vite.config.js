import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// https://vite.dev/config/
export default defineConfig(({ command }) => ({
  plugins: [
    svelte({
      emitCss: command === 'build',
    }),
  ],
  server: {
    host: '127.0.0.1',
    port: 5174,
    strictPort: true,
    watch: {
      ignored: [
        '**/.git/**',
        '**/dist/**',
        '**/target/**',
        '**/node_modules/**',
        '**/untracked/**',
      ],
    },
  },
}))
