import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { inflateSync } from "node:zlib";
import net from "node:net";

const root = new URL("..", import.meta.url).pathname;
const artifactDir = join(root, "test-artifacts", "screenshots");
const viewport = { width: 1600, height: 1000 };

const scenarios = [
  {
    name: "small-output-fit-preview",
    validateFit: true,
    params: {
      markMode: "curve",
      curveSpan: "full-width",
      syncCurveChannels: "false",
      editCurve: "c",
      outputWidth: "110",
      outputHeight: "76",
      preserveAspect: "false",
      cells: "1024",
      gridScale: "786",
      minMark: "0",
      maxMark: "85",
      background: "true",
      cEnabled: "true",
      cRotation: "0",
      cScale: "1",
      cThreshold: "0",
      cMaxSize: "100",
      cResolution: "1",
      cOffsetX: "0",
      cOffsetY: "0",
      cOpacity: "92",
      cPath: "M -0.5 0 C -0.2 -0.25 0.2 0.25 0.5 0",
      cConnect: "false",
      cSmoothSeam: "true",
    },
  },
  {
    name: "curve-independent-all-controls",
    params: {
      markMode: "curve",
      curveSpan: "motif-pattern",
      syncCurveChannels: "false",
      editCurve: "k",
      outputWidth: "640",
      outputHeight: "420",
      preserveAspect: "false",
      cells: "72",
      gridScale: "68",
      minMark: "5",
      maxMark: "95",
      valueMode: "cmyk",
      background: "true",
      cEnabled: "true",
      cColor: "#00aeef",
      cRotation: "0",
      cMotifCoverageMode: "manual",
      cMotifBleed: "2",
      cTileCount: "8",
      cTileSpacing: "34",
      cStackCount: "6",
      cStackSpacing: "28",
      cScale: "0.8",
      cThreshold: "3",
      cMaxSize: "60",
      cResolution: "0.6",
      cOffsetX: "-4",
      cOffsetY: "3",
      cOpacity: "55",
      cPath: "M -0.5 0 C -0.3 -0.3 0.3 0.3 0.5 0",
      cConnect: "true",
      cSmoothSeam: "true",
      mEnabled: "true",
      mColor: "#ec008c",
      mRotation: "45",
      mMotifCoverageMode: "manual",
      mMotifBleed: "2",
      mTileCount: "8",
      mTileSpacing: "34",
      mStackCount: "6",
      mStackSpacing: "28",
      mScale: "1.1",
      mThreshold: "20",
      mMaxSize: "80",
      mResolution: "1",
      mOffsetX: "2",
      mOffsetY: "-5",
      mOpacity: "70",
      mPath: "M -0.5 0 L 0 0.22 L 0.5 0",
      mConnect: "false",
      mSmoothSeam: "false",
      yEnabled: "true",
      yColor: "#ffd400",
      yRotation: "90",
      yMotifCoverageMode: "manual",
      yMotifBleed: "2",
      yTileCount: "8",
      yTileSpacing: "34",
      yStackCount: "6",
      yStackSpacing: "28",
      yScale: "1.4",
      yThreshold: "40",
      yMaxSize: "100",
      yResolution: "1.4",
      yOffsetX: "7",
      yOffsetY: "5",
      yOpacity: "85",
      yPath: "M -0.5 0 C -0.1 0.35 0.1 -0.35 0.5 0",
      yConnect: "true",
      ySmoothSeam: "false",
      kEnabled: "true",
      kColor: "#111111",
      kRotation: "135",
      kMotifCoverageMode: "manual",
      kMotifBleed: "2",
      kTileCount: "8",
      kTileSpacing: "34",
      kStackCount: "6",
      kStackSpacing: "28",
      kScale: "1.8",
      kThreshold: "60",
      kMaxSize: "125",
      kResolution: "2",
      kOffsetX: "-9",
      kOffsetY: "-3",
      kOpacity: "100",
      kPath: "M -0.5 0 C -0.25 0.25 0.25 -0.25 0.5 0",
      kConnect: "true",
      kSmoothSeam: "true",
    },
  },
  {
    name: "shape-per-channel-presets",
    params: {
      markMode: "shape",
      geometryMode: "per-channel",
      outputWidth: "640",
      outputHeight: "420",
      preserveAspect: "false",
      cells: "84",
      gridScale: "58",
      minMark: "10",
      maxMark: "90",
      valueMode: "cmyk",
      background: "true",
      cEnabled: "true",
      cPreset: "triangle",
      cRotation: "0",
      cScale: "0.75",
      cThreshold: "0",
      cMaxSize: "50",
      cResolution: "0.5",
      cOffsetX: "-8",
      cOffsetY: "6",
      cOpacity: "55",
      mEnabled: "true",
      mPreset: "pentagon",
      mRotation: "60",
      mScale: "1.1",
      mThreshold: "20",
      mMaxSize: "75",
      mResolution: "1",
      mOffsetX: "4",
      mOffsetY: "-6",
      mOpacity: "70",
      yEnabled: "true",
      yPreset: "hexagon",
      yRotation: "120",
      yScale: "1.4",
      yThreshold: "40",
      yMaxSize: "100",
      yResolution: "1.5",
      yOffsetX: "8",
      yOffsetY: "5",
      yOpacity: "85",
      kEnabled: "true",
      kPreset: "rectangle",
      kRotation: "180",
      kScale: "1.8",
      kThreshold: "60",
      kMaxSize: "125",
      kResolution: "2",
      kOffsetX: "-9",
      kOffsetY: "-3",
      kOpacity: "100",
    },
  },
];

