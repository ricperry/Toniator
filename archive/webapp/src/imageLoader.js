const SAMPLE_FILE_NAME = "built-in-sample.svg";
const SAMPLE_SOURCE_URL = new URL("../built-in-sample.svg", import.meta.url);
const EMBEDDED_SAMPLE_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="900" height="620" viewBox="0 0 900 620">
  <defs>
    <linearGradient id="warm" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#ffcf33"/>
      <stop offset="0.48" stop-color="#ec008c"/>
      <stop offset="1" stop-color="#0047ff"/>
    </linearGradient>
    <radialGradient id="cool" cx="42%" cy="40%" r="70%">
      <stop offset="0" stop-color="#ffffff"/>
      <stop offset="0.45" stop-color="#00aeef"/>
      <stop offset="1" stop-color="#08111f"/>
    </radialGradient>
  </defs>
  <rect width="100%" height="100%" fill="url(#warm)"/>
  <circle cx="320" cy="290" r="210" fill="url(#cool)" opacity="0.9"/>
  <rect x="515" y="110" width="250" height="330" rx="42" fill="#101114" opacity="0.72"/>
  <path d="M95 505 C230 390 330 645 490 505 S720 420 820 540" fill="none" stroke="#fff" stroke-width="54" stroke-linecap="round" opacity="0.82"/>
  <text x="548" y="300" font-family="Arial, sans-serif" font-size="86" font-weight="800" fill="#fff">T</text>
