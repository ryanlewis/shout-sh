import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
	test: {
		environment: 'happy-dom',
		include: ['tests/**/*.test.ts'],
	},
	resolve: {
		alias: {
			// Tests never touch the real wasm glue; any import from '@wasm/*'
			// resolves to a stub that throws if actually invoked.
			'@wasm/shout_wasm.js': resolve(root, 'tests/wasm-stub.ts'),
		},
	},
});
