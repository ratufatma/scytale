import { cpSync, mkdirSync, rmSync } from 'node:fs';

rmSync('out', { recursive: true, force: true });
mkdirSync('out', { recursive: true });

for (const asset of ['index.html', 'favicon.svg', 'gemini-svg.svg', 'logo.svg']) {
    cpSync(asset, `out/${asset}`);
}
