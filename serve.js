import { resolve, sep } from 'node:path';

const root = resolve('dist');
const index = Bun.file(resolve(root, 'index.html'));
if (!await index.exists()) throw new Error('Run bun run build first.');

const server = Bun.serve({
  hostname: Bun.env.HOST || '127.0.0.1',
  port: Number(Bun.env.PORT || 5173),
  async fetch(request) {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return new Response('Method not allowed', { status: 405 });
    }
    let path;
    try { path = decodeURIComponent(new URL(request.url).pathname); }
    catch { return new Response('Bad path', { status: 400 }); }
    const filePath = resolve(root, '.' + path);
    if (filePath !== root && !filePath.startsWith(root + sep)) {
      return new Response('Not found', { status: 404 });
    }
    const file = filePath === root ? index : Bun.file(filePath);
    if (!await file.exists()) return new Response('Not found', { status: 404 });
    return new Response(request.method === 'HEAD' ? null : file, {
      headers: { 'Content-Type': file.type, 'X-Content-Type-Options': 'nosniff' },
    });
  },
});
console.log(`RoboWarNet listening on ${server.url}`);