mkdirSync(artifactDir, { recursive: true });

const firefox = findFirefox();
if (!firefox) {
  console.warn("preview screenshot tests skipped: Firefox executable not found");
  process.exit(0);
}

execFileSync("npm", ["run", "build"], {
  cwd: root,
  stdio: "inherit",
});

const port = await getFreePort();
const server = spawn(
  process.execPath,
  ["./node_modules/vite/bin/vite.js", "preview", "--host", "127.0.0.1", "--port", String(port)],
  {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
  },
);

try {
  await waitForServer(`http://127.0.0.1:${port}/`);

  for (const scenario of scenarios) {
    const screenshotPath = join(artifactDir, `${scenario.name}.png`);
    const url = `http://127.0.0.1:${port}/?${new URLSearchParams(scenario.params)}`;
    captureScreenshot({ firefox, url, screenshotPath });

    const png = parsePng(screenshotPath);
    assert.equal(png.width, viewport.width, `${scenario.name}: unexpected screenshot width`);
    assert.equal(png.height, viewport.height, `${scenario.name}: unexpected screenshot height`);

    if (scenario.validateFit) {
      const brightBounds = findBrightPixelBounds(png, {
        minX: Math.round(viewport.width * 0.25),
        minY: Math.round(viewport.height * 0.24),
        maxX: viewport.width,
        maxY: viewport.height,
      });

      assert.ok(brightBounds, `${scenario.name}: no bright rendered artboard detected`);
      assert.ok(
        brightBounds.width >= 650,
        `${scenario.name}: fitted preview is too narrow (${brightBounds.width}px)`,
      );
      assert.ok(
        brightBounds.height >= 430,
        `${scenario.name}: fitted preview is too short (${brightBounds.height}px)`,
      );
    }
  }
} finally {
  server.kill("SIGTERM");
}

console.log(`preview screenshot tests passed (${artifactDir})`);

function findFirefox() {
  for (const binary of ["firefox", "firefox-esr"]) {
    try {
      return execFileSync("which", [binary], { encoding: "utf8" }).trim();
    } catch {
      // Try the next binary name.
    }
  }
  return "";
}

function captureScreenshot({ firefox, url, screenshotPath }) {
  const profile = mkdtempSync(join(tmpdir(), "toniator-firefox-"));
  try {
    execFileSync(
      firefox,
      [
        "--headless",
        "--no-remote",
        "--profile",
        profile,
        "--window-size",
        `${viewport.width},${viewport.height}`,
        "--screenshot",
        screenshotPath,
        url,
      ],
      {
        cwd: root,
        stdio: "ignore",
        timeout: 60_000,
      },
    );
  } finally {
    rmSync(profile, { recursive: true, force: true });
  }
}

