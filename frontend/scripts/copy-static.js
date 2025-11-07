const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const DIST = path.join(ROOT, 'dist');

const TO_COPY = [
  { from: 'css', to: 'css' },
  { from: 'js', to: 'js' },
  { from: 'assets', to: 'assets' },
  { from: 'views', to: 'views' },
];

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

function copyRecursive(src, dest) {
  const stats = fs.statSync(src);

  if (stats.isDirectory()) {
    ensureDir(dest);
    for (const entry of fs.readdirSync(src)) {
      copyRecursive(path.join(src, entry), path.join(dest, entry));
    }
  } else if (stats.isFile()) {
    ensureDir(path.dirname(dest));
    fs.copyFileSync(src, dest);
  }
}

ensureDir(DIST);

for (const item of TO_COPY) {
  const sourcePath = path.join(ROOT, item.from);
  const targetPath = path.join(DIST, item.to);

  if (fs.existsSync(sourcePath)) {
    copyRecursive(sourcePath, targetPath);
  }
}

console.log('Static frontend assets copied to dist/');

