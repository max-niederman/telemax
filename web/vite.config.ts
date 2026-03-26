import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const socketPath = process.env.XDG_RUNTIME_DIR
  ? `${process.env.XDG_RUNTIME_DIR}/telemax.sock`
  : '/run/user/1000/telemax.sock';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: {
      '/api': {
        target: { socketPath },
        ws: true,
      }
    }
  }
});