</svg>`;

export async function loadSourceFile(file) {
  if (!file) {
    throw new Error("No file selected.");
  }

  const dataUrl = await readAsDataUrl(file);
  const bytes = await file.arrayBuffer();
  const image = new Image();
  image.decoding = "async";

  if (file.type === "image/svg+xml" || file.name.toLowerCase().endsWith(".svg")) {
    const text = await file.text();
    const svgSize = getSvgIntrinsicSize(text);
    image.src = dataUrl;
    await image.decode();
    const width = svgSize.width || image.naturalWidth || 800;
    const height = svgSize.height || image.naturalHeight || 600;
    const rasterized = await rasterizeImage(image, width, height);
    return {
      fileName: file.name,
      type: "svg",
      dataUrl,
      previewDataUrl: rasterized?.dataUrl ?? dataUrl,
      image: rasterized?.image ?? image,
      width,
      height,
      physicalSize: svgSize.physicalSize ?? fallbackPhysicalSize(width, height),
    };
  }

  image.src = dataUrl;
  await image.decode();
  const physicalSize =
    parseRasterPhysicalSize(bytes, image.naturalWidth, image.naturalHeight) ??
    fallbackPhysicalSize(image.naturalWidth, image.naturalHeight);
  return {
    fileName: file.name,
    type: "raster",
    dataUrl,
    previewDataUrl: dataUrl,
    image,
    width: image.naturalWidth,
    height: image.naturalHeight,
    physicalSize,
  };
}

export async function createSampleSource() {
  try {
    const response = await fetch(SAMPLE_SOURCE_URL);
    if (!response.ok) {
      throw new Error(`Could not load ${SAMPLE_FILE_NAME}.`);
    }

    return loadBlobAsSource(await response.blob(), SAMPLE_FILE_NAME, "image/svg+xml");
  } catch (error) {
    console.warn(`Falling back to embedded ${SAMPLE_FILE_NAME}.`, error);
    return loadBlobAsSource(
      new Blob([EMBEDDED_SAMPLE_SVG], { type: "image/svg+xml" }),
      SAMPLE_FILE_NAME,
      "image/svg+xml",
    );
  }
}

function loadBlobAsSource(blob, fileName, fallbackType) {
  const file = new File([blob], fileName, {
    type: blob.type || fallbackType,
  });

  return loadSourceFile(file);
}

async function rasterizeImage(image, width, height) {
  try {
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(width));
    canvas.height = Math.max(1, Math.round(height));
    const context = canvas.getContext("2d");
    context.clearRect(0, 0, canvas.width, canvas.height);
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const dataUrl = canvas.toDataURL("image/png");
    const rasterImage = new Image();
    rasterImage.decoding = "async";
    rasterImage.src = dataUrl;
    await rasterImage.decode();
    return { dataUrl, image: rasterImage };
  } catch {
    return null;
  }
}

function readAsDataUrl(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function getSvgIntrinsicSize(text) {
  const parser = new DOMParser();
  const doc = parser.parseFromString(text, "image/svg+xml");
  const svg = doc.documentElement;

  if (!svg || svg.nodeName.toLowerCase() !== "svg") {
    return { width: 0, height: 0 };
  }

  const width = parseSvgLength(svg.getAttribute("width"));
  const height = parseSvgLength(svg.getAttribute("height"));
  if (width.px && height.px) {
    return {
      width: width.px,
      height: height.px,
      physicalSize:
        width.inches && height.inches
          ? { widthInches: width.inches, heightInches: height.inches, source: "embedded" }
          : null,
    };
  }

  const viewBox = svg.getAttribute("viewBox");
  if (viewBox) {
    const [, , vbWidth, vbHeight] = viewBox.split(/[\s,]+/).map(Number);
    if (vbWidth > 0 && vbHeight > 0) {
      return { width: vbWidth, height: vbHeight, physicalSize: null };
    }
  }

  return { width: width.px || 0, height: height.px || 0, physicalSize: null };
}

function parseSvgLength(value) {
  if (!value) return { px: 0, inches: 0 };
  const match = String(value).trim().match(/^([+-]?(?:\d+\.?\d*|\.\d+))(px|in|cm|mm|pt|pc)?$/i);
  if (!match) return { px: 0, inches: 0 };
  const number = Number.parseFloat(match[1]);
  if (!Number.isFinite(number) || number <= 0) return { px: 0, inches: 0 };
  const unit = (match[2] || "px").toLowerCase();
  const inchesPerUnit = {
    in: 1,
    cm: 1 / 2.54,
    mm: 1 / 25.4,
    pt: 1 / 72,
    pc: 1 / 6,
  }[unit];

  if (inchesPerUnit) {
    const inches = number * inchesPerUnit;
    return { px: inches * 96, inches };
  }

  return { px: number, inches: 0 };
}

function parseRasterPhysicalSize(buffer, width, height) {
  return parsePngPhysicalSize(buffer, width, height) ?? parseJpegPhysicalSize(buffer, width, height);
}

function parsePngPhysicalSize(buffer, width, height) {
  const bytes = new Uint8Array(buffer);
  const pngSignature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 33 || !pngSignature.every((byte, index) => bytes[index] === byte)) {
    return null;
  }

  let offset = 8;
  while (offset + 12 <= bytes.length) {
    const length = readUint32(bytes, offset);
    const type = String.fromCharCode(bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]);
    const data = offset + 8;
    if (type === "pHYs" && length >= 9 && bytes[data + 8] === 1) {
      const xPixelsPerMeter = readUint32(bytes, data);
      const yPixelsPerMeter = readUint32(bytes, data + 4);
      if (xPixelsPerMeter > 0 && yPixelsPerMeter > 0) {
        return {
          widthInches: width / (xPixelsPerMeter * 0.0254),
          heightInches: height / (yPixelsPerMeter * 0.0254),
          dpiX: xPixelsPerMeter * 0.0254,
          dpiY: yPixelsPerMeter * 0.0254,
          source: "embedded",
        };
      }
    }
    offset += 12 + length;
  }

  return null;
}

function parseJpegPhysicalSize(buffer, width, height) {
  const bytes = new Uint8Array(buffer);
  if (bytes.length < 20 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return null;

  let offset = 2;
  while (offset + 4 < bytes.length) {
    if (bytes[offset] !== 0xff) return null;
    const marker = bytes[offset + 1];
    const length = (bytes[offset + 2] << 8) | bytes[offset + 3];
    if (marker === 0xe0 && length >= 16 && jpegMarkerText(bytes, offset + 4, 5) === "JFIF\u0000") {
      const units = bytes[offset + 11];
      const xDensity = (bytes[offset + 12] << 8) | bytes[offset + 13];
      const yDensity = (bytes[offset + 14] << 8) | bytes[offset + 15];
      if (xDensity > 0 && yDensity > 0 && (units === 1 || units === 2)) {
        const dpiX = units === 1 ? xDensity : xDensity * 2.54;
        const dpiY = units === 1 ? yDensity : yDensity * 2.54;
        return {
          widthInches: width / dpiX,
          heightInches: height / dpiY,
          dpiX,
          dpiY,
          source: "embedded",
        };
      }
    }
    offset += 2 + length;
  }

  return null;
}

function fallbackPhysicalSize(width, height) {
  return {
    widthInches: width / 96,
    heightInches: height / 96,
    dpiX: 96,
    dpiY: 96,
    source: "pixel",
  };
}

function readUint32(bytes, offset) {
  return (
    bytes[offset] * 0x1000000 +
    ((bytes[offset + 1] << 16) | (bytes[offset + 2] << 8) | bytes[offset + 3])
  );
}

function jpegMarkerText(bytes, offset, length) {
  return String.fromCharCode(...bytes.slice(offset, offset + length));
}