async function getFreePort() {
  const listener = net.createServer();
  await new Promise((resolve, reject) => {
    listener.once("error", reject);
    listener.listen(0, "127.0.0.1", resolve);
  });
  const { port } = listener.address();
  await new Promise((resolve) => listener.close(resolve));
  return port;
}

async function waitForServer(url) {
  const started = Date.now();
  let lastError = null;

  while (Date.now() - started < 20_000) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }

  throw new Error(`Timed out waiting for Vite preview server: ${lastError?.message ?? "no response"}`);
}

function parsePng(path) {
  const bytes = readFileSync(path);
  assert.equal(bytes.toString("ascii", 1, 4), "PNG", `${path}: not a PNG`);

  let offset = 8;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  const idat = [];

  while (offset < bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.toString("ascii", offset + 4, offset + 8);
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;

    if (type === "IHDR") {
      width = bytes.readUInt32BE(dataStart);
      height = bytes.readUInt32BE(dataStart + 4);
      bitDepth = bytes[dataStart + 8];
      colorType = bytes[dataStart + 9];
    } else if (type === "IDAT") {
      idat.push(bytes.subarray(dataStart, dataEnd));
    } else if (type === "IEND") {
      break;
    }

    offset = dataEnd + 4;
  }

  assert.equal(bitDepth, 8, `${path}: expected 8-bit PNG`);
  const channels = colorType === 6 ? 4 : colorType === 2 ? 3 : 0;
  assert.ok(channels, `${path}: unsupported PNG color type ${colorType}`);

  const inflated = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const pixels = new Uint8Array(width * height * 4);
  let inputOffset = 0;
  let previous = new Uint8Array(stride);

  for (let y = 0; y < height; y += 1) {
    const filter = inflated[inputOffset];
    inputOffset += 1;
    const row = Uint8Array.from(inflated.subarray(inputOffset, inputOffset + stride));
    inputOffset += stride;
    unfilterRow(row, previous, channels, filter);

    for (let x = 0; x < width; x += 1) {
      const sourceIndex = x * channels;
      const targetIndex = (y * width + x) * 4;
      pixels[targetIndex] = row[sourceIndex];
      pixels[targetIndex + 1] = row[sourceIndex + 1];
      pixels[targetIndex + 2] = row[sourceIndex + 2];
      pixels[targetIndex + 3] = channels === 4 ? row[sourceIndex + 3] : 255;
    }

    previous = row;
  }

  return { width, height, pixels };
}

function unfilterRow(row, previous, bytesPerPixel, filter) {
  for (let index = 0; index < row.length; index += 1) {
    const left = index >= bytesPerPixel ? row[index - bytesPerPixel] : 0;
    const up = previous[index] ?? 0;
    const upLeft = index >= bytesPerPixel ? previous[index - bytesPerPixel] : 0;

    if (filter === 1) {
      row[index] = (row[index] + left) & 0xff;
    } else if (filter === 2) {
      row[index] = (row[index] + up) & 0xff;
    } else if (filter === 3) {
      row[index] = (row[index] + Math.floor((left + up) / 2)) & 0xff;
    } else if (filter === 4) {
      row[index] = (row[index] + paeth(left, up, upLeft)) & 0xff;
    } else {
      assert.equal(filter, 0, `unsupported PNG filter ${filter}`);
    }
  }
}

function paeth(left, up, upLeft) {
  const estimate = left + up - upLeft;
  const distanceLeft = Math.abs(estimate - left);
  const distanceUp = Math.abs(estimate - up);
  const distanceUpLeft = Math.abs(estimate - upLeft);

  if (distanceLeft <= distanceUp && distanceLeft <= distanceUpLeft) return left;
  if (distanceUp <= distanceUpLeft) return up;
  return upLeft;
}

function findBrightPixelBounds(png, crop) {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (let y = crop.minY; y < crop.maxY; y += 1) {
    for (let x = crop.minX; x < crop.maxX; x += 1) {
      const index = (y * png.width + x) * 4;
      const red = png.pixels[index];
      const green = png.pixels[index + 1];
      const blue = png.pixels[index + 2];
      if (red + green + blue < 640) continue;

      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }

  return Number.isFinite(minX)
    ? { minX, minY, maxX, maxY, width: maxX - minX + 1, height: maxY - minY + 1 }
    : null;
}
